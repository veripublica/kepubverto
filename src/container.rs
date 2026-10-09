//! Reading the container and writing the converted one.
//!
//! Only the entries the conversion needs are inflated: `container.xml`, the
//! package document and the content documents. Every other entry, and every
//! content document left unchanged, is raw-copied: the same compressed bytes,
//! method, timestamp and position. A rewritten entry keeps its name, place,
//! compression method, timestamp and permissions.

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};

use roxmltree::{Document, ParsingOptions};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::Error;

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
        Ok(Container { zip, inflated: 0 })
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
        let mut f = self
            .zip
            .by_index(i)
            .map_err(|e| Error::Unreadable(name.to_string(), e.to_string()))?;
        let mut data = Vec::new();
        // One byte past the cap, so an entry whose header understates its size
        // is caught by what it actually inflates to.
        (&mut f)
            .take(MAX_ENTRY_BYTES + 1)
            .read_to_end(&mut data)
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

    /// Write the container again, with `changed` entries replaced.
    pub fn write(mut self, changed: &BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>, Error> {
        let io = |e: zip::result::ZipError| Error::Write(e.to_string());
        let mut out = ZipWriter::new(Cursor::new(Vec::new()));
        for i in 0..self.zip.len() {
            let f = self.zip.by_index_raw(i).map_err(io)?;
            let Some(data) = changed.get(f.name()) else {
                out.raw_copy_file(f).map_err(io)?;
                continue;
            };
            let method = match f.compression() {
                CompressionMethod::Stored => CompressionMethod::Stored,
                _ => CompressionMethod::Deflated,
            };
            let mut opts = SimpleFileOptions::default().compression_method(method);
            if let Some(t) = f.last_modified() {
                opts = opts.last_modified_time(t);
            }
            if let Some(mode) = f.unix_mode() {
                opts = opts.unix_permissions(mode);
            }
            let name = f.name().to_string();
            drop(f);
            out.start_file(name, opts).map_err(io)?;
            out.write_all(data)
                .map_err(|e| Error::Write(e.to_string()))?;
        }
        Ok(out.finish().map_err(io)?.into_inner())
    }
}

/// What the package document says about the content documents.
pub struct Package {
    pub epub3: bool,
    /// Container paths of the content documents (`application/xhtml+xml`,
    /// and `text/html`), in manifest order.
    pub documents: Vec<String>,
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

    let mut documents: Vec<String> = Vec::new();
    for item in root.descendants().filter(|n| {
        n.has_tag_name("item") && n.parent().is_some_and(|p| p.has_tag_name("manifest"))
    }) {
        // `text/html` is not a content-document type in either EPUB version,
        // but kepubify converts such items, and so do we when they parse as
        // XML, so that their ids match (docs/SPANS.md).
        let is_content = item.attribute("media-type").is_some_and(|m| {
            let m = m.trim();
            m.eq_ignore_ascii_case("application/xhtml+xml") || m.eq_ignore_ascii_case("text/html")
        });
        if !is_content {
            continue;
        }
        let Some(href) = item.attribute("href") else {
            continue;
        };
        let path = resolve(base, &percent_decode(href));
        if !documents.contains(&path) {
            documents.push(path);
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
