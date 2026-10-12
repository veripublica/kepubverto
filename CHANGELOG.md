# Changelog

All notable changes to `kepubverto` (and the `kepubverto-wasm` bindings, which
track the same version) are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
kepubverto is pre-1.0, so breaking changes land as minor-version bumps
(`0.x.0`), per [Cargo's SemVer compatibility
rules](https://doc.rust-lang.org/cargo/reference/semver.html).

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
