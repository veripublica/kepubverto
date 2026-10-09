//! Converts an EPUB to a KEPUB for Kobo e-readers.
//!
//! The conversion wraps text and images in `koboSpan` elements, wraps each
//! body in two divs and adds one style rule (docs/SPANS.md). Span ids match
//! kepubify's, so reading positions and highlights survive a switch. Unlike a
//! converter that re-serializes, kepubverto inserts its markup at byte
//! offsets: every other byte of a document stays as it was, and entries the
//! conversion does not touch are copied from the archive unchanged.
//!
//! ```no_run
//! let book = std::fs::read("book.epub")?;
//! let conversion = kepubverto::convert(&book)?;
//! std::fs::write("book.kepub.epub", &conversion.epub)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! kepubverto converts; it does not validate. Checking a book is
//! [epubveri](https://crates.io/crates/epubveri)'s job.

#![forbid(unsafe_code)]

pub mod container;
pub mod content;
mod segment;

use std::collections::BTreeMap;

pub use content::{Outcome, Reason};

/// The crate version, with `+<git hash>[.dirty]` when built from a checkout.
pub const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), env!("KEPUBVERTO_BUILD"));

/// A book could not be converted at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Not a ZIP, or no package document can be found or read.
    NotAnEpub(String),
    /// Past one of the container or XML safety limits.
    TooLarge(String),
    /// An entry the conversion needs could not be inflated.
    Unreadable(String, String),
    /// The converted container could not be written.
    Write(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotAnEpub(why) | Error::TooLarge(why) => f.write_str(why),
            Error::Unreadable(name, why) => write!(f, "cannot read {name}: {why}"),
            Error::Write(why) => write!(f, "cannot write the converted book: {why}"),
        }
    }
}

impl std::error::Error for Error {}

/// The converted book, and what happened to each content document.
#[derive(Debug, Clone)]
pub struct Conversion {
    /// The converted EPUB.
    pub epub: Vec<u8>,
    /// One entry per content document in the manifest (`application/xhtml+xml`
    /// or `text/html`), in manifest order.
    pub documents: Vec<Document>,
}

/// One content document and its outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    /// The path inside the container.
    pub path: String,
    pub outcome: Outcome,
}

impl Conversion {
    /// The documents that could not be converted.
    pub fn left_alone(&self) -> impl Iterator<Item = (&str, &Reason)> {
        self.documents.iter().filter_map(|d| match &d.outcome {
            Outcome::LeftAlone(r) => Some((d.path.as_str(), r)),
            _ => None,
        })
    }

    /// Every content document is converted: now, or before this run.
    pub fn complete(&self) -> bool {
        self.left_alone().next().is_none()
    }

    /// The `koboSpan` elements this run added.
    pub fn spans(&self) -> usize {
        self.documents
            .iter()
            .map(|d| match d.outcome {
                Outcome::Converted { spans } => spans,
                _ => 0,
            })
            .sum()
    }
}

/// Convert an EPUB. A content document that cannot be converted is left as
/// it was and reported in [`Conversion::documents`]; only a book that cannot
/// be read at all is an [`Error`].
pub fn convert(epub: &[u8]) -> Result<Conversion, Error> {
    let mut c = container::Container::open(epub)?;
    let package = container::package(&mut c)?;
    let mut changed = BTreeMap::new();
    let mut documents = Vec::with_capacity(package.documents.len());
    for path in package.documents {
        let outcome = if !c.has(&path) {
            Outcome::LeftAlone(Reason::Missing)
        } else {
            match c.read(&path) {
                Ok(Some(bytes)) => {
                    let (outcome, out) = content::convert(&bytes, package.epub3);
                    if let Some(out) = out {
                        changed.insert(path.clone(), out);
                    }
                    outcome
                }
                Ok(None) => Outcome::LeftAlone(Reason::Missing),
                Err(Error::Unreadable(_, why)) => Outcome::LeftAlone(Reason::Unreadable(why)),
                Err(e) => return Err(e),
            }
        };
        documents.push(Document { path, outcome });
    }
    let epub = c.write(&changed)?;
    Ok(Conversion { epub, documents })
}
