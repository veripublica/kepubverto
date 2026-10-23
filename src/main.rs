//! kepubverto's CLI, following the **veripublica CLI convention v0.7**
//! (<https://github.com/veripublica/conventions>).
//!
//! kepubverto is a *transformer*: exactly one input, a converted copy written
//! beside it (`book.epub` → `book.kepub.epub`), never in place and never over
//! an existing file without `-f`. It asks nothing, so it has no prompts.
//!
//! The argument grammar is epubsana's, ported deliberately rather than
//! re-derived: one family, one parser, one set of surprises.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use epubveri::envelope::{Envelope, Input, Item};
use kepubverto::{Conversion, Hyphens, Options, Outcome};
use serde::Serialize;

const HELP: &str = "\
kepubverto — convert an EPUB to a KEPUB for Kobo e-readers

USAGE:
    kepubverto -i <PATH> [OPTIONS]

OPTIONS:
    -i, --input <PATH>      The input. The only input form; positional paths are
                            not accepted.
    -o, --output <PATH>     Where to write the output. Defaults to
                            <input-stem>.kepub.epub, beside the input.
    -f, --force             Permit replacing existing output files. Never lifts
                            the output-equals-input refusal.
        --hyphens <auto|none>
                            Add CSS that turns hyphenation on (auto) or off
                            (none) in every converted document. auto leaves
                            headings, table cells and the book's own
                            per-element choices alone; none overrides the
                            book. Without it, the book decides.
        --css <CSS>         Add this CSS to every converted document, after
                            kepubverto's own rules; repeat it to add more.
                            Refused (exit 2, nothing written) if it refers to a
                            file, has @font-face, or would add a finding
                            epubveri reports.
        --format <FORMAT>   Report format: human (the default), or json: the
                            shared machine envelope (one JSON object on stdout,
                            see the veripublica FORMATS spec).
    -V, --version           Print kepubverto <version> to stdout and exit 0.
    -h, --help              Print this help to stdout and exit 0.

EXAMPLES:
    kepubverto -i book.epub                 # writes book.kepub.epub
    kepubverto -i book.epub --hyphens auto  # also hyphenate
    kepubverto -i book.epub --css 'p { text-indent: 1.5em }'
    kepubverto -i book.epub -o out.kepub.epub -f

The original is never modified in place, and an existing output file is never
silently replaced (use -f). kepubverto converts; it does not validate. To check
a book first, run epubveri -i book.epub.

EXIT CODES:
    0   every content document was converted (or already had been).
    1   the book was written, but some content documents could not be
        converted and were left as they were; each is named on stderr.
    2   kepubverto could not run: a usage error, a refused --css, an unreadable
        EPUB, an output path that is the input, an existing output file
        without -f, or an I/O failure. With --format json, an EPUB that cannot be read or a book that
        cannot be written is still described in the envelope; the rest print
        no envelope.

Conforms to veripublica conventions v0.7.";

/// The convention version this CLI and its envelope implement (FORMATS §1.1):
/// asserted here, never inherited from epubveri.
const CONVENTION: &str = "0.7";

/// The outcome of parsing `argv`, decided entirely before any work is done.
#[derive(Debug, PartialEq)]
enum Cli {
    Run(Run),
    /// `-h`/`--help` was requested (short-circuits everything else).
    Help,
    /// `-V`/`--version` was requested.
    Version,
    /// The invocation was malformed; the string is the short problem message
    /// (without the `error:` prefix or the `--help` pointer main adds).
    Usage(String),
}

#[derive(Debug, PartialEq)]
struct Run {
    input: String,
    output: Option<String>,
    force: bool,
    hyphens: Option<Hyphens>,
    css: Vec<String>,
    json: bool,
}

/// Parse the arguments after the program name into a [`Cli`] decision.
///
/// The accepted syntaxes are the convention's (§3.3): `--name value` and
/// `--name=value`; `-i value` and the attached `-ivalue`; boolean short flags
/// bundle (`-fh`); a value-taking short flag consumes the rest of its token, or
/// the next token, as its value (POSIX: `-iv` means `-i v`); and the token after
/// a value-taking option is *always* its value, never re-parsed as an option
/// (`-i -q.epub` names the file `-q.epub`).
fn parse(args: &[String]) -> Cli {
    let mut inputs: Vec<String> = Vec::new();
    let mut output: Option<String> = None;
    let mut format: Option<String> = None;
    let mut hyphens: Option<String> = None;
    let mut css: Vec<String> = Vec::new();
    let mut force = false;
    let mut help = false;
    let mut version = false;
    let mut error: Option<String> = None;

    // Record the first usage error but keep scanning, so a later `-h` can still
    // short-circuit a malformed line (§5). Help wins over any error below.
    macro_rules! fail {
        ($($a:tt)*) => {{ if error.is_none() { error = Some(format!($($a)*)); } }};
    }
    // Assign a value to a single-valued option, rejecting a second answer (§3.4).
    macro_rules! set_single {
        ($slot:expr, $name:literal, $value:expr) => {{
            if $slot.is_some() {
                fail!(concat!("option '", $name, "' given more than once"));
            } else {
                $slot = Some($value);
            }
        }};
    }

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--" {
            // Accepted and ignored; the convention gives it no other meaning.
        } else if let Some(long) = arg.strip_prefix("--") {
            let (name, attached) = match long.split_once('=') {
                Some((n, v)) => (n, Some(v.to_string())),
                None => (long, None),
            };
            match name {
                "help" => help = true,
                "version" => version = true,
                "force" => force = true,
                "input" | "output" | "format" | "hyphens" | "css" => {
                    let value = match attached {
                        Some(v) => v,
                        None => {
                            i += 1;
                            match args.get(i) {
                                Some(v) => v.clone(),
                                None => {
                                    fail!("option '--{name}' needs a value");
                                    break;
                                }
                            }
                        }
                    };
                    match name {
                        "input" => inputs.push(value),
                        "output" => set_single!(output, "--output", value),
                        "format" => set_single!(format, "--format", value),
                        "hyphens" => set_single!(hyphens, "--hyphens", value),
                        "css" => css.push(value),
                        _ => unreachable!(),
                    }
                }
                _ => fail!("unexpected option '--{name}'"),
            }
        } else if arg.len() > 1 && arg.starts_with('-') {
            // A short cluster: booleans bundle; the first value-taking flag ends
            // it by consuming the remainder of the token (or the next token).
            let chars: Vec<char> = arg[1..].chars().collect();
            let mut j = 0;
            while j < chars.len() {
                match chars[j] {
                    'h' => help = true,
                    'V' => version = true,
                    'f' => force = true,
                    c @ ('i' | 'o') => {
                        let rest: String = chars[j + 1..].iter().collect();
                        let value = if !rest.is_empty() {
                            rest
                        } else {
                            i += 1;
                            match args.get(i) {
                                Some(v) => v.clone(),
                                None => {
                                    fail!("option '-{c}' needs a value");
                                    break;
                                }
                            }
                        };
                        match c {
                            'i' => inputs.push(value),
                            _ => set_single!(output, "--output", value),
                        }
                        break; // the value-taking flag consumed the rest of the cluster
                    }
                    c => {
                        fail!("unexpected option '-{c}'");
                        break;
                    }
                }
                j += 1;
            }
        } else {
            // A bare word: positional inputs are not accepted (§2). Point the
            // user straight at the form that works.
            fail!("unexpected argument '{arg}'; use -i {arg}");
        }
        i += 1;
    }

    // Reject an out-of-set value (§3.5) after the scan, so a `-h` anywhere
    // still short-circuits to help.
    let json = match format.as_deref() {
        None | Some("human") => false,
        Some("json") => true,
        Some(f) => {
            fail!("invalid value '{f}' for --format; supported values: human, json");
            false
        }
    };
    let hyphens = match hyphens.as_deref() {
        None => None,
        Some("auto") => Some(Hyphens::Auto),
        Some("none") => Some(Hyphens::None),
        Some(h) => {
            fail!("invalid value '{h}' for --hyphens; supported values: auto, none");
            None
        }
    };

    // Precedence: help short-circuits even a malformed line; a usage error
    // outranks a version request; version outranks a run; a run needs an input.
    if help {
        return Cli::Help;
    }
    if let Some(msg) = error {
        return Cli::Usage(msg);
    }
    if version {
        return Cli::Version;
    }
    // A transformer takes exactly one input (§2): a second `-i` is a usage
    // error, never a silently-kept last one.
    match inputs.len() {
        0 => Cli::Usage("missing required -i".to_string()),
        1 => Cli::Run(Run {
            input: inputs.remove(0),
            output,
            force,
            hyphens,
            css,
            json,
        }),
        n => Cli::Usage(format!(
            "kepubverto converts one book at a time: expected 1 input, got {n}"
        )),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse(&args) {
        Cli::Help => {
            println!("{HELP}");
            ExitCode::SUCCESS
        }
        Cli::Version => {
            println!("kepubverto {}", kepubverto::VERSION);
            ExitCode::SUCCESS
        }
        Cli::Usage(msg) => {
            // Short stderr message + a pointer to --help; never the full help.
            eprintln!("error: {msg} (see --help)");
            ExitCode::from(2)
        }
        Cli::Run(run) => match execute(&run) {
            Ok(code) => code,
            Err(Stop::Refused(e)) => {
                eprintln!("error: {e}");
                ExitCode::from(2)
            }
            Err(Stop::Input(e)) => {
                eprintln!("error: {e}");
                if run.json {
                    print_envelope(input_error(run.input.clone(), e));
                }
                ExitCode::from(2)
            }
        },
    }
}

/// Why a run stopped with exit `2` (FORMATS §1, as epubsana splits it).
enum Stop {
    /// The tool would not run: output = input, an existing output without
    /// `-f`. No envelope.
    Refused(String),
    /// The input could not be processed: unreadable, not an EPUB, or the
    /// converted book could not be written. Described in the envelope.
    Input(String),
}

/// Convert the one input, report, and return the exit code. Anything that
/// stops the run from writing a book is an `Err`: exit `2`.
fn execute(run: &Run) -> Result<ExitCode, Stop> {
    let input = Path::new(&run.input);
    // Resolve the output and enforce the file-safety rules before any work.
    let out = match &run.output {
        Some(o) => PathBuf::from(o),
        None => default_output(input),
    };
    let bytes = std::fs::read(input)
        .map_err(|e| Stop::Input(format!("cannot read {}: {e}", input.display())))?;
    if same_path(input, &out) {
        return Err(Stop::Refused(format!(
            "output path is the input ({}); refusing to modify the original in place \
             — choose a different -o (-f does not lift this)",
            out.display()
        )));
    }
    if out.exists() && !run.force {
        return Err(Stop::Refused(format!(
            "'{}' exists; use -f to replace it",
            out.display()
        )));
    }

    let options = Options {
        hyphens: run.hyphens,
        css: run.css.clone(),
    };
    let conversion = kepubverto::convert_with(&bytes, &options).map_err(|e| match e {
        // The argument is at fault, not the book: no envelope, as for any
        // other usage error.
        kepubverto::Error::Css(e) => Stop::Refused(e.to_string()),
        e => Stop::Input(format!("cannot convert {}: {e}", run.input)),
    })?;
    std::fs::write(&out, &conversion.epub)
        .map_err(|e| Stop::Input(format!("cannot write {}: {e}", out.display())))?;

    if run.json {
        print_envelope(input_converted(&conversion, &run.input, &out));
    } else {
        for (path, reason) in conversion.left_alone() {
            eprintln!("warning: {path} was left as it was: {reason}");
        }
        print_report(&conversion, &run.input, &out);
    }
    Ok(if conversion.complete() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn print_report(c: &Conversion, input: &str, out: &Path) {
    let count = |f: fn(&Outcome) -> bool| c.documents.iter().filter(|d| f(&d.outcome)).count();
    let converted = count(|o| matches!(o, Outcome::Converted { .. }));
    let already = count(|o| matches!(o, Outcome::AlreadyConverted));
    let left = count(|o| matches!(o, Outcome::LeftAlone(_)));
    let mut line = format!(
        "{input}: {converted} of {} content documents converted, {} spans added",
        c.documents.len(),
        c.spans()
    );
    if already > 0 {
        line.push_str(&format!(", {already} already converted"));
    }
    if left > 0 {
        line.push_str(&format!(", {left} left as they were"));
    }
    println!("{line}");
    println!("wrote {}", out.display());
}

/// kepubverto's per-input `summary` (FORMATS §2, tool-owned): how many
/// content documents came to each outcome, and the spans added. No severity
/// counters: the items are findings only, and their severities are in them.
#[derive(Serialize)]
struct Summary {
    documents: usize,
    converted: usize,
    already_converted: usize,
    nothing_to_do: usize,
    left_alone: usize,
    spans: usize,
}

/// No tool-owned item `data` yet.
type Data = ();

/// The input object for a converted book: one `finding` per content document
/// left as it was (FORMATS §1.3, "A part a transformer leaves untouched"),
/// at the code and severity [`kepubverto::Reason::code`] gives.
fn input_converted(c: &Conversion, input: &str, out: &Path) -> Input<Summary, Data> {
    let count = |f: fn(&Outcome) -> bool| c.documents.iter().filter(|d| f(&d.outcome)).count();
    let summary = Summary {
        documents: c.documents.len(),
        converted: count(|o| matches!(o, Outcome::Converted { .. })),
        already_converted: count(|o| matches!(o, Outcome::AlreadyConverted)),
        nothing_to_do: count(|o| matches!(o, Outcome::NothingToDo)),
        left_alone: count(|o| matches!(o, Outcome::LeftAlone(_))),
        spans: c.spans(),
    };
    let items = c
        .documents
        .iter()
        .filter_map(|d| match &d.outcome {
            Outcome::LeftAlone(reason) => {
                let (code, severity) = reason.code(&d.media_type);
                Some(Item::finding(
                    code.to_string(),
                    None,
                    severity.as_str(),
                    Some(d.path.clone()),
                    None,
                    format!("Left as it was: {reason}"),
                    None,
                ))
            }
            _ => None,
        })
        .collect();
    Input::transformed(
        input.to_string(),
        out.display().to_string(),
        Some(summary),
        items,
    )
}

/// The input object for a book that could not be read or written: no
/// verdict, so no summary (FORMATS §1.2, §1.4).
fn input_error(path: String, error: String) -> Input<Summary, Data> {
    Input {
        path,
        status: "error",
        error: Some(error),
        output: None,
        summary: None,
        items: Vec::new(),
    }
}

fn print_envelope(input: Input<Summary, Data>) {
    let env = Envelope::for_tool(
        "kepubverto",
        kepubverto::VERSION,
        CONVENTION,
        None,
        vec![input],
    );
    // Serializing owned strings and integers cannot fail.
    println!(
        "{}",
        serde_json::to_string_pretty(&env).expect("the envelope serializes")
    );
}

/// Default output: `<input-stem>.kepub.epub`, beside the input (§4: an
/// extension-changing transformer, so no `_<verb>`).
fn default_output(input: &Path) -> PathBuf {
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("book");
    input.with_file_name(format!("{stem}.kepub.epub"))
}

/// Whether `output` resolves to the same file as `input` (so we never overwrite
/// the original). Handles the output not existing yet by resolving its parent.
fn same_path(input: &Path, output: &Path) -> bool {
    let Ok(ci) = std::fs::canonicalize(input) else {
        return false;
    };
    if let Ok(co) = std::fs::canonicalize(output) {
        return ci == co;
    }
    match (output.parent(), output.file_name()) {
        (Some(parent), Some(name)) => {
            let parent = if parent.as_os_str().is_empty() {
                Path::new(".")
            } else {
                parent
            };
            std::fs::canonicalize(parent)
                .map(|cp| cp.join(name) == ci)
                .unwrap_or(false)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(argv: &[&str]) -> Cli {
        parse(&argv.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    fn run_of(argv: &[&str]) -> Run {
        match parse_str(argv) {
            Cli::Run(run) => run,
            other => panic!("expected Run, got {other:?}"),
        }
    }

    #[test]
    fn bare_invocation_is_missing_input_not_help() {
        assert_eq!(parse_str(&[]), Cli::Usage("missing required -i".into()));
    }

    #[test]
    fn positional_is_rejected_with_a_migration_hint() {
        assert_eq!(
            parse_str(&["book.epub"]),
            Cli::Usage("unexpected argument 'book.epub'; use -i book.epub".into())
        );
    }

    #[test]
    fn input_forms_all_name_the_same_file() {
        for argv in [
            vec!["-i", "book.epub"],
            vec!["--input", "book.epub"],
            vec!["--input=book.epub"],
            vec!["-ibook.epub"],
        ] {
            assert_eq!(run_of(&argv).input, "book.epub");
        }
    }

    #[test]
    fn a_second_input_is_a_usage_error_not_a_silent_last_wins() {
        assert_eq!(
            parse_str(&["-i", "a.epub", "-i", "b.epub"]),
            Cli::Usage("kepubverto converts one book at a time: expected 1 input, got 2".into())
        );
    }

    #[test]
    fn a_value_token_is_never_reparsed_as_an_option() {
        assert_eq!(run_of(&["-i", "-q.epub"]).input, "-q.epub");
    }

    #[test]
    fn bundled_value_flag_takes_the_remainder_posix() {
        assert_eq!(run_of(&["-fia.epub"]).input, "a.epub");
        assert!(run_of(&["-fia.epub"]).force);
    }

    #[test]
    fn repeated_single_valued_option_is_an_error() {
        assert_eq!(
            parse_str(&["-i", "a.epub", "-o", "x.epub", "-o", "y.epub"]),
            Cli::Usage("option '--output' given more than once".into())
        );
    }

    #[test]
    fn repeated_boolean_is_not_an_error() {
        assert!(run_of(&["-i", "a.epub", "-f", "--force", "-f"]).force);
    }

    #[test]
    fn unknown_option_is_a_usage_error() {
        assert_eq!(
            parse_str(&["-x", "-i", "a.epub"]),
            Cli::Usage("unexpected option '-x'".into())
        );
        // kepubify's in-place flag has no equivalent here.
        assert_eq!(
            parse_str(&["--inplace", "-i", "a.epub"]),
            Cli::Usage("unexpected option '--inplace'".into())
        );
    }

    #[test]
    fn format_takes_human_and_json() {
        assert!(!run_of(&["-i", "a.epub"]).json);
        assert!(!run_of(&["-i", "a.epub", "--format=human"]).json);
        assert!(run_of(&["-i", "a.epub", "--format", "json"]).json);
        assert_eq!(
            parse_str(&["-i", "a.epub", "--format", "JSON"]),
            Cli::Usage("invalid value 'JSON' for --format; supported values: human, json".into())
        );
    }

    #[test]
    fn hyphens_takes_the_css_values() {
        assert_eq!(run_of(&["-i", "a.epub"]).hyphens, None);
        assert_eq!(
            run_of(&["-i", "a.epub", "--hyphens", "auto"]).hyphens,
            Some(Hyphens::Auto)
        );
        assert_eq!(
            run_of(&["-i", "a.epub", "--hyphens=none"]).hyphens,
            Some(Hyphens::None)
        );
        // Values are lowercase and matched exactly (§3.5).
        assert_eq!(
            parse_str(&["-i", "a.epub", "--hyphens", "Auto"]),
            Cli::Usage("invalid value 'Auto' for --hyphens; supported values: auto, none".into())
        );
        // kepubify's flag pair is not accepted.
        assert_eq!(
            parse_str(&["-i", "a.epub", "--hyphenate"]),
            Cli::Usage("unexpected option '--hyphenate'".into())
        );
    }

    #[test]
    fn hyphens_twice_is_two_answers_to_one_question() {
        assert_eq!(
            parse_str(&["-i", "a.epub", "--hyphens", "auto", "--hyphens", "none"]),
            Cli::Usage("option '--hyphens' given more than once".into())
        );
        assert_eq!(
            parse_str(&["-i", "a.epub", "--hyphens"]),
            Cli::Usage("option '--hyphens' needs a value".into())
        );
    }

    #[test]
    fn css_can_be_given_more_than_once() {
        assert_eq!(run_of(&["-i", "a.epub"]).css, Vec::<String>::new());
        assert_eq!(
            run_of(&["-i", "a.epub", "--css", "p {}", "--css=h1 {}"]).css,
            vec!["p {}".to_string(), "h1 {}".to_string()]
        );
        assert_eq!(
            parse_str(&["-i", "a.epub", "--css"]),
            Cli::Usage("option '--css' needs a value".into())
        );
        // kepubify's short flag is not accepted (§3.2).
        assert_eq!(
            parse_str(&["-i", "a.epub", "-c", "p {}"]),
            Cli::Usage("unexpected option '-c'".into())
        );
    }

    #[test]
    fn help_short_circuits_even_a_malformed_line() {
        assert_eq!(parse_str(&["--bogus", "-h"]), Cli::Help);
        assert_eq!(parse_str(&["-hV"]), Cli::Help);
    }

    #[test]
    fn version_is_recognized_and_needs_no_input() {
        assert_eq!(parse_str(&["-V"]), Cli::Version);
        assert_eq!(parse_str(&["--version"]), Cli::Version);
    }

    #[test]
    fn default_output_changes_the_extension() {
        assert_eq!(
            default_output(Path::new("/books/Aylak Adam.epub")),
            PathBuf::from("/books/Aylak Adam.kepub.epub")
        );
        assert_eq!(
            default_output(Path::new("book")),
            PathBuf::from("book.kepub.epub")
        );
    }
}
