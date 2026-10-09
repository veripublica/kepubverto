//! "Never make a book worse": valid books stay valid. Each fixture is graded
//! by epubveri before and after the conversion. Only the tests validate;
//! the conversion never does (docs/SPANS.md).

use std::io::{Cursor, Write};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

/// A 1×1 PNG.
const PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

/// The bodies every fixture book carries: the shapes the rules treat
/// differently.
const BODIES: &[&str] = &[
    "<h1>Chapter one</h1><p>One. Two! Three? “Four.” Five</p><p>3.14 is pi. Mr. Smith.</p>",
    "<div><p>In a div. <b>Bold.</b> <i>Italic</i> tail.</p> <p/> <p></p></div>",
    "<p><img src=\"../Images/i.png\" alt=\"\"/></p><p>a<img src=\"../Images/i.png\" alt=\"\"/>b</p>",
    "<ul><li>x. y</li><li><p>nested. p</p></li></ul><ol><li>one</li></ol>",
    "<table><tr><td>c. d</td><td> </td></tr></table>",
    "<blockquote><p>Quoted. Text.</p></blockquote><pre>pre. text</pre>",
    "<p>a<!-- c. d -->b<br/>c. d</p><dl><dt>t</dt><dd>d. e</dd></dl>",
    "<p><a href=\"c0.xhtml\">link. text</a> more. <span>inner</span></p>",
];

fn epub3_page(body: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE html>\n<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\" xml:lang=\"en\" lang=\"en\">\n<head>\n<title>T</title>\n</head>\n<body>\n{body}\n</body>\n</html>\n"
    )
}

fn epub2_page(body: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE html PUBLIC \"-//W3C//DTD XHTML 1.1//EN\" \"http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd\">\n<html xmlns=\"http://www.w3.org/1999/xhtml\" xml:lang=\"en\">\n<head>\n<title>T</title>\n</head>\n<body>\n{body}\n<p>Entities&nbsp;too. &ldquo;Quoted.&rdquo; Done</p>\n</body>\n</html>\n"
    )
}

fn book(epub3: bool) -> Vec<u8> {
    let mut items = String::new();
    let mut spine = String::new();
    for i in 0..BODIES.len() {
        items.push_str(&format!(
            r#"<item id="c{i}" href="Text/c{i}.xhtml" media-type="application/xhtml+xml"/>"#
        ));
        spine.push_str(&format!(r#"<itemref idref="c{i}"/>"#));
    }
    items.push_str(r#"<item id="img" href="Images/i.png" media-type="image/png"/>"#);
    let opf = if epub3 {
        format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">urn:uuid:0b2cbd9c-5f30-4d5c-9a35-2a7e3b9f3c11</dc:identifier><dc:title>T</dc:title><dc:language>en</dc:language><meta property="dcterms:modified">2026-01-01T00:00:00Z</meta></metadata><manifest>{items}<item id="nav" href="Text/nav.xhtml" media-type="application/xhtml+xml" properties="nav"/></manifest><spine>{spine}</spine></package>"#
        )
    } else {
        format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf"><dc:identifier id="id">urn:uuid:0b2cbd9c-5f30-4d5c-9a35-2a7e3b9f3c11</dc:identifier><dc:title>T</dc:title><dc:language>en</dc:language></metadata><manifest>{items}<item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/></manifest><spine toc="ncx">{spine}</spine></package>"#
        )
    };
    let mut z = ZipWriter::new(Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    z.start_file("mimetype", stored).unwrap();
    z.write_all(b"application/epub+zip").unwrap();
    z.start_file("META-INF/container.xml", deflated).unwrap();
    z.write_all(br#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#).unwrap();
    z.start_file("OEBPS/content.opf", deflated).unwrap();
    z.write_all(opf.as_bytes()).unwrap();
    z.start_file("OEBPS/Images/i.png", stored).unwrap();
    z.write_all(PNG).unwrap();
    for (i, body) in BODIES.iter().enumerate() {
        let page = if epub3 {
            epub3_page(body)
        } else {
            epub2_page(body)
        };
        z.start_file(format!("OEBPS/Text/c{i}.xhtml"), deflated)
            .unwrap();
        z.write_all(page.as_bytes()).unwrap();
    }
    if epub3 {
        z.start_file("OEBPS/Text/nav.xhtml", deflated).unwrap();
        z.write_all(epub3_page(r#"<nav epub:type="toc"><h1>Contents</h1><ol><li><a href="c0.xhtml">Chapter one. Start</a></li></ol></nav>"#).as_bytes()).unwrap();
    } else {
        z.start_file("OEBPS/toc.ncx", deflated).unwrap();
        z.write_all(br#"<?xml version="1.0" encoding="utf-8"?><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><head><meta name="dtb:uid" content="urn:uuid:0b2cbd9c-5f30-4d5c-9a35-2a7e3b9f3c11"/><meta name="dtb:depth" content="1"/><meta name="dtb:totalPageCount" content="0"/><meta name="dtb:maxPageNumber" content="0"/></head><docTitle><text>T</text></docTitle><navMap><navPoint id="n1" playOrder="1"><navLabel><text>One</text></navLabel><content src="Text/c0.xhtml"/></navPoint></navMap></ncx>"#).unwrap();
    }
    z.finish().unwrap().into_inner()
}

fn problems(epub: &[u8]) -> Vec<String> {
    let report = epubveri::validate_bytes(epub.to_vec());
    report
        .messages
        .iter()
        .filter(|m| {
            matches!(
                m.severity,
                epubveri::report::Severity::Error | epubveri::report::Severity::Fatal
            )
        })
        .map(|m| format!("{} {} [{:?}]", m.id, m.text, m.location))
        .collect()
}

fn stays_valid(epub3: bool) {
    let input = book(epub3);
    assert_eq!(
        problems(&input),
        Vec::<String>::new(),
        "the fixture itself must be valid"
    );
    let c = kepubverto::convert(&input).unwrap();
    assert!(c.complete());
    assert!(
        c.spans() > 40,
        "the fixture exercises the rules ({} spans)",
        c.spans()
    );
    assert_eq!(problems(&c.epub), Vec::<String>::new());
}

#[test]
fn a_valid_epub3_stays_valid() {
    stays_valid(true);
}

#[test]
fn a_valid_epub2_stays_valid() {
    stays_valid(false);
}
