//! WebAssembly bindings for [`kepubverto`]: convert an EPUB to a KEPUB in the
//! browser. The bytes never leave the page.
//!
//! ```js
//! import init, { convert } from "kepubverto-wasm";
//! await init();
//! const r = convert(new Uint8Array(await file.arrayBuffer()));
//! r.documents.filter((d) => d.outcome === "left_alone"); // and why, in d.reason
//! const kepub = r.epub; // Uint8Array → download <name>.kepub.epub
//! convert(bytes, "auto"); // the CLI's --hyphens auto ("none" turns it off)
//! ```

use serde::Serialize;
use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;

/// The crate version, with the git hash it was built from.
#[wasm_bindgen]
pub fn version() -> String {
    kepubverto::VERSION.to_string()
}

/// One content document and what became of it.
#[derive(Serialize, Tsify)]
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
    /// For a document left alone: the code the CLI's `--format json` gives
    /// it (`"RSC-016"`, `"KEPUB-003"`, …), epubcheck's where it has one.
    #[tsify(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// For a document left alone: that code's severity, `"fatal"` or
    /// `"error"`.
    #[tsify(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
}

#[derive(Serialize, Tsify)]
pub struct Conversion {
    #[serde(with = "serde_bytes_as_array")]
    #[tsify(type = "Uint8Array")]
    pub epub: Vec<u8>,
    pub documents: Vec<Document>,
    /// Every content document is converted (now, or before this run).
    pub complete: bool,
}

/// Convert an EPUB. `hyphens` mirrors the CLI's `--hyphens`: `"auto"` or
/// `"none"` adds CSS that turns hyphenation on or off; leave it out to let the
/// book decide. Throws on any other value, and when the book cannot be read
/// at all.
#[wasm_bindgen]
pub fn convert(epub: &[u8], hyphens: Option<String>) -> Result<Ts<Conversion>, JsError> {
    Ok(Ts::from_rust(&conversion(epub, hyphens)?)?)
}

// The body of `convert`, kept apart so a native test can read what it returns:
// a `Ts<…>` holds a JavaScript value, which a native test cannot build. The
// documentation is on `convert`, since that is what reaches the `.d.ts`.
fn conversion(epub: &[u8], hyphens: Option<String>) -> Result<Conversion, JsError> {
    let hyphens = match hyphens.as_deref() {
        None => None,
        Some("auto") => Some(kepubverto::Hyphens::Auto),
        Some("none") => Some(kepubverto::Hyphens::None),
        Some(h) => {
            return Err(JsError::new(&format!(
                "invalid value '{h}' for hyphens; supported values: auto, none"
            )));
        }
    };
    let options = kepubverto::Options { hyphens };
    let c = kepubverto::convert_with(epub, &options).map_err(|e| JsError::new(&e.to_string()))?;
    let complete = c.complete();
    let documents = c
        .documents
        .into_iter()
        .map(|d| {
            let (outcome, spans, reason) = match d.outcome {
                kepubverto::Outcome::Converted { spans } => ("converted", spans, None),
                kepubverto::Outcome::AlreadyConverted => ("already_converted", 0, None),
                kepubverto::Outcome::NothingToDo => ("nothing_to_do", 0, None),
                kepubverto::Outcome::LeftAlone(r) => ("left_alone", 0, Some(r)),
            };
            let code = reason.as_ref().map(|r| r.code(&d.media_type));
            Document {
                path: d.path,
                outcome: outcome.to_string(),
                spans,
                reason: reason.map(|r| r.to_string()),
                code: code.map(|(c, _)| c.to_string()),
                severity: code.map(|(_, s)| s.as_str().to_string()),
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
