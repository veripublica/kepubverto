# kepubverto

**Converts EPUB to KEPUB for Kobo e-readers**, in pure Rust, without making
the book worse.

A KEPUB is an EPUB with extra markup that lets a Kobo reader track reading
position, page through a chapter quickly and show reading statistics.
kepubverto adds that markup and nothing else. A book that was valid before the
conversion is still valid after it.

> Part of the **veripublica** family: `epubveri` (verify), `epubsana` (heal),
> `kepubverto` (convert).

## Status

Not released yet. The conversion works and is tested; the web page, the
release binaries and `--format json` are still to come.

What the conversion does is written down in [docs/SPANS.md](docs/SPANS.md),
rule by rule. In short, in each XHTML content document it:

- wraps sentences and images in `<span class="koboSpan" id="kobo.P.S">`,
  numbered exactly as kepubify numbers them, so reading positions and
  highlights survive a switch between the two tools;
- wraps the body in `<div id="book-columns"><div id="book-inner">`;
- adds one `<style>` rule to the head.

## Why another converter

[kepubify](https://github.com/pgaskin/kepubify) is the converter most Kobo
users know, and kepubverto is measured against it. kepubify reads XHTML with
an HTML parser and writes every document out again. That breaks books in ways
a reader does not always show:

- An element written as `<div/>` or `<pre/>` is valid XHTML, but an HTML
  parser treats it as left open, and the rest of the chapter ends up inside
  it. 122 of the 544 books on our test shelf contain one.
- Spans land where they are not allowed (inside `<option>`, `<picture>`, or
  directly in an EPUB 2 `<blockquote>`). On the test shelf this added 3,994
  validation errors to two books.
- A book converted twice gets the style rule twice.
- DOCTYPEs, entity references, line endings and whitespace are rewritten.

kepubverto reads documents as XML, as the EPUB specification requires, and
inserts its markup at byte offsets. Every other byte of a document stays as it
was, and files it does not convert are copied from the archive unchanged.

On the 544-book test shelf, the conversion added **no** validation findings
(graded by [epubveri](https://github.com/veripublica/epubveri)), and the spans
are identical to kepubify's in 525 books. In the rest, kepubify misreads an
invalid document, or kepubverto leaves out a span that would be invalid while
keeping kepubify's ids for all the others. [docs/SPANS.md](docs/SPANS.md) has
the details.

## Install

Not on crates.io yet. To build from source, with a Rust toolchain:

```sh
cargo install --locked --git https://github.com/veripublica/kepubverto kepubverto
```

## Usage

```sh
# Convert, writing book.kepub.epub beside book.epub:
kepubverto -i book.epub

# Choose the output, and replace it if it exists:
kepubverto -i book.epub -o out/book.kepub.epub -f

# A whole folder, one book at a time:
find . -name '*.epub' ! -name '*.kepub.epub' -print0 | xargs -0 -n1 kepubverto -i
```

kepubverto converts; it does not validate. To check a book first, run
`epubveri -i book.epub`, and to repair it, `epubsana -i book.epub`.

Exit codes: `0` every content document was converted; `1` the book was
written, but some documents could not be converted and were left as they
were (each is named on stderr); `2` kepubverto could not run.

The CLI conforms to the [veripublica conventions
v0.6](https://github.com/veripublica/conventions): input by `-i` only, never
a positional path; output never in place; an existing file is replaced only
with `-f`.

### Coming from kepubify

| kepubify | kepubverto |
|---|---|
| `kepubify book.epub` | `kepubverto -i book.epub` |
| `-o, --output PATH` | `-o, --output PATH` (a file, not a directory) |
| several inputs, or a directory | one `-i` per run; let the shell loop (above) |
| `-i, --inplace` (no `_converted` suffix) | not needed: the output is always `<name>.kepub.epub` |
| `-u, --update` (do not overwrite) | the default: an existing output is replaced only with `-f` |
| `--calibre` (`.kepub` extension) | `-o book.kepub` |
| `--no-preserve-dirs`, `-x, --copy` | not needed: one book per run |
| `--add-dummy-titlepage`, `--no-add-dummy-titlepage` | never adds one (kepubify adds one when a heuristic says so) |
| `--smarten-punctuation`, `-c, --css`, `--hyphenate`, `--no-hyphenate`, `--fullscreen-reading-fixes`, `-r, --replace`, `--charset` | not yet; each will come in its own release |
| `-v, --verbose` | not yet |
| `--version` | `-V, --version` |

## Library

```rust
let book = std::fs::read("book.epub")?;
let conversion = kepubverto::convert(&book)?;
for (path, reason) in conversion.left_alone() {
    eprintln!("{path} was left as it was: {reason}");
}
std::fs::write("book.kepub.epub", &conversion.epub)?;
```

`kepubverto-wasm` exposes the same function to JavaScript for the browser.

## License

Dual-licensed: **AGPL-3.0-only** OR a **commercial license**; see
[`LICENSE`](./LICENSE) and [`LICENSE-COMMERCIAL.md`](./LICENSE-COMMERCIAL.md).

**Using this tool, or building something that calls it? Read
[`LICENSING.md`](./LICENSING.md).** Short version: the books you convert are
yours unconditionally, commercial use of the tool needs no commercial license,
and the commercial license is only for embedding this code in a closed-source
product or serving a modified version over a network.

kepubverto contains no kepubify code. kepubify is used only as a reference,
by running it and comparing outputs.
