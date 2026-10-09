//! WebAssembly bindings for [`kepubverto`]: convert an EPUB to a KEPUB in the
//! browser. The bytes never leave the page.
//!
//! ```js
//! import init, { convert } from "kepubverto-wasm";
//! await init();
//! const r = convert(new Uint8Array(await file.arrayBuffer()));
//! r.documents.filter((d) => d.outcome === "left_alone"); // and why, in d.reason
//! const kepub = r.epub; // Uint8Array → download <name>.kepub.epub
//! ```

use serde::Serialize;
use tsify::Tsify;
use wasm_bindgen::prelude::*;

/// The crate version, with the git hash it was built from.
#[wasm_bindgen]
pub fn version() -> String {
    kepubverto::VERSION.to_string()
}

/// One content document and what became of it.
#[derive(Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct Document {
    pub path: String,
    /// `"converted"`, `"already_converted"`, `"nothing_to_do"` or `"left_alone"`.
    pub outcome: String,
    /// The `koboSpan` elements added (0 unless converted).
    pub spans: usize,
    /// Why a document was left alone, for a person to read.
    #[tsify(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct Conversion {
    #[serde(with = "serde_bytes_as_array")]
    #[tsify(type = "Uint8Array")]
    pub epub: Vec<u8>,
    pub documents: Vec<Document>,
    /// Every content document is converted (now, or before this run).
    pub complete: bool,
}

/// Convert an EPUB. Throws when the book cannot be read at all.
#[wasm_bindgen]
pub fn convert(epub: &[u8]) -> Result<Conversion, JsError> {
    let c = kepubverto::convert(epub).map_err(|e| JsError::new(&e.to_string()))?;
    let complete = c.complete();
    let documents = c
        .documents
        .into_iter()
        .map(|d| {
            let (outcome, spans, reason) = match d.outcome {
                kepubverto::Outcome::Converted { spans } => ("converted", spans, None),
                kepubverto::Outcome::AlreadyConverted => ("already_converted", 0, None),
                kepubverto::Outcome::NothingToDo => ("nothing_to_do", 0, None),
                kepubverto::Outcome::LeftAlone(r) => ("left_alone", 0, Some(r.to_string())),
            };
            Document {
                path: d.path,
                outcome: outcome.to_string(),
                spans,
                reason,
            }
        })
        .collect();
    Ok(Conversion {
        epub: c.epub,
        documents,
        complete,
    })
}

/// serde-wasm-bindgen turns a `Vec<u8>` into a JS array of numbers; this sends
/// it as bytes, which arrive as a `Uint8Array`.
mod serde_bytes_as_array {
    pub fn serialize<S: serde::Serializer>(v: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bytes(v)
    }
}
