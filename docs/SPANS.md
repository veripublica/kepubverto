# What the conversion does to a content document

**Status: draft.** This is the behavior kepubverto implements, written before
the code. The code is written from this document. Each rule marked
*(oracle)* is checked by tests that run the reference converter,
[kepubify](https://github.com/pgaskin/kepubify), on the same input and
compare. The aim is compatibility with what kepubify produces, not with how it
is built: kepubverto contains no kepubify code.

Every *(oracle)* rule was checked against kepubify 4.0.4 on synthetic
documents on 2026-10-09. The oracle is run with `--no-add-dummy-titlepage`,
because by default kepubify may add a title page and change the OPF, which
this version does not do.

## Why span ids have to match

Kobo readers record reading position, bookmarks and highlights against the
`id` of the `koboSpan` they fall in (to be confirmed on a device before the
first release). If a book is converted again by a tool
that numbers its spans differently, those records point at the wrong text or
at nothing. kepubify is the converter most Kobo users already have books
from, so kepubverto numbers spans exactly as kepubify does. A user who
switches tools keeps their annotations.

There is one limit. kepubify reads XHTML with an HTML parser, and an HTML
parser does not understand a self-closed non-void element such as `<pre/>`
or `<div/>`. It treats the element as left open, so everything after it moves
inside it. On a Project Gutenberg book this put the rest of the book inside a
`<pre>`. The output gained 273 validation errors and lost its spans, because
`pre` is skipped. 122 of the 544 books on the test shelf contain such an
element. kepubverto reads documents as XML, as the EPUB specification
requires. The promise is therefore: **ids match kepubify's wherever kepubify
reads the document correctly.** Where it does not, kepubverto follows the
document.

## What changes, and what does not

The conversion changes three things in an XHTML content document:

1. It wraps text runs and images in `<span class="koboSpan" id="kobo.P.S">`.
2. It wraps the contents of `<body>` in
   `<div id="book-columns"><div id="book-inner">…</div></div>`.
3. It appends one `<style>` element to `<head>`.

Every byte of the original document that is not one of these insertions is
written out unchanged. kepubverto does not re-serialize documents: the XML
declaration, the DOCTYPE, entity references, namespace prefixes, attribute
order and quoting, comments, CDATA sections and whitespace all stay exactly as
they were. A document that is unchanged by the rules below is not rewritten
at all.

## Which documents are converted

A content document is every manifest item whose media type is
`application/xhtml+xml`, the navigation document included. All other entries
(the OPF, CSS, images, fonts, …) are copied byte for byte.

An EPUB 2 document whose DOCTYPE references the XHTML DTD may use that DTD's
named entities (`&nbsp;`, `&rsquo;`, …). An XML parser does not load the DTD,
so for parsing only, the entities used are declared in the document's
internal subset. The written document never contains these declarations. In
EPUB 3 such entities are an error, and the document is not well-formed.

A content document is **left untouched**, and the report says why, when:

- it is not well-formed XML;
- it fails the family's XML safety limits (nesting depth, entity expansion,
  attribute and element counts);
- it already contains an element whose `class` includes the token `koboSpan`
  (it was converted before; converting twice changes nothing). kepubify adds
  no spans to such a document either *(oracle)*, but it still adds the divs if
  they are missing, and it appends its `<style>` again, so a book converted
  twice by kepubify carries the rule twice. Leaving the document alone
  differs from kepubify only in those two insertions, never in an id;
- it already uses an id that the conversion would add (`book-columns`,
  `book-inner`, or any `kobo.N.N`), since converting would create a
  duplicate id.

## Span numbering

### Walk order

The rules walk the descendants of `<body>` in document order (depth first,
parent before children). Only elements in the XHTML namespace are classified
below. Elements in other namespaces count as "other elements".

Two counters are kept for the whole document: **P** (the paragraph) and
**S** (the segment within it). Both start at 0. There is also a **pending**
flag, initially off.

### Elements

| Element | Effect |
|---|---|
| `script`, `style`, `pre`, `audio`, `video`, `svg`, `math` | Skipped with everything inside it. No spans are added inside, and the counters do not move. *(oracle)* |
| `p`, `ol`, `ul`, `table`, `h1` to `h6` | Sets **pending**, then its contents are walked. *(oracle)* |
| `option`, `textarea` | Skipped like the row above. Their content model is text only, so a span inside them is invalid. **This differs from kepubify**, which wraps them. |
| `img` inside `picture` | Left unwrapped, and the counters do not move. `picture` may not contain a `span`. **Differs from kepubify.** |
| `img` | P increases by 1, S becomes 1, **pending** is cleared, and the `img` is wrapped in a span `kobo.P.1`. *(oracle)* |
| any other element (`div`, `li`, `span`, `a`, `blockquote`, …) | Its contents are walked. The counters are not affected. |

Leaving an element never changes the counters. Text that follows `</p>`
inside the same `div` continues the paragraph that was open, unless another
listed element comes first. This is deliberate: it is how kepubify numbers,
and the ids must match.

**Pending** means "the next span starts a new paragraph". It is applied
lazily, so an empty `<p>` or a `<p>` holding only an image uses no paragraph
number of its own.

### Text

Each text node in the walk is split into **segments** (see
[Sentence segments](#sentence-segments)). For each segment, in order:

- If the segment is whitespace only and the text node's **direct** parent is
  not a `p`, it is written as it was, with no span.
- Otherwise: if **pending** is set, P increases by 1, S becomes 0, and
  **pending** is cleared. Then S increases by 1, and the segment is wrapped in
  a span `kobo.P.S`. *(oracle)*

Whitespace-only text directly inside a `<p>` is wrapped, and so uses up a
paragraph number when **pending** is set. Whitespace-only text directly inside
any other element (`li`, `h1`, `td`, a `span` inside a `p`, …) is not.
Both match kepubify. *(oracle)*

A comment, a CDATA section or a processing instruction ends a text node: the
text on either side of it is segmented and wrapped separately, and its own
content is never wrapped. (An HTML parser reads a CDATA section as a comment,
which is why kepubify behaves this way.) *(oracle)*

Text before the first listed element gets P = 0, so ids `kobo.0.1`,
`kobo.0.2`, ….

### Sentence segments

A text node is cut into segments. Joined back together, the segments
reproduce the text exactly. A cut happens at the start of the first
character that follows all of these, in order:

1. one or more of `.` `!` `?`;
2. optionally **one** closing character: `'` `"` `”` `’` `“` `…`;
3. one or more whitespace characters, where whitespace means only space, tab,
   line feed, carriage return and form feed (not NO-BREAK SPACE);

and when that next character is not itself whitespace. The whitespace stays
at the end of the earlier segment. *(oracle)*

Consequences, each a test:

| Text | Segments |
|---|---|
| `One. Two.` | `One. ` · `Two.` |
| `3.14 is pi.` | one segment (no whitespace after the `.`) |
| `“Stop!” she said.` | `“Stop!” ` · `she said.` |
| `Wait..."' Then` | one segment (two closing characters) |
| `Really?! Yes.` | `Really?! ` · `Yes.` |
| `End.` + NO-BREAK SPACE + `Next` | one segment |
| `End.` + EM SPACE + `Next` | one segment |
| `Mr. Smith. A.B. c` | `Mr. ` · `Smith. ` · `A.B. ` · `c` |
| `A!!! B?? C.?! D` | `A!!! ` · `B?? ` · `C.?! ` · `D` |
| `A… B. C’ D.’ E.“ F.” G` | `A… B. ` · `C’ D.’ ` · `E.“ ` · `F.” ` · `G` |
| `One.` + form feed + `Two` | `One.` + form feed · `Two` |
| `End. ` (at the end of the text) | one segment; never an empty one |

Characters are classified **after** entity and character references are
decoded: `&#46;` is a full stop, `&nbsp;` is not whitespace. The cut itself
is made in the original bytes, so references are never split and never
rewritten. A span boundary never falls inside a reference.

## The wrapper divs

The children of `<body>` are wrapped, unchanged, in
`<div id="book-columns"><div id="book-inner">` … `</div></div>`. This is not
done when `<body>` already has a child `div#book-columns` that has a child
`div#book-inner`. *(oracle)*

## The style rule

Appended as the last child of `<head>`:

```html
<style type="text/css" class="kobostylehacks">div#book-inner { margin-top: 0; margin-bottom: 0;}</style>
```

This text is byte-identical to kepubify 4.0.4's. epubveri 0.21.0 accepts it
in EPUB 2 and EPUB 3 documents alike.

It is not appended when `<head>` already has an element whose `class`
includes `kobostylehacks`. A document kepubify converted earlier but that has
no `koboSpan` (no text at all) would otherwise get the rule twice, which is
what kepubify itself does.

## How "never make a book worse" is kept

kepubverto converts. It does not validate. Checking a book belongs to
[epubveri](https://github.com/veripublica/epubveri), and repairing it to
[epubsana](https://github.com/veripublica/epubsana). The integrations (the
Sigil and calibre plugins, the web page) run those checks before calling
kepubverto. A CLI user who wants them runs `epubveri -i book.epub` first.

The promise that a valid book stays valid therefore rests on how the
conversion is built and tested, not on a check at run time:

- **By construction.** Documents are read as XML, and only the three
  insertions above are made. Every other byte is kept. The rules skip every
  place where a `span` would be invalid, and any document where an added id
  would collide. A document that cannot be read is left as it was.
- **On every commit.** CI converts a fixture set (public-domain books and
  synthetic edge cases) and grades input and output with epubveri. A valid
  input that produces an invalid output fails the build.
- **Before every release.** The whole test shelf (544 real books) is
  converted, and the findings the conversion added must be zero.

A defect found that way is fixed in the rules, for every book, never patched
around in one.

## What kepubverto does not do (in this version)

kepubify also changes things this version leaves alone:

- character-set declarations in content documents;
- the OPF (cover metadata);
- leftover files from other tools (`__MACOSX/`, `.DS_Store`, …);
- optional features: smart punctuation, find and replace, hyphenation and
  fullscreen CSS, extra CSS, a generated title page.

Each may come in a later version, one at a time, with its own section here.

Because kepubify re-serializes every document through an HTML parser, it also
makes changes that are not part of the conversion and that kepubverto never
makes. Seen on the oracle runs: it adds `<tbody>` to tables and
`type="text/javascript"` to scripts, rewrites references (`&#46;` becomes
`.`, `&quot;` becomes `&#34;`, `&nbsp;` becomes `&#160;`), turns carriage
returns into line feeds, expands `<image/>` to `<image></image>`, and moves
the whitespace after `</body>` inside `book-inner`. None of these changes a
span id, so they do not affect compatibility.

## Open questions

Counts are from the 544-book test shelf (2026-10-05). Each question is
settled by the EPUB specification first and checked on a Kobo device second.

1. **Elements that may not contain a `span`:** text inside `option` or
   `textarea`, and an `img` inside `picture`. kepubify wraps these, and the
   result is invalid. The specification wins: kepubverto skips them. The shelf
   has no such element, so ids do not diverge in practice. A device test
   should confirm that the reader does not need spans there.
2. **Ids already in use.** No shelf book uses `book-columns` or `book-inner`.
   One uses `kobo.P.S` ids, and it is already converted (it has `koboSpan`),
   so it is left alone under the rule above. Any other collision leaves the
   document untouched (see [Which documents are converted](#which-documents-are-converted)).
3. **Items declared `text/html`.** 5 books. That media type is not allowed
   for content documents in either EPUB version, so such a book is already
   invalid. kepubify converts them like any other content document
   *(oracle)*. Proposal: do the same when the document is well-formed XML,
   so that ids match for those books. To check: what a Kobo reader does with
   them.

Settled:

- **Navigation document** (75 books, in the spine in 5). kepubify converts it
  whether or not it is in the spine *(oracle)*, and the converted `nav`
  validates (epubveri 0.24.0). kepubverto converts it too, as
  [Which documents are converted](#which-documents-are-converted) says.
