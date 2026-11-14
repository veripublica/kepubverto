//! File names in the table of contents (docs/SPANS.md): where the NCX or the
//! navigation document spells a file differently from the manifest, both are
//! written in one valid spelling. Every book is graded by epubveri before and
//! after: the conversion may take findings away, never add one.

use std::io::{Cursor, Read, Write};

use kepubverto::convert;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const EPUB2_HEAD: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE html PUBLIC \"-//W3C//DTD XHTML 1.1//EN\" \"http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd\">\n<html xmlns=\"http://www.w3.org/1999/xhtml\" xml:lang=\"en\"><head><title>T</title></head>";
const EPUB3_HEAD: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE html>\n<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\" xml:lang=\"en\" lang=\"en\"><head><title>T</title></head>";
const UID: &str = "urn:uuid:0b2cbd9c-5f30-4d5c-9a35-2a7e3b9f3c11";

/// An EPUB 2 book: chapters at `(manifest href, container name)`, and an NCX
/// whose `content/@src` values are `srcs`.
fn epub2(chapters: &[(&str, &str)], srcs: &[&str]) -> Vec<u8> {
    let mut items = String::new();
    let mut spine = String::new();
    for (i, (href, _)) in chapters.iter().enumerate() {
        items.push_str(&format!(
            r#"<item id="c{i}" href="{href}" media-type="application/xhtml+xml"/>"#
        ));
        spine.push_str(&format!(r#"<itemref idref="c{i}"/>"#));
    }
    let opf = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf"><dc:identifier id="id">{UID}</dc:identifier><dc:title>T</dc:title><dc:language>en</dc:language></metadata><manifest>{items}<item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/></manifest><spine toc="ncx">{spine}</spine></package>"#
    );
    let points: String = srcs
        .iter()
        .enumerate()
        .map(|(i, s)| {
            format!(
                r#"<navPoint id="n{i}" playOrder="{}"><navLabel><text>Point {i}</text></navLabel><content src="{s}"/></navPoint>"#,
                i + 1
            )
        })
        .collect();
    let ncx = format!(
        r#"<?xml version="1.0" encoding="utf-8"?><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><head><meta name="dtb:uid" content="{UID}"/><meta name="dtb:depth" content="1"/><meta name="dtb:totalPageCount" content="0"/><meta name="dtb:maxPageNumber" content="0"/></head><docTitle><text>T</text></docTitle><navMap>{points}</navMap></ncx>"#
    );
    let mut files = vec![
        ("OEBPS/content.opf".to_string(), opf),
        ("OEBPS/toc.ncx".to_string(), ncx),
    ];
    for (_, name) in chapters {
        files.push((
            format!("OEBPS/{name}"),
            format!("{EPUB2_HEAD}<body><p id=\"x\">One. Two.</p></body></html>\n"),
        ));
    }
    zip(&files)
}

/// An EPUB 3 book: one chapter at `(manifest href, container name)` and a
/// navigation document in `OEBPS/Text/` whose toc link is `link`.
fn epub3(chapter: (&str, &str), link: &str) -> Vec<u8> {
    let (href, name) = chapter;
    let opf = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">{UID}</dc:identifier><dc:title>T</dc:title><dc:language>en</dc:language><meta property="dcterms:modified">2026-01-01T00:00:00Z</meta></metadata><manifest><item id="c0" href="{href}" media-type="application/xhtml+xml"/><item id="nav" href="Text/nav.xhtml" media-type="application/xhtml+xml" properties="nav"/></manifest><spine><itemref idref="c0"/></spine></package>"#
    );
    let nav = format!(
        r#"{EPUB3_HEAD}<body><nav epub:type="toc"><h1>Contents</h1><ol><li><a href="{link}">Chapter one</a></li></ol></nav></body></html>
"#
    );
    zip(&[
        ("OEBPS/content.opf".to_string(), opf),
        ("OEBPS/Text/nav.xhtml".to_string(), nav),
        (
            format!("OEBPS/{name}"),
            format!("{EPUB3_HEAD}<body><p id=\"x\">One. Two.</p></body></html>\n"),
        ),
    ])
}

fn zip(files: &[(String, String)]) -> Vec<u8> {
    let mut z = ZipWriter::new(Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    z.start_file("mimetype", stored).unwrap();
    z.write_all(b"application/epub+zip").unwrap();
    z.start_file("META-INF/container.xml", deflated).unwrap();
    z.write_all(br#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#).unwrap();
    for (name, text) in files {
        z.start_file(name.as_str(), deflated).unwrap();
        z.write_all(text.as_bytes()).unwrap();
    }
    z.finish().unwrap().into_inner()
}

fn entry(epub: &[u8], name: &str) -> String {
    let mut z = ZipArchive::new(Cursor::new(epub)).unwrap();
    let mut s = String::new();
    z.by_name(name).unwrap().read_to_string(&mut s).unwrap();
    s
}

fn raw_entry(epub: &[u8], name: &str) -> Vec<u8> {
    let mut z = ZipArchive::new(Cursor::new(epub)).unwrap();
    let i = z.index_for_name(name).unwrap();
    let mut v = Vec::new();
    z.by_index_raw(i).unwrap().read_to_end(&mut v).unwrap();
    v
}

/// Every finding, at every severity.
fn findings(epub: &[u8]) -> Vec<String> {
    epubveri::validate_bytes(epub.to_vec())
        .messages
        .iter()
        .map(|m| format!("{} {:?} {}", m.id, m.severity, m.text))
        .collect()
}

/// The findings of `after` are a subset of those of `before`.
fn adds_no_finding(before: &[u8], after: &[u8]) {
    let b = findings(before);
    let added: Vec<String> = findings(after)
        .into_iter()
        .filter(|f| !b.contains(f))
        .collect();
    assert_eq!(added, Vec::<String>::new(), "findings the conversion added");
}

#[test]
fn an_escape_the_manifest_does_not_use_is_written_as_the_manifest_writes_it() {
    let input = epub2(
        &[("Agatha'nin.xhtml", "Agatha'nin.xhtml")],
        &["Agatha%27nin.xhtml", "Agatha%27nin.xhtml#x"],
    );
    assert_eq!(
        findings(&input),
        Vec::<String>::new(),
        "the fixture is valid"
    );
    let c = convert(&input).unwrap();
    assert!(c.complete());
    let ncx = entry(&c.epub, "OEBPS/toc.ncx");
    assert!(
        ncx.contains(r#"<content src="Agatha'nin.xhtml"/>"#),
        "{ncx}"
    );
    assert!(
        ncx.contains(r#"<content src="Agatha'nin.xhtml#x"/>"#),
        "{ncx}"
    );
    assert!(!ncx.contains("%27"));
    // The manifest already writes a valid spelling: the package document is
    // copied as it came.
    assert_eq!(
        raw_entry(&c.epub, "OEBPS/content.opf"),
        raw_entry(&input, "OEBPS/content.opf")
    );
    assert_eq!(findings(&c.epub), Vec::<String>::new());
}

#[test]
fn a_raw_space_in_the_manifest_is_encoded_and_every_reference_follows() {
    let input = epub2(
        &[
            ("Text/a b.xhtml", "Text/a b.xhtml"),
            ("Text/c d.xhtml", "Text/c d.xhtml"),
        ],
        &["Text/a%20b.xhtml", "Text/a b.xhtml#x", "Text/c d.xhtml"],
    );
    let c = convert(&input).unwrap();
    assert!(c.complete());
    let opf = entry(&c.epub, "OEBPS/content.opf");
    assert!(opf.contains(r#"href="Text/a%20b.xhtml""#), "{opf}");
    // No reference spells `c d` differently, so it is left as it is.
    assert!(opf.contains(r#"href="Text/c d.xhtml""#), "{opf}");
    let ncx = entry(&c.epub, "OEBPS/toc.ncx");
    assert!(ncx.contains(r#"src="Text/a%20b.xhtml""#));
    assert!(ncx.contains(r#"src="Text/a%20b.xhtml#x""#));
    assert!(ncx.contains(r#"src="Text/c d.xhtml""#));
    // The renamed reference names the same entry, which is converted.
    assert!(entry(&c.epub, "OEBPS/Text/a b.xhtml").contains("koboSpan"));
    adds_no_finding(&input, &c.epub);
    assert!(
        findings(&c.epub).len() < findings(&input).len(),
        "an RSC-020 goes away"
    );
}

#[test]
fn the_navigation_document_is_respelled_and_converted() {
    let input = epub3(("Text/a'b.xhtml", "Text/a'b.xhtml"), "a%27b.xhtml#x");
    assert_eq!(
        findings(&input),
        Vec::<String>::new(),
        "the fixture is valid"
    );
    let c = convert(&input).unwrap();
    assert!(c.complete());
    let nav = entry(&c.epub, "OEBPS/Text/nav.xhtml");
    assert!(nav.contains(r#"href="a'b.xhtml#x""#), "{nav}");
    assert!(nav.contains("koboSpan"), "{nav}");
    assert_eq!(findings(&c.epub), Vec::<String>::new());
}

#[test]
fn names_that_agree_leave_both_documents_as_they_came() {
    for input in [
        epub2(&[("Text/c0.xhtml", "Text/c0.xhtml")], &["Text/c0.xhtml"]),
        // A raw space on both sides is one name to the renderer.
        epub2(&[("Text/a b.xhtml", "Text/a b.xhtml")], &["Text/a b.xhtml"]),
        // A difference in case is a broken reference: epubsana's to repair.
        epub2(&[("Text/C0.xhtml", "Text/C0.xhtml")], &["Text/c0.xhtml"]),
    ] {
        let c = convert(&input).unwrap();
        for name in ["OEBPS/content.opf", "OEBPS/toc.ncx"] {
            assert_eq!(raw_entry(&c.epub, name), raw_entry(&input, name), "{name}");
        }
    }
}

#[test]
fn converting_twice_changes_nothing_more() {
    let input = epub2(
        &[("Text/a b.xhtml", "Text/a b.xhtml")],
        &["Text/a%20b.xhtml"],
    );
    let once = convert(&input).unwrap().epub;
    let twice = convert(&once).unwrap().epub;
    assert_eq!(once, twice);
}
