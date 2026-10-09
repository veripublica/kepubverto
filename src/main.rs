//! kepubverto's CLI, following the **veripublica CLI convention v0.6**
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

use kepubverto::{Conversion, Outcome};

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
        --format <FORMAT>   Report format: human (the default).
    -V, --version           Print kepubverto <version> to stdout and exit 0.
    -h, --help              Print this help to stdout and exit 0.

EXAMPLES:
    kepubverto -i book.epub                 # writes book.kepub.epub
    kepubverto -i book.epub -o out.kepub.epub -f

The original is never modified in place, and an existing output file is never
silently replaced (use -f). kepubverto converts; it does not validate. To check
a book first, run epubveri -i book.epub.

EXIT CODES:
    0   every content document was converted (or already had been).
    1   the book was written, but some content documents could not be
        converted and were left as they were; each is named on stderr.
    2   kepubverto could not run: a usage error, an unreadable EPUB, an output
        path that is the input, an existing output file without -f, or an I/O
        failure.

Conforms to veripublica conventions v0.6.";

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
                "input" | "output" | "format" => {
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
    // still short-circuits to help. `json` is reserved but not implemented
    // yet, and a reserved name that is not supported is rejected.
    if let Some(f) = &format
        && f != "human"
    {
        fail!("invalid value '{f}' for --format; supported values: human");
    }

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
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::from(2)
            }
        },
    }
}

/// Convert the one input, report, and return the exit code. Anything that
/// stops the run from writing a book is an `Err`: exit `2`.
fn execute(run: &Run) -> Result<ExitCode, String> {
    let input = Path::new(&run.input);
    // Resolve the output and enforce the file-safety rules before any work.
    let out = match &run.output {
        Some(o) => PathBuf::from(o),
        None => default_output(input),
    };
    let bytes =
        std::fs::read(input).map_err(|e| format!("cannot read {}: {e}", input.display()))?;
    if same_path(input, &out) {
        return Err(format!(
            "output path is the input ({}); refusing to modify the original in place \
             — choose a different -o (-f does not lift this)",
            out.display()
        ));
    }
    if out.exists() && !run.force {
        return Err(format!("'{}' exists; use -f to replace it", out.display()));
    }

    let conversion =
        kepubverto::convert(&bytes).map_err(|e| format!("cannot convert {}: {e}", run.input))?;
    std::fs::write(&out, &conversion.epub)
        .map_err(|e| format!("cannot write {}: {e}", out.display()))?;

    for (path, reason) in conversion.left_alone() {
        eprintln!("warning: {path} was left as it was: {reason}");
    }
    print_report(&conversion, &run.input, &out);
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
    fn json_is_reserved_and_rejected_until_implemented() {
        assert_eq!(
            parse_str(&["-i", "a.epub", "--format", "json"]),
            Cli::Usage("invalid value 'json' for --format; supported values: human".into())
        );
        assert_eq!(run_of(&["-i", "a.epub", "--format=human"]).input, "a.epub");
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
