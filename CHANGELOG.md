# Changelog

All notable changes to `kepubverto` (and the `kepubverto-wasm` bindings, which
track the same version) are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
kepubverto is pre-1.0, so breaking changes land as minor-version bumps
(`0.x.0`), per [Cargo's SemVer compatibility
rules](https://doc.rust-lang.org/cargo/reference/semver.html).

## [0.5.0] - 2026-11-11

### Fixed

- An `img` with no `alt` that is alone in a `figure` with a caption is no
  longer wrapped in a span (#2). HTML lets such an image leave out `alt`,
  and the span ended that exception, so a valid EPUB 3 book gained an
  RSC-005 error for every such image. The image is counted and its span left
  out, so every other span keeps kepubify's id. epubveri reports this from
  0.39.0 on; EPUBCheck 5.4.0 does not.
- On the 562-book shelf, graded by epubveri 0.40.1 with no option and with
  `--hyphens auto` plus two `--css` values: findings added 0 at every
  severity. The one shelf book with such a figure is EPUB 2, where its image
  was already an error; the image now has no span there either.

### Changed

- **Breaking:** `Conversion` has a new public field, `epub3`. Code that
  builds a `Conversion` itself must set it.
- New `Reason::code_for(media_type, epub3)`. A UTF-16 document in an EPUB 2
  book now gets `KEPUB-006` (error) in the `--format json` report and on
  the web page, not `HTM_058`: OCF 2.0.1 § 1.3 allows UTF-16 there.
  `Reason::code` is unchanged and gives EPUB 3's grade.
- epubveri 0.40.1 (was 0.37.1). Its CSS changes since 0.37.1 do not change
  which `--css` values are accepted: `@charset` labels are read only from a
  file's bytes, and a `url()` nested in another function was already
  refused, like every `url()`.

### Security

- zip 9.5.1 (was 9.3.0), with the family (R19). Its changes are in writing
  and extraction, which kepubverto does not use.

## [0.4.0] - 2026-11-04

### Security

- zip 9.3.0 (was 9.1.3), for reader fixes merged after 9.1.3 that no advisory
  listed yet on 2026-11-03. kepubverto opens untrusted files with it. On the
  562-book shelf, no book that opened before is refused now.
- roxmltree 0.21.2 (was 0.21.1), which fixes a stack overflow on deeply nested
  elements. `xmlguard` already stopped such a document before the parse.

### Changed

- epubveri 0.37.1 (was 0.35.0) and styloria 0.19.0 (was 0.17.0). This changes
  which `--css` values are accepted, and only that:
  - A value its property cannot take is now refused, such as `p { font-style:
    bold }` or `p { width: 150 }` (epubveri's ADV-024).
  - Names the CSS specifications define are no longer refused as unknown, such
    as `size`.
  - In an EPUB 2 book, CSS2's own names are accepted, such as `marker-offset`.
- With no option and with `--hyphens auto` plus two `--css` values, the converted
  book, stderr and the exit code are byte for byte the previous build's on the
  562-book shelf. Findings added: 0 at every severity.

## [0.3.1] - 2026-10-29

### Changed

- The container is written by kepubverto itself (`src/zipwrite.rs`, taken
  from epubsana 0.36.0), not by `zip`. An entry the conversion did not change
  is copied verbatim: local header, data, data descriptor and central record,
  only the local-header offset moved. Before, its extra fields were dropped
  from both headers. A converted document keeps every header field of the
  original (time, attributes, extra fields, entry comment) but CRC, sizes and
  the data-descriptor flag. The archive comment is kept. On the 562-book
  shelf, every entry's contents are byte for byte 0.3.0's, and every entry
  left unchanged is byte for byte the input's.

- epubveri 0.35.0 (was 0.33.0) and styloria 0.17.0 (was 0.16.1). For
  `--css`, a selector that fails only on a code point the CSS Syntax draft
  may take out of names (`.‣ {}`, w3c/csswg-drafts#7129, under review) no
  longer draws epubveri's CSS-008, so such a value is accepted; epubcheck
  accepts it too. A value invalid in both readings is still refused.

### Security

- zip 9.1.3 (was 9.0.4), the floor raised with it, for GHSA-m665-v7pm-7grj,
  GHSA-gmfj-vffv-32pw and GHSA-wjr6-mhvj-cjvm. zip now only reads the
  container. epubveri 0.35.0 found that the last one's fallback to an
  earlier end record is still so in 9.1.3; kepubverto does not validate,
  and such a book is epubveri's to refuse before conversion.

## [0.3.0] - 2026-10-23

### Added

- `--css <CSS>` (library: `Options::css`), repeatable: adds the user's CSS
  to every converted document, in one `<style>` after the style rule and
  the hyphenation rule, in the form the EPUB version allows. The values
  are written back by styloria's `serialize_stylesheet_for_xml`, so the
  element holds no `<`, `&` or `]]>`. A value is refused (exit 2, nothing
  written; `Error::Css` in the library) when it cannot be written that way,
  refers to a file, has `@font-face`, holds no CSS, or would add a finding:
  it is checked with epubveri's `check_style_text` by the book's EPUB
  version, advisory checks included. kepubify's `-c` is not accepted.
  docs/SPANS.md, "Extra CSS".
- kepubverto-wasm: `convert(epub, hyphens, css)` takes the `--css` values
  as a third, optional argument; a refused value throws `extra CSS: …`.
  The web page has an "Extra CSS" box.
- `CssReason` displays the reason alone, without the value number.

### Changed

- epubveri 0.33.0 (was 0.31.0), for `check_style_text`; styloria 0.16.1
  is now a direct dependency, at epubveri's own requirement.
- `content::convert` takes the extra CSS as a fourth argument.

## [0.2.3] - 2026-10-21

### Changed

- epubveri 0.31.0 (was 0.30.0), which moves to styloria 0.15. kepubverto
  uses none of what changed (stylesheet decoding and `url()` scans in
  `css`, a new `NEXT-018` id): on all 562 shelf books, with no option and
  with `--hyphens auto` and `none`, the converted book, the JSON report,
  stderr and the exit code are byte for byte those of 0.2.2.

## [0.2.2] - 2026-10-18

### Changed

- epubveri 0.30.0 (was 0.29.1), which moves to styloria 0.13. kepubverto
  uses none of what changed (selector checks in `css`, a new `ADV-017`
  id): on all 562 shelf books the converted book, the JSON report, stderr
  and the exit code are byte for byte those of 0.2.1. zip stays at 9.0.4,
  the version epubveri and epubsana use.

## [0.2.1] - 2026-10-17

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
