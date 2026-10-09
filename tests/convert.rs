//! The conversion end to end, on books built here (docs/SPANS.md).

use std::io::{Cursor, Read, Write};

use kepubverto::{Outcome, Reason, convert};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const EPUB3_HEAD: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE html>\n<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>T</title></head>";
const EPUB2_HEAD: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE html PUBLIC \"-//W3C//DTD XHTML 1.1//EN\" \"http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd\">\n<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>T</title></head>";

fn page(head: &str, body: &str) -> String {
    format!("{head}<body>{body}</body></html>\n")
}

/// A book with one content document per entry of `docs`, plus a stylesheet.
fn book(epub3: bool, docs: &[&str]) -> Vec<u8> {
    let items: String = (0..docs.len())
        .map(|i| {
            format!(
                r#"<item id="c{i}" href="Text/c{i}.xhtml" media-type="application/xhtml+xml"/>"#
            )
        })
        .collect();
    let spine: String = (0..docs.len())
        .map(|i| format!(r#"<itemref idref="c{i}"/>"#))
        .collect();
    let version = if epub3 { "3.0" } else { "2.0" };
    let opf = format!(
        r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="{version}" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">urn:x</dc:identifier><dc:title>T</dc:title><dc:language>en</dc:language></metadata><manifest>{items}<item id="css" href="style.css" media-type="text/css"/></manifest><spine>{spine}</spine></package>"#
    );
    let mut z = ZipWriter::new(Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    z.start_file("mimetype", stored).unwrap();
    z.write_all(b"application/epub+zip").unwrap();
    z.start_file("META-INF/container.xml", deflated).unwrap();
    z.write_all(br#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#).unwrap();
    z.start_file("OEBPS/content.opf", deflated).unwrap();
    z.write_all(opf.as_bytes()).unwrap();
    z.start_file("OEBPS/style.css", deflated).unwrap();
    z.write_all(b"p { margin: 0 }").unwrap();
    for (i, d) in docs.iter().enumerate() {
        z.start_file(format!("OEBPS/Text/c{i}.xhtml"), deflated)
            .unwrap();
        z.write_all(d.as_bytes()).unwrap();
    }
    z.finish().unwrap().into_inner()
}

fn entry(epub: &[u8], name: &str) -> String {
    let mut z = ZipArchive::new(Cursor::new(epub)).unwrap();
    let mut s = String::new();
    z.by_name(name).unwrap().read_to_string(&mut s).unwrap();
    s
}

fn raw_entry(epub: &[u8], name: &str) -> (CompressionMethod, Vec<u8>) {
    let mut z = ZipArchive::new(Cursor::new(epub)).unwrap();
    let i = z.index_for_name(name).unwrap();
    let mut f = z.by_index_raw(i).unwrap();
    let mut v = Vec::new();
    f.read_to_end(&mut v).unwrap();
    (f.compression(), v)
}

const STYLE: &str = r#"<style type="text/css" class="kobostylehacks">div#book-inner { margin-top: 0; margin-bottom: 0;}</style>"#;

#[test]
fn converts_a_document_and_keeps_every_other_byte() {
    let doc = page(EPUB3_HEAD, "\r\n<p>One. Two.</p>\r\n");
    let c = convert(&book(true, &[&doc])).unwrap();
    assert_eq!(c.documents[0].outcome, Outcome::Converted { spans: 2 });
    assert!(c.complete());
    let out = entry(&c.epub, "OEBPS/Text/c0.xhtml");
    let expected = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE html>\n<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>T</title>{STYLE}</head><body><div id=\"book-columns\"><div id=\"book-inner\">\r\n<p><span class=\"koboSpan\" id=\"kobo.1.1\">One. </span><span class=\"koboSpan\" id=\"kobo.1.2\">Two.</span></p>\r\n</div></div></body></html>\n"
    );
    assert_eq!(out, expected);
}

#[test]
fn untouched_entries_are_copied_raw() {
    let doc = page(EPUB3_HEAD, "<p>x</p>");
    let input = book(true, &[&doc]);
    let c = convert(&input).unwrap();
    for name in [
        "mimetype",
        "META-INF/container.xml",
        "OEBPS/content.opf",
        "OEBPS/style.css",
    ] {
        assert_eq!(raw_entry(&input, name), raw_entry(&c.epub, name), "{name}");
    }
    let names = |b: &[u8]| {
        ZipArchive::new(Cursor::new(b))
            .unwrap()
            .file_names()
            .map(str::to_string)
            .collect::<Vec<_>>()
    };
    let mut a = names(&input);
    let mut b = names(&c.epub);
    a.sort();
    b.sort();
    assert_eq!(a, b);
    assert_eq!(
        raw_entry(&c.epub, "OEBPS/Text/c0.xhtml").0,
        CompressionMethod::Deflated
    );
}

#[test]
fn converting_twice_changes_nothing() {
    let doc = page(
        EPUB3_HEAD,
        "<h1>T</h1><p>a. b</p><p><img src=\"i.png\" alt=\"\"/></p>",
    );
    let once = convert(&book(true, &[&doc])).unwrap();
    let twice = convert(&once.epub).unwrap();
    assert_eq!(twice.documents[0].outcome, Outcome::AlreadyConverted);
    assert_eq!(
        entry(&once.epub, "OEBPS/Text/c0.xhtml"),
        entry(&twice.epub, "OEBPS/Text/c0.xhtml")
    );
}

#[test]
fn epub2_named_entities_parse_and_are_kept() {
    let doc = page(EPUB2_HEAD, "<p>a&nbsp;b. c&rsquo;s. d</p>");
    let c = convert(&book(false, &[&doc])).unwrap();
    assert_eq!(c.documents[0].outcome, Outcome::Converted { spans: 3 });
    let out = entry(&c.epub, "OEBPS/Text/c0.xhtml");
    assert!(out.contains(r#"<span class="koboSpan" id="kobo.1.1">a&nbsp;b. </span><span class="koboSpan" id="kobo.1.2">c&rsquo;s. </span>"#), "{out}");
    assert!(
        !out.contains("<!ENTITY"),
        "the declarations are for parsing only"
    );
    assert!(
        out.contains("xhtml11.dtd\">\n<html"),
        "the DOCTYPE is kept as it was"
    );
}

#[test]
fn a_document_that_is_not_well_formed_is_left_alone() {
    let bad = page(EPUB3_HEAD, "<p>open");
    let good = page(EPUB3_HEAD, "<p>x</p>");
    let input = book(true, &[&bad, &good]);
    let c = convert(&input).unwrap();
    assert!(matches!(
        c.documents[0].outcome,
        Outcome::LeftAlone(Reason::NotWellFormed(_))
    ));
    assert_eq!(c.documents[1].outcome, Outcome::Converted { spans: 1 });
    assert!(!c.complete());
    assert_eq!(
        raw_entry(&input, "OEBPS/Text/c0.xhtml"),
        raw_entry(&c.epub, "OEBPS/Text/c0.xhtml")
    );
}

#[test]
fn an_id_the_conversion_would_add_leaves_the_document_alone() {
    for body in [r#"<p id="kobo.3.1">x</p>"#, r#"<p id="book-inner">x</p>"#] {
        let c = convert(&book(true, &[&page(EPUB3_HEAD, body)])).unwrap();
        assert!(
            matches!(
                c.documents[0].outcome,
                Outcome::LeftAlone(Reason::IdInUse(_))
            ),
            "{body}"
        );
    }
}

#[test]
fn existing_divs_and_style_are_not_added_again() {
    let head = EPUB3_HEAD.replace("</head>", &format!("{STYLE}</head>"));
    let doc = page(
        &head,
        r#"<div id="book-columns"><div id="book-inner"><p>x</p></div></div>"#,
    );
    let c = convert(&book(true, &[&doc])).unwrap();
    let out = entry(&c.epub, "OEBPS/Text/c0.xhtml");
    assert_eq!(out.matches("book-columns").count(), 1);
    assert_eq!(out.matches("kobostylehacks").count(), 1);
    assert!(out.contains(r#"<p><span class="koboSpan" id="kobo.1.1">x</span></p>"#));
}

#[test]
fn where_a_span_is_invalid_it_is_counted_and_left_out() {
    // kepubify gives the option text kobo.0.1 and the paragraph kobo.1.1.
    let doc = page(
        EPUB3_HEAD,
        "<form><select><option>o</option></select></form><picture><img src=\"i.png\" alt=\"\"/></picture><p>after</p>",
    );
    let c = convert(&book(true, &[&doc])).unwrap();
    let out = entry(&c.epub, "OEBPS/Text/c0.xhtml");
    assert!(out.contains("<option>o</option>"), "{out}");
    assert!(out.contains("<picture><img"), "{out}");
    assert!(
        out.contains(r#"<p><span class="koboSpan" id="kobo.2.1">after</span></p>"#),
        "{out}"
    );

    // XHTML 1.1 allows no text directly in a blockquote; EPUB 3 does.
    let body = "<blockquote>stray</blockquote><p>after</p>";
    let two = entry(
        &convert(&book(false, &[&page(EPUB2_HEAD, body)]))
            .unwrap()
            .epub,
        "OEBPS/Text/c0.xhtml",
    );
    assert!(two.contains("<blockquote>stray</blockquote>"), "{two}");
    assert!(two.contains(r#"id="kobo.1.1">after"#), "{two}");
    let three = entry(
        &convert(&book(true, &[&page(EPUB3_HEAD, body)]))
            .unwrap()
            .epub,
        "OEBPS/Text/c0.xhtml",
    );
    assert!(
        three.contains(
            r#"<blockquote><span class="koboSpan" id="kobo.0.1">stray</span></blockquote>"#
        ),
        "{three}"
    );
}

#[test]
fn comments_cdata_and_instructions_split_text_and_are_never_wrapped() {
    let doc = page(
        EPUB3_HEAD,
        "<p>abc<![CDATA[x. y]]>def<!-- c. d -->g<?pi x?>h</p>",
    );
    let out = entry(
        &convert(&book(true, &[&doc])).unwrap().epub,
        "OEBPS/Text/c0.xhtml",
    );
    assert!(out.contains(r#"<p><span class="koboSpan" id="kobo.1.1">abc</span><![CDATA[x. y]]><span class="koboSpan" id="kobo.1.2">def</span><!-- c. d --><span class="koboSpan" id="kobo.1.3">g</span><?pi x?><span class="koboSpan" id="kobo.1.4">h</span></p>"#), "{out}");
}

#[test]
fn an_empty_p_starts_a_paragraph_like_p_p() {
    let a = entry(
        &convert(&book(true, &[&page(EPUB3_HEAD, "<div>a<p/>b</div>")]))
            .unwrap()
            .epub,
        "OEBPS/Text/c0.xhtml",
    );
    let b = entry(
        &convert(&book(true, &[&page(EPUB3_HEAD, "<div>a<p></p>b</div>")]))
            .unwrap()
            .epub,
        "OEBPS/Text/c0.xhtml",
    );
    assert!(
        a.contains(r#"<p/><span class="koboSpan" id="kobo.1.1">b</span>"#),
        "{a}"
    );
    assert!(
        b.contains(r#"<p></p><span class="koboSpan" id="kobo.1.1">b</span>"#),
        "{b}"
    );
}

#[test]
fn a_missing_document_is_reported_not_fatal() {
    let input = book(true, &[&page(EPUB3_HEAD, "<p>x</p>")]);
    // Point the manifest at a second, absent file.
    let opf = entry(&input, "OEBPS/content.opf").replace(
        "</manifest>",
        r#"<item id="gone" href="Text/gone.xhtml" media-type="application/xhtml+xml"/></manifest>"#,
    );
    let mut src = ZipArchive::new(Cursor::new(input.as_slice())).unwrap();
    let mut z = ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..src.len() {
        let f = src.by_index_raw(i).unwrap();
        if f.name() == "OEBPS/content.opf" {
            drop(f);
            z.start_file("OEBPS/content.opf", SimpleFileOptions::default())
                .unwrap();
            z.write_all(opf.as_bytes()).unwrap();
        } else {
            z.raw_copy_file(f).unwrap();
        }
    }
    let c = convert(&z.finish().unwrap().into_inner()).unwrap();
    assert_eq!(c.documents[1].outcome, Outcome::LeftAlone(Reason::Missing));
}

#[test]
fn a_file_that_is_not_a_zip_is_an_error() {
    assert!(matches!(
        convert(b"not a zip"),
        Err(kepubverto::Error::NotAnEpub(_))
    ));
}
