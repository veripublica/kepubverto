# kepubverto

kepubverto converts an EPUB to a KEPUB, for people who read on a Kobo. A KEPUB is an EPUB with some extra markup that helps the reader keep your place, turn pages quickly and show reading statistics. kepubverto adds that markup and leaves the rest of the book as it was, so a valid book stays valid.

I got into e-books after a book I wanted to read failed to display correctly on my Kobo. Later I noticed that some books that passed validation stopped passing once they were converted to KEPUB, and that is why I wrote this.

The easiest way to use it is [the web page](https://veripublica.github.io/kepubverto/). Your book stays in your browser and nothing is uploaded. There is also a [calibre plugin](https://github.com/veripublica/calibre-plugins).

For the command line, download a binary from the [latest release](https://github.com/veripublica/kepubverto/releases/latest), or install it with `cargo install --locked kepubverto`. Then run:

```sh
kepubverto -i book.epub
```

This writes `book.kepub.epub` next to your book and doesn't touch the original.

If you're coming from [kepubify](https://github.com/pgaskin/kepubify), I test against it and number the spans the way version 4 does, so your reading positions and highlights should carry over. None of its code is in here.

kepubverto only converts. To check a book, use [epubveri](https://github.com/veripublica/epubveri), and to repair one, [epubsana](https://github.com/veripublica/epubsana). The options, the JSON report, the library, the npm package and a table of kepubify's flags are in [AGENTS.md](https://github.com/veripublica/kepubverto/blob/main/AGENTS.md).

The books you convert are yours, and you can use the tool commercially for free. The code is AGPL-3.0-only, or under a commercial licence if you want to embed it in a closed-source product or run a modified version as a service. [LICENSING.md](https://github.com/veripublica/kepubverto/blob/main/LICENSING.md) has the details.
