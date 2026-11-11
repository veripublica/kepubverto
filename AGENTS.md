# kepubverto: detail for code agents and maintainers

`README.md` is the short introduction. This file holds the detail and is kept
true at every release. What the conversion does, rule by rule, and every
measurement run are in [`docs/SPANS.md`](docs/SPANS.md). Past changes:
`CHANGELOG.md`. API docs: <https://docs.rs/kepubverto>.

Figures below name the versions they were measured on. Current version: 0.4.0.

## Scope

kepubverto converts an EPUB to a KEPUB for Kobo e-readers. It ships as a CLI
and library (crate `kepubverto`) and a WebAssembly package
(`@veripublica/kepubverto-wasm`), released together under one version. The web
page at <https://veripublica.github.io/kepubverto/> runs the WebAssembly build;
the book never leaves the browser.

In each content document the conversion makes three insertions:

1. `<span class="koboSpan" id="kobo.P.S">` around text runs and images.
2. `<div id="book-columns"><div id="book-inner">…</div></div>` around the
   contents of `<body>`.
3. One `<style>` element at the end of `<head>`.

On request it adds a second `<style>` (`--hyphens`) and a further one (`--css`).
A content document is every manifest item of type `application/xhtml+xml`,
the navigation document included, or `text/html`.

Rules the code follows:

- **Never make a book worse.** A valid input stays valid. "Added findings = 0"
  counts every severity, usage included.
- **Surgical byte edits.** Documents are parsed with roxmltree to find byte
  offsets and our markup is inserted at them. Nothing is re-serialized. Entries
  the conversion does not change are copied verbatim, headers included. A
  converted entry keeps its header fields.
- **Span ids match kepubify 4.x** (`kobo.P.S`), so reading positions and
  highlights survive a switch. Where a span would be invalid, it is counted and
  not written: the spans present are a subset of kepubify's, with the same ids.
  This is verified by running kepubify, not assumed. kepubify makes no promise
  of identical output across its own versions, and has said v5 will not count
  these skipped places (pgaskin/kepubify#119).
- **Fresh code.** No kepubify code, and no translation of it. Behaviour is
  written in `docs/SPANS.md` first and the code from that. kepubify is only a
  black-box test oracle. `LICENSING.md` says so; if that ever changes, its
  notice ships and that sentence changes.
- **Added markup follows the EPUB version's grammar.** For example, an EPUB 2
  `<style>` carries `type` and no `class`.
- **No C dependencies, no `unsafe`.** The core must build for
  `wasm32-unknown-unknown`.

## What it declines to do

- **It does not validate.** Invalid input is neither refused nor reported.
  [epubveri](https://github.com/veripublica/epubveri) does the checking and
  [epubsana](https://github.com/veripublica/epubsana) the repair; plugins and
  the web page run them first. epubveri is a run-time dependency only for the
  shared parts below. Its `validate_bytes` is used in tests alone and must stay
  out of the run-time path.
- **It has no image policy.** WebP and the like belong elsewhere.
- **It has no in-place mode.** The output is never the input.
- **It converts one book per run.** Batch conversion belongs to plugins and
  shell loops.
- **It makes no OPF changes yet.** The EPUB 2 cover fix waits on a device test,
  and must write EPUB 2 syntax in EPUB 2 (`<meta name="cover">`, no
  `properties`), with namespace prefixes taken from the document.
- **It does not yet offer** kepubify's smart punctuation, find and replace,
  fullscreen CSS, character-set rewriting, or leftover-file removal. Each comes
  in its own release, with its own section in `docs/SPANS.md`. It never adds a
  generated title page.

## Install

Pre-built binaries are attached to every
[GitHub Release](https://github.com/veripublica/kepubverto/releases/latest), for
macOS (Intel and Apple Silicon), Windows (x64 and ARM) and Linux (x64 and ARM,
each as a fully static `musl` build plus a dynamically linked `gnu` variant).
They need no toolchain.

```sh
cargo install --locked kepubverto         # the CLI
npm install @veripublica/kepubverto-wasm  # WASM bindings for the browser
```

`--locked` builds with the exact dependency versions the release was tested
against. On Windows, Rust also needs Microsoft's linker, which comes with
[Build Tools for Visual Studio](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
when you tick *Desktop development with C++*.

## CLI

```
kepubverto -i <PATH> [OPTIONS]
```

The CLI follows the family spec, [veripublica conventions
v0.7](https://github.com/veripublica/conventions). It is a transformer: exactly
one `-i`, no positional paths, and no prompts off a TTY.

| Option | Meaning |
| --- | --- |
| `-i, --input <PATH>` | The input. |
| `-o, --output <PATH>` | The output file. Default `<input-stem>.kepub.epub`, beside the input. |
| `-f, --force` | Allow replacing an existing output. Never lifts the output-equals-input refusal. |
| `--hyphens <auto\|none>` | Turn hyphenation on or off in every converted document. `auto` leaves headings, table cells and the book's per-element choices alone; `none` overrides the book. Without it, the book decides. |
| `--css <CSS>` | Add CSS to every converted document, after kepubverto's rules. Repeatable; all values go in one `<style>`. |
| `--format <human\|json>` | Report format. Default `human`. |
| `-V, --version` | Print the version, with the git hash when built from a checkout. |
| `-h, --help` | Print help. |

`--css` is refused (exit 2, nothing written) when a value is empty, cannot be
written safely in XML, refers to a file, has `@font-face`, or draws any finding
from epubveri's `check_style_text` at the book's version. That checks the
argument, not the book. Since epubveri 0.37 that check includes CSS values
(ADV-024), so `p { font-style: bold }` and `p { width: 150 }` are refused.

```sh
kepubverto -i book.epub                 # writes book.kepub.epub
kepubverto -i book.epub --hyphens auto
kepubverto -i book.epub --css 'p { text-indent: 1.5em; margin: 0 }'
kepubverto -i book.epub -o out/book.kepub.epub -f
find . -name '*.epub' ! -name '*.kepub.epub' -print0 | xargs -0 -n1 kepubverto -i
```

### Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Every content document was converted, or already had been. |
| `1` | The book was written, but some documents were left as they were. Each is named on stderr, or as a finding in the JSON report. |
| `2` | kepubverto could not run: a usage error, a refused `--css`, an unreadable EPUB, an output path that is the input, an existing output without `-f`, or an I/O failure. |

### JSON report

`--format json` prints one JSON object on stdout: the family's shared envelope
([FORMATS](https://github.com/veripublica/conventions/blob/main/FORMATS.md)),
with `output` naming the book written. The input's `summary` counts the content
documents by outcome:

```json
"summary": { "documents": 12, "converted": 11, "already_converted": 0,
             "nothing_to_do": 0, "left_alone": 1, "spans": 4210 }
```

Each document left as it was is one `finding`, with its path as `location` and
the reason as `message`. The `code` is epubcheck's where epubcheck grades the
same state as an error or worse, and kepubverto's own otherwise. A document left
alone is always at least an error, so the input is `problems` and the exit code
`1`.

| The document | `code` | `severity` |
| --- | --- | --- |
| is not well-formed XML | `RSC-016` | fatal |
| has no XHTML `<body>` | `RSC-005` | error |
| is named in the manifest but missing from the container | `RSC-001` | error |
| is UTF-16 (`application/xhtml+xml` in EPUB 3) | `HTM_058` | error |
| is UTF-16 (`text/html` in EPUB 3, which epubcheck grades only as a warning, or any item in EPUB 2, where UTF-16 is allowed) | `KEPUB-006` | error |
| is in UCS-4, EBCDIC or another declared encoding | `RSC-028` | error |
| … declared under a name epubcheck's XML parser does not take | `RSC-016` | fatal |
| is not UTF-8, and declares no other encoding | `KEPUB-001` | error |
| is past the XML safety limits | `KEPUB-002` | error |
| already uses an id the conversion would add | `KEPUB-003` | error |
| declares an entity that expands to markup | `KEPUB-004` | error |
| cannot be read out of the container | `KEPUB-005` | error |

A book that cannot be read, or written after converting, is an input with
`status: "error"` and exit `2`. A refusal (the output is the input, or exists
without `-f`) and a usage error print no envelope, only a line on stderr.

### From kepubify

| kepubify | kepubverto |
| --- | --- |
| `kepubify book.epub` | `kepubverto -i book.epub` |
| `-o, --output PATH` | `-o, --output PATH` (a file, not a directory) |
| several inputs, or a directory | one `-i` per run; let the shell loop |
| `-i, --inplace` (no `_converted` suffix) | not needed: the output is always `<name>.kepub.epub` |
| `-u, --update` (do not overwrite) | the default: an existing output is replaced only with `-f` |
| `--calibre` (`.kepub` extension) | `-o book.kepub` |
| `--no-preserve-dirs`, `-x, --copy` | not needed: one book per run |
| `--add-dummy-titlepage`, `--no-add-dummy-titlepage` | never adds one (kepubify adds one when a heuristic says so) |
| `--hyphenate` | `--hyphens auto` |
| `--no-hyphenate` | `--hyphens none` |
| `-c, --css` | `--css` (no short flag), with the refusals above |
| `--smarten-punctuation`, `--fullscreen-reading-fixes`, `-r, --replace`, `--charset` | not yet |
| `-v, --verbose` | not yet |
| `--version` | `-V, --version` |

Option names follow the family's rules and the language the option writes, not
kepubify's spelling. A new long name is checked against the family's registry
first. The short-flag set is closed.

## API

### Rust

```rust
let book = std::fs::read("book.epub")?;
let conversion = kepubverto::convert(&book)?;
for (path, reason) in conversion.left_alone() {
    eprintln!("{path} was left as it was: {reason}");
}
std::fs::write("book.kepub.epub", &conversion.epub)?;
```

- `convert(&[u8])` and `convert_with(&[u8], &Options)` return
  `Result<Conversion, Error>`.
- `Options { hyphens: Option<Hyphens>, css: Vec<String> }` mirrors `--hyphens`
  and `--css`.
- `Conversion` holds `epub`, `documents` (one `Document { path, media_type,
  outcome }` per content document, in manifest order) and `epub3`, plus
  `left_alone()`, `complete()` and `spans()`.
- `Error` is `NotAnEpub`, `TooLarge`, `Unreadable`, `Write` or `Css`. A document
  that cannot be converted is not an error: it is an `Outcome` with a `Reason`.
  `Reason::code_for(media_type, epub3)` gives the code and severity of the
  table above. `Reason::code(media_type)` is EPUB 3's grade.
- `container` and `content` are public modules, and `codes` holds the
  `KEPUB-00N` constants.

### WebAssembly

```js
import init, { convert, version } from "@veripublica/kepubverto-wasm";

await init();
const r = convert(new Uint8Array(await file.arrayBuffer()));
// r.epub, r.complete, r.documents
convert(bytes, "auto");                                  // --hyphens auto
convert(bytes, undefined, ["p { text-indent: 1.5em }"]); // --css
```

The result is byte for byte what the CLI writes. `convert` throws on an
unreadable book, on a `hyphens` value other than `"auto"` or `"none"`, and on a
refused `css` value (the message starts `extra CSS:`).
`kepubverto-wasm/README.md` has the full shape.

## Limits

| Limit | Value | Constant |
| --- | --- | --- |
| Entries in the container | 10,000 | `container::MAX_ENTRIES` |
| One inflated entry | 64 MiB | `container::MAX_ENTRY_BYTES` |
| All inflated entries | 256 MiB | `container::MAX_INFLATED_BYTES` |

Before every roxmltree parse, `epubveri::xmlguard::check` applies the family's
XML limits (nesting depth, entity expansion, attribute and element counts). A
document past them is left alone (`KEPUB-002`).

A document is also left alone when it is not UTF-8, is not well-formed XML, has
no XHTML `<body>`, already uses an id the conversion would add, or declares an
entity that expands to markup. A document with a `koboSpan` already in it counts
as already converted: converting twice changes nothing.

## Measurements

From the test shelf, 562 real books, kepubify 4.0.4 as the oracle. The shelf is
private and never goes to public CI. Latest run: 2026-11-04, graded by epubveri
0.37.1, with no option and with `--hyphens auto` plus two `--css` values. Every
earlier run is in `docs/SPANS.md`.

- Findings added by the conversion, every severity: **0**.
- Books whose spans are identical to kepubify's: **543**. In the other 19, as
  analysed at 0.2.0: 13 have markup an HTML parser rebuilds (a block inside a
  `p`, or a self-closed non-void element), 4 have spans left out where they
  would be invalid, the rest carrying kepubify's ids, and 2 have matching ids
  with text that differs because an HTML parser maps `&#128;`–`&#159;` onto
  Windows-1252.

Converted books have also been read on a real Kobo.

## Where the output differs from kepubify 4.0.4

These come from different goals. kepubify reads through an HTML parser and emits
polyglot XHTML/HTML that both kinds of parser can read, and its author has said
it is deliberately not necessarily DTD-valid (pgaskin/kepubify#118). kepubverto
holds itself to what epubcheck and epubveri accept, and reads documents as XML,
as the EPUB specification requires. Measured differences:

- `<div/>` or `<pre/>` is valid XHTML, but an HTML parser treats it as left open
  and the rest of the chapter ends up inside it. 122 of 544 shelf books had one
  (2026-10-05).
- kepubify wraps text and images wherever it finds them, including where a span
  is not allowed. On the shelf, two books gained 3,994 `RSC-005` errors this way.
  EPUBCheck 5.4.0 now accepts a span in `option`; kepubverto still skips one
  there, which leaves ids unchanged.
- kepubify also wraps an `img` with no `alt` that its `figure`'s caption stands
  in for. The span ends HTML's exception and the image becomes `RSC-005`
  (epubveri 0.39.0 on; EPUBCheck 5.4.0 does not report it). kepubverto counts
  the image and leaves its span out (#2).
- A book converted twice gets kepubify's style rule twice.
- kepubify's `<style class=…>` is not allowed on an XHTML 1.1 `<style>`, so
  epubcheck reports it once per EPUB 2 document. In EPUB 3 it carries an obsolete
  `type`.
- DOCTYPEs, entity references, line endings and whitespace are rewritten.

`pgaskin/kepubify#118` and `#119` track what v5 may change. Re-measure before
quoting any of this about v5.

## Layout

| Path | Holds |
| --- | --- |
| `src/lib.rs` | `convert`, `Options`, `Conversion`, `Error` |
| `src/main.rs` | The CLI, the envelope |
| `src/container.rs` | Reading the zip and the package document; the limits |
| `src/zipwrite.rs` | Writing the container; unchanged entries verbatim |
| `src/content.rs` | One content document: spans, divs, style, outcomes and codes |
| `src/segment.rs` | Sentence segmentation |
| `src/css.rs` | Checking and writing `--css` |
| `tests/convert.rs` | Conversion rules |
| `tests/validity.rs` | Fixtures graded by epubveri before and after |
| `tests/cli.rs` | Exit codes, refusals, output naming |
| `docs/SPANS.md` | The behaviour document, written before the code |
| `kepubverto-wasm/` | The WebAssembly member and the web page (`demo/`) |
| `scripts/` | `bump.sh`, `preflight.sh`, `wait-runs.sh`, `release-verify.sh` |

Dependencies:

- **epubveri:** `xmlguard::check`, `envelope`, `htm::declare_dtd_entities` and
  `htm::xhtml_entity`, `encoding`, `ids`, `check_style_text` and
  `style_text_urls`. These are never a second copy.
- **styloria:** `serialize_stylesheet_for_xml`.
- **zip:** reading only.
- **flate2 (zlib-rs):** raw inflate in `Container::read`.
- **roxmltree:** parsing.

The release-asset layout (`release-binaries.yml`, `SHA256SUMS.txt`) is a contract
with the plugins. The two publish workflows are named in trusted publishing, so
they must not be renamed.

## Build and test

CI runs these on every push to `main` and on pull requests:

```sh
cargo fmt --all --check
cargo build --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
cargo build --lib --target wasm32-unknown-unknown --locked
cargo build -p kepubverto-wasm --target wasm32-unknown-unknown --locked
```

CI also builds against the declared minimum epubveri and compiles at the MSRV,
1.88. `scripts/preflight.sh X.Y.Z` runs all of these before a release, with the
release checks.

Any change to behaviour goes in `docs/SPANS.md` first, and `LICENSING.md` lists
what the conversion adds to a book: update it when the scope grows. A behaviour
change also needs the shelf run before release, with 0 findings added, recorded
in `docs/SPANS.md`.

## Contributing and licence

AGPL-3.0-only OR a commercial licence; `LICENSING.md` explains which applies to
what. External contributions are not accepted yet, because the dual licence
requires one copyright holder.

"Kobo" stays out of the name and branding (a Rakuten trademark); "for Kobo
e-readers" is fine.
