# Changelog

All notable changes to `kepubverto` (and the `kepubverto-wasm` bindings, which
track the same version) are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
kepubverto is pre-1.0, so breaking changes land as minor-version bumps
(`0.x.0`), per [Cargo's SemVer compatibility
rules](https://doc.rust-lang.org/cargo/reference/semver.html).

## [Unreleased]

### Changed

- zip 9 (`>= 9.0.4`), which clears RUSTSEC-2026-0367, -0368 and -0369 (zip
  below 9.0.0; kepubverto never called the affected `extract`). epubveri
  0.29.1, tsify 0.5.8 and the compatible updates. Without options the
  converted entries are the same as 0.2.0's on all 562 shelf books.
- An entry copied unchanged keeps the "made by" system and the file
  attributes the input gives it. zip 8 rewrote both to Unix ones.
- Under zip 9, stored and deflated entries are inflated from their raw
  bytes, as epubveri and epubsana do, and untouched entries are copied
  without extra fields, as zip 8 copied them. So a book with an entry whose
  header understates its size, or with an Info-ZIP Unix extra field
  (0x7875), converts as it did with 0.2.0; zip 9 alone refuses both.

### Fixed

- An entry whose CRC is wrong no longer stops the conversion (exit 2, or a
  document left alone); epubcheck ignores the CRC too.
- `kepubverto-wasm`: `convert` returns its result as `tsify::Ts`, since
  `into_wasm_abi` leaked memory (tsify issue #65). The `.d.ts` types are
  unchanged.

## [0.2.0] - 2026-10-15

### Added

- `--hyphens <auto|none>` adds CSS that turns hyphenation on or off in every
  converted document; without it, the book's own CSS decides. `auto` leaves
  headings, table cells and the book's per-element choices alone; `none`
  overrides the book. It writes `-webkit-hyphens`, which Kobo readers use,
  and the standard `hyphens`, in the `<style>` form each EPUB version allows.
  kepubify's `--hyphenate` / `--no-hyphenate` map to `auto` / `none`
  (docs/SPANS.md, "Hyphenation").
- Library: `convert_with(&epub, &Options)` and `Hyphens`; `convert` is
  `convert_with` with the default options. `content::convert` takes the
  hyphenation setting as a third argument.
- Web page and `kepubverto-wasm`: a hyphenation choice, and
  `convert(epub, hyphens?)`.
- `--format json`: the family's shared envelope (FORMATS, conventions
  v0.7), with `output`, a per-input `summary` of the documents by outcome,
  and one `finding` per document left as it was, coded as epubcheck codes
  the same state where it grades it an error or worse, `KEPUB-001` to
  `KEPUB-006` otherwise (README, "Machine-readable output"). An EPUB that
  cannot be read, or a book that cannot be written, is an `error` input; a
  refusal or a usage error prints no envelope.
- Library: `Reason::code(media_type)` gives that code and severity;
  `Document::media_type`; `Reason::Utf16` and `Reason::Encoding` say which
  encoding a document left alone for its encoding is in, read with
  epubveri's `encoding::sniff`. `content::codes` holds the `KEPUB-` codes.
- `kepubverto-wasm`: a document left as it was carries `code` and
  `severity`, as in the CLI's `--format json`. The web page shows them.

### Changed

- Requires epubveri 0.28 (its `encoding` module, which classifies a
  document's encoding the way epubcheck does, and `Input::transformed`).
- Conforms to veripublica conventions v0.7 (was v0.6): `--help` says so, and
  the envelope's `convention` is `"0.7"`.
- `Reason::NotUtf8` now means only *not UTF-8, with no sign of another
  encoding*; the other cases are `Reason::Utf16` and `Reason::Encoding`.

## [0.1.1] - 2026-10-12

### Fixed

- The style element no longer makes a valid EPUB 2 book invalid. 0.1.0 wrote
  kepubify's `<style type="text/css" class="kobostylehacks">` in every
  document. XHTML 1.1 allows no `class` on `<style>`, so epubcheck reported an
  error (RSC-005) in each converted EPUB 2 document. In EPUB 3 the obsolete
  `type` added a usage finding (RSC-036) per document. The element now
  follows the EPUB version: `<style type="text/css">` in EPUB 2,
  `<style class="kobostylehacks">` in EPUB 3. The rule inside is unchanged.

### Changed

- The validity tests fail on any finding the conversion adds, usage
  included, not only on errors. They grade with epubveri 0.25, the first
  release that reports the EPUB 2 `<style>` errors.

## [0.1.0] - 2026-10-09

### Added

- The conversion: `koboSpan` elements around sentences and images, numbered
  as kepubify 4.0.4 numbers them; the `book-columns`/`book-inner` divs; the
  `kobostylehacks` style rule. The behavior is specified in `docs/SPANS.md`.
- Markup is inserted at byte offsets, so every other byte of a document is
  kept, and entries the conversion does not touch are copied from the archive
  unchanged.
- Where a span would be invalid, it is counted and left out: the book gains
  no errors and every added span keeps kepubify's id.
- Manifest items declared `text/html` are converted too, when they parse as
  XML, as kepubify converts them.
- The CLI (`kepubverto -i book.epub` → `book.kepub.epub`), following the
  veripublica conventions v0.6.
- `kepubverto-wasm`, and a page that converts a book in the browser.
