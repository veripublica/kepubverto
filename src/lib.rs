//! Converts an EPUB to a KEPUB for Kobo e-readers.
//!
//! The conversion wraps text and images in `koboSpan` elements, wraps each
//! body in two divs and adds one style rule (docs/SPANS.md). On request it
//! also sets hyphenation on or off ([`Options::hyphens`]) and adds the
//! caller's CSS ([`Options::css`]). Span ids match
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
mod css;
mod segment;
mod zipwrite;

use std::collections::BTreeMap;

pub use content::{Hyphens, Outcome, Reason, codes};
pub use css::{CssError, CssFinding, CssReason};

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
    /// An [`Options::css`] value was refused. Nothing was converted.
    Css(CssError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotAnEpub(why) | Error::TooLarge(why) => f.write_str(why),
            Error::Unreadable(name, why) => write!(f, "cannot read {name}: {why}"),
            Error::Write(why) => write!(f, "cannot write the converted book: {why}"),
            Error::Css(e) => e.fmt(f),
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
    /// The manifest's media type, lowercased: `application/xhtml+xml` or
    /// `text/html`. [`Reason::code`] takes it.
    pub media_type: String,
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

/// What a conversion adds beyond the spans, the divs and the style rule. The
/// default adds nothing more.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Options {
    /// Append a rule that sets hyphenation on or off in every converted
    /// document (the CLI's `--hyphens`). `None` leaves it to the book.
    pub hyphens: Option<Hyphens>,
    /// CSS to append to every converted document, in this order, in one
    /// `<style>` after the others (the CLI's `--css`; docs/SPANS.md, "Extra
    /// CSS"). A value that cannot be written safely, refers to a file, has
    /// `@font-face` or would add an epubveri finding is refused with
    /// [`Error::Css`] before any document is converted.
    pub css: Vec<String>,
}

/// Convert an EPUB with the default [`Options`]. A content document that
/// cannot be converted is left as it was and reported in
/// [`Conversion::documents`]; only a book that cannot be read at all is an
/// [`Error`].
pub fn convert(epub: &[u8]) -> Result<Conversion, Error> {
    convert_with(epub, &Options::default())
}

/// Convert an EPUB with `options`, as [`convert`] does otherwise.
///
/// ```no_run
/// let book = std::fs::read("book.epub")?;
/// let options = kepubverto::Options {
///     hyphens: Some(kepubverto::Hyphens::Auto),
///     css: vec!["p { text-indent: 1.5em }".to_string()],
/// };
/// let conversion = kepubverto::convert_with(&book, &options)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn convert_with(epub: &[u8], options: &Options) -> Result<Conversion, Error> {
    let mut c = container::Container::open(epub)?;
    let package = container::package(&mut c)?;
    // The values are checked by the book's version's rules, so only now.
    let css = if options.css.is_empty() {
        None
    } else {
        Some(css::prepare(&options.css, package.epub3).map_err(Error::Css)?)
    };
    let mut changed = BTreeMap::new();
    let mut documents = Vec::with_capacity(package.documents.len());
    for (path, media_type) in package.documents {
        let outcome = if !c.has(&path) {
            Outcome::LeftAlone(Reason::Missing)
        } else {
            match c.read(&path) {
                Ok(Some(bytes)) => {
                    let (outcome, out) =
                        content::convert(&bytes, package.epub3, options.hyphens, css.as_deref());
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
        documents.push(Document {
            path,
            media_type,
            outcome,
        });
    }
    let epub = c.write(&changed)?;
    Ok(Conversion { epub, documents })
}
