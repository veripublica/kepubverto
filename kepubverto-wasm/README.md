# kepubverto-wasm

WebAssembly bindings for [**kepubverto**](https://github.com/veripublica/kepubverto),
a pure-Rust EPUB to KEPUB converter for Kobo e-readers. Convert an `.epub`
**entirely in the browser** (or any JS runtime): no server, no native
dependencies. **The bytes never leave the page.**

It runs the same conversion as the command line, and the result is byte for
byte what the CLI writes.

## Install

```
npm install @veripublica/kepubverto-wasm
```

## Usage

```js
import init, { convert, version } from "@veripublica/kepubverto-wasm";

await init();
const r = convert(new Uint8Array(await file.arrayBuffer()));
// r.epub:      Uint8Array, the converted book → save as <name>.kepub.epub
// r.complete:  every content document was converted
// r.documents: [{ path, outcome, spans, reason? }]
//   outcome: "converted" | "already_converted" | "nothing_to_do" | "left_alone"
//   reason:  why a document was left as it was, for a person to read
```

`convert` throws when the file cannot be read as an EPUB at all.

## License

**AGPL-3.0-only** OR a **commercial license**; see [`LICENSE`](./LICENSE),
[`LICENSE-COMMERCIAL.md`](./LICENSE-COMMERCIAL.md) and the project's
[LICENSING.md](https://github.com/veripublica/kepubverto/blob/main/LICENSING.md).
