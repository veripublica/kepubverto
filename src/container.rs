//! Reading the container and writing the converted one.
//!
//! Only the entries the conversion needs are inflated: `container.xml`, the
//! package document and the content documents. Every other entry, and every
//! content document left unchanged, is copied verbatim: its local header, data,
//! data descriptor and central record, only the local-header offset patched
//! (`src/zipwrite.rs`). A rewritten entry keeps its place and every header
//! field (name, method, timestamp, attributes, extra fields, comment) but CRC,
//! sizes and the data-descriptor flag; a method other than stored becomes
//! deflated. The archive comment is kept.

use std::collections::BTreeMap;
use std::io::{Cursor, Read};

use flate2::read::DeflateDecoder;
use roxmltree::{Document, ParsingOptions};
use zip::{CompressionMethod, ZipArchive, ZipReadOptions};

use crate::Error;
use crate::zipwrite::{Writer, layout};

/// The most entries a container may hold. The largest count on the 544-book
/// test shelf is 583.
pub const MAX_ENTRIES: usize = 10_000;

/// The most one inflated entry may hold: epubveri's and epubsana's per-entry
/// cap. The largest entry on the test shelf is 21.8 MB.
pub const MAX_ENTRY_BYTES: u64 = 64 * 1024 * 1024;

/// The most the entries this tool inflates may hold together. Raw-copied
/// entries are never inflated and do not count.
pub const MAX_INFLATED_BYTES: u64 = 256 * 1024 * 1024;

pub struct Container<'a> {
    src: &'a [u8],
    zip: ZipArchive<Cursor<&'a [u8]>>,
    inflated: u64,
}

impl<'a> Container<'a> {
    pub fn open(bytes: &'a [u8]) -> Result<Self, Error> {
        let zip = ZipArchive::new(Cursor::new(bytes))
            .map_err(|e| Error::NotAnEpub(format!("not a ZIP archive: {e}")))?;
        if zip.len() > MAX_ENTRIES {
            return Err(Error::TooLarge(format!(
                "the container holds {} entries, more than the {MAX_ENTRIES} allowed",
                zip.len()
            )));
        }
        Ok(Container {
            src: bytes,
            zip,
            inflated: 0,
        })
    }

    pub fn has(&self, name: &str) -> bool {
        self.zip.index_for_name(name).is_some()
    }

    /// Inflate one entry, within the caps. `Ok(None)` when there is no such
    /// entry.
    pub fn read(&mut self, name: &str) -> Result<Option<Vec<u8>>, Error> {
        let Some(i) = self.zip.index_for_name(name) else {
            return Ok(None);
        };
        // One byte past the cap, so an entry whose header understates its size
        // is caught by what it actually inflates to.
        let data = self
            .inflate(i, MAX_ENTRY_BYTES + 1)
            .map_err(|e| Error::Unreadable(name.to_string(), e.to_string()))?;
        if data.len() as u64 > MAX_ENTRY_BYTES {
            return Err(Error::TooLarge(format!(
                "{name} inflates past the {} MiB limit for one entry",
                MAX_ENTRY_BYTES / (1024 * 1024)
            )));
        }
        self.inflated += data.len() as u64;
        if self.inflated > MAX_INFLATED_BYTES {
            return Err(Error::TooLarge(format!(
                "the documents to convert inflate past the {} MiB limit",
                MAX_INFLATED_BYTES / (1024 * 1024)
            )));
        }
        Ok(Some(data))
    }

    /// Entry `index`'s contents, at most `limit` bytes of them, read the way
    /// epubveri and epubsana read them. Stored and deflated entries are
    /// inflated here from their raw bytes: zip 9 bounds every read by the size
    /// the header declares (zip-rs/zip2#1022) and checks the CRC, so a book
    /// epubcheck accepts, with one entry longer than its header says or with a
    /// wrong CRC, would fail to convert. Anything else (another method, an
    /// encrypted entry) goes through zip's own reader, which refuses it as
    /// before.
    fn inflate(&mut self, index: usize, limit: u64) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut data = Vec::new();
        let raw = self.zip.by_index_raw(index)?;
        if !raw.encrypted() {
            match raw.compression() {
                CompressionMethod::Stored => {
                    raw.take(limit).read_to_end(&mut data)?;
                    return Ok(data);
                }
                CompressionMethod::Deflated => {
                    DeflateDecoder::new(raw)
                        .take(limit)
                        .read_to_end(&mut data)?;
                    return Ok(data);
                }
                _ => {}
            }
        }
        drop(raw);
        let options = ZipReadOptions::new().ignore_crc32(true);
        self.zip
            .by_index_with_options(index, options)?
            .take(limit)
            .read_to_end(&mut data)?;
        Ok(data)
    }

    /// Write the container again, with `changed` entries replaced.
    pub fn write(self, changed: &BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>, Error> {
        let mut out = Writer::new(self.src);
        for raw in layout(self.src)? {
            match changed.get(&raw.name) {
                Some(data) => out.rewrite(&raw, data)?,
                None => out.copy(&raw),
            }
        }
        out.finish(self.zip.comment())
    }
}

/// What the package document says about the content documents.
pub struct Package {
    pub epub3: bool,
    /// The content documents (`application/xhtml+xml`, and `text/html`), in
    /// manifest order: container path and media type as the manifest gives it.
    pub documents: Vec<(String, String)>,
}

/// Find the package document through `META-INF/container.xml` and read its
/// manifest.
pub fn package(c: &mut Container) -> Result<Package, Error> {
    let xml = c
        .read("META-INF/container.xml")?
        .ok_or_else(|| Error::NotAnEpub("META-INF/container.xml is missing".into()))?;
    let xml = text(&xml, "META-INF/container.xml")?;
    let doc = parse(&xml, "META-INF/container.xml")?;
    let rootfile = doc
        .descendants()
        .filter(|n| n.has_tag_name("rootfile"))
        .find(|n| n.attribute("media-type") == Some("application/oebps-package+xml"))
        .or_else(|| doc.descendants().find(|n| n.has_tag_name("rootfile")))
        .and_then(|n| n.attribute("full-path"))
        .ok_or_else(|| Error::NotAnEpub("container.xml names no package document".into()))?
        .to_string();

    let opf = c
        .read(&rootfile)?
        .ok_or_else(|| Error::NotAnEpub(format!("the package document {rootfile} is missing")))?;
    let opf = text(&opf, &rootfile)?;
    let doc = parse(&opf, &rootfile)?;
    let root = doc.root_element();
    let epub3 = root
        .attribute("version")
        .is_some_and(|v| v.trim().starts_with('3'));
    let base = rootfile.rsplit_once('/').map_or("", |(dir, _)| dir);

    let mut documents: Vec<(String, String)> = Vec::new();
    for item in root.descendants().filter(|n| {
        n.has_tag_name("item") && n.parent().is_some_and(|p| p.has_tag_name("manifest"))
    }) {
        // `text/html` is not a content-document type in either EPUB version,
        // but kepubify converts such items, and so do we when they parse as
        // XML, so that their ids match (docs/SPANS.md).
        let Some(media_type) = item.attribute("media-type").map(str::trim).filter(|m| {
            m.eq_ignore_ascii_case("application/xhtml+xml") || m.eq_ignore_ascii_case("text/html")
        }) else {
            continue;
        };
        let Some(href) = item.attribute("href") else {
            continue;
        };
        let path = resolve(base, &percent_decode(href));
        if !documents.iter().any(|(p, _)| *p == path) {
            documents.push((path, media_type.to_ascii_lowercase()));
        }
    }
    Ok(Package { epub3, documents })
}

fn text(bytes: &[u8], name: &str) -> Result<String, Error> {
    String::from_utf8(bytes.to_vec()).map_err(|_| Error::NotAnEpub(format!("{name} is not UTF-8")))
}

fn parse<'t>(text: &'t str, name: &str) -> Result<Document<'t>, Error> {
    epubveri::xmlguard::check(text).map_err(|r| Error::TooLarge(format!("{name}: {r}")))?;
    let opts = ParsingOptions {
        allow_dtd: true,
        ..ParsingOptions::default()
    };
    Document::parse_with_options(text, opts)
        .map_err(|e| Error::NotAnEpub(format!("{name} is not well-formed XML: {e}")))
}

/// Resolve a manifest href against the package document's directory.
fn resolve(base: &str, href: &str) -> String {
    let href = href.split('#').next().unwrap_or(href);
    let mut parts: Vec<&str> = if href.starts_with('/') {
        Vec::new()
    } else {
        base.split('/').filter(|s| !s.is_empty()).collect()
    };
    for seg in href.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Some(v) = std::str::from_utf8(&b[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hrefs_resolve_against_the_package_directory() {
        assert_eq!(resolve("OEBPS", "Text/c1.xhtml"), "OEBPS/Text/c1.xhtml");
        assert_eq!(resolve("OEBPS/a", "../b/c.xhtml"), "OEBPS/b/c.xhtml");
        assert_eq!(resolve("", "c.xhtml"), "c.xhtml");
        assert_eq!(resolve("OEBPS", "./c.xhtml#x"), "OEBPS/c.xhtml");
    }

    #[test]
    fn percent_escapes_are_decoded() {
        assert_eq!(percent_decode("Chapter%201.xhtml"), "Chapter 1.xhtml");
        assert_eq!(percent_decode("%C3%A7.xhtml"), "ç.xhtml");
        assert_eq!(percent_decode("100%.xhtml"), "100%.xhtml");
        assert_eq!(percent_decode("a%2"), "a%2");
    }
}
