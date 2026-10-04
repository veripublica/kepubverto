# Licensing, in plain terms

`kepubverto` is dual-licensed: **AGPL-3.0-only** ([`LICENSE`](./LICENSE)) **OR** a
commercial license ([`LICENSE-COMMERCIAL.md`](./LICENSE-COMMERCIAL.md)).

This page answers the questions people actually ask before adopting it. It is
short on purpose. **The full answers — including the GPL-compatibility
reasoning, clause by clause — live in one shared FAQ for the veripublica
tools:**

**→ [Licensing FAQ](https://github.com/veripublica/epubsana/blob/main/LICENSING-FAQ.md)**

There is deliberately only one canonical text, so the projects cannot drift
into saying different things. This page does not repeat its legal reasoning; it
states the few things that are specific to `kepubverto`, plus the commitment
behind all of it.

---

## Are the books I convert covered by your license?

**No. Unconditionally.**

The license governs this software, not the data it processes. Converting an
EPUB with `kepubverto` no more licenses that book than a compiler licenses the
program it compiles.

`kepubverto` does write to your book — that is its job — so it is fair to ask
what it adds. The conversion itself adds structural markup: `koboSpan`
wrappers around sentences, with identifiers derived mechanically from their
position; the `book-columns` / `book-inner` wrapper divs; and one small
`<style>` rule for them. Optional fixes you switch on change your own text or
add CSS you supply. Nothing expressive of ours, and nothing we would have any
claim over even if the license reached the output, which it does not.

Sell the result, ship it to a retailer, load it on your own reader. You owe us
nothing and you need no permission.

## I convert books commercially. Do I need the commercial license?

**No.** Running `kepubverto` — on your own books, on customers' books, inside
your company, as often as you like, for money — needs no commercial license and
no permission. There is no hobbyist/commercial distinction in how you may *use*
it.

## What is the commercial license actually for?

Two narrow cases, both about **distributing or serving this code** rather than
using it:

1. **Embedding `kepubverto` in a closed-source product you distribute** — an
   e-reader, an editor, a retailer's ingestion pipeline — without meeting the
   AGPL's source-disclosure obligations.
2. **Running a modified version as a network service** without publishing your
   modifications.

Nothing else. If you are not doing one of those two things, the AGPL is free
and sufficient.

## Can a Sigil or calibre plugin use `kepubverto`? Can GPL-3.0 software?

Yes to both — but the reasoning is a licence-compatibility argument, and it
belongs in exactly one place.
**See the [Licensing FAQ](https://github.com/veripublica/epubsana/blob/main/LICENSING-FAQ.md)**,
which walks through GPLv3 §13 and AGPLv3 §13 and quotes both.

## What about kepubify?

`kepubverto` builds on [kepubify](https://github.com/pgaskin/kepubify) by
Patrick Gaskin. kepubify is MIT-licensed, which permits exactly this: using its
work in software distributed under other terms, AGPL and commercial alike, as
long as its copyright notice and permission notice travel with it. They do, in
[`LICENSE-kepubify`](./LICENSE-kepubify), under both licenses.

## Contributing

External contributions are not being accepted yet. Selling commercial licenses
requires the copyright holder to hold full copyright, so a CLA has to exist
before any external code can be merged. That mechanism is not built yet.

---

## The commitment, and the disclaimer

I am not a lawyer, and nothing here is legal advice. Where this page and
[`LICENSE`](./LICENSE) could be read differently, **`LICENSE` governs.**

What is *not* hedged is the commitment itself, which I can make plainly as the
copyright holder: **I will not come after users, plugin authors, or editor
projects.** The AGPL is here because work of mine was once closed and
commercialised by someone else and I got nothing back. It is aimed at that, and
at nothing that a person converting their own books is doing.

Questions: baris@kayadelen.com
