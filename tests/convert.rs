//! The conversion end to end, on books built here (docs/SPANS.md).

use std::io::{Cursor, Read, Write};

use kepubverto::{Hyphens, Options, Outcome, Reason, convert, convert_with};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const EPUB3_HEAD: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE html>\n<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>T</title></head>";
const EPUB2_HEAD: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE html PUBLIC \"-//W3C//DTD XHTML 1.1//EN\" \"http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd\">\n<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>T</title></head>";

fn page(head: &str, body: &str) -> String {
    format!("{head}<body>{body}</body></html>\n")
}

/// A book with one content document per entry of `docs`, plus a stylesheet.
fn book(epub3: bool, docs: &[&str]) -> Vec<u8> {
    let docs: Vec<&[u8]> = docs.iter().map(|d| d.as_bytes()).collect();
    book_bytes(epub3, &docs)
}

/// [`book`], with documents given as bytes (for encodings other than UTF-8).
fn book_bytes(epub3: bool, docs: &[&[u8]]) -> Vec<u8> {
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
        z.write_all(d).unwrap();
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

const STYLE: &str =
    r#"<style class="kobostylehacks">div#book-inner { margin-top: 0; margin-bottom: 0;}</style>"#;
const STYLE_EPUB2: &str =
    r#"<style type="text/css">div#book-inner { margin-top: 0; margin-bottom: 0;}</style>"#;
/// What kepubify 4.0.4 appends, in EPUB 2 and EPUB 3 alike.
const KEPUBIFY_STYLE: &str = r#"<style type="text/css" class="kobostylehacks">div#book-inner { margin-top: 0; margin-bottom: 0;}</style>"#;

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
            .map(|n| n.unwrap().into_owned())
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
fn the_style_element_follows_the_epub_version() {
    let c = convert(&book(true, &[&page(EPUB3_HEAD, "<p>x</p>")])).unwrap();
    let out = entry(&c.epub, "OEBPS/Text/c0.xhtml");
    assert!(out.contains(&format!("{STYLE}</head>")), "{out}");
    assert!(!out.contains("type=\"text/css\""), "{out}");

    let c = convert(&book(false, &[&page(EPUB2_HEAD, "<p>x</p>")])).unwrap();
    let out = entry(&c.epub, "OEBPS/Text/c0.xhtml");
    assert!(out.contains(&format!("{STYLE_EPUB2}</head>")), "{out}");
    assert!(!out.contains("kobostylehacks"), "{out}");
}

#[test]
fn the_unmarked_epub2_style_is_not_added_again() {
    // No text, so no span marks the document as converted: only the rule
    // text itself shows that the style is already there.
    let head = EPUB2_HEAD.replace("</head>", &format!("{STYLE_EPUB2}</head>"));
    let doc = page(
        &head,
        r#"<div id="book-columns"><div id="book-inner"><hr/></div></div>"#,
    );
    let c = convert(&book(false, &[&doc])).unwrap();
    assert_eq!(c.documents[0].outcome, Outcome::NothingToDo);
}

fn with_hyphens(h: Hyphens) -> Options {
    Options {
        hyphens: Some(h),
        ..Default::default()
    }
}

#[test]
fn hyphens_appends_its_rule_after_the_style_rule() {
    let auto = kepubverto::content::HYPHENS_AUTO;
    let none = kepubverto::content::HYPHENS_NONE;
    let epub3 = book(true, &[&page(EPUB3_HEAD, "<p>x</p>")]);
    let epub2 = book(false, &[&page(EPUB2_HEAD, "<p>x</p>")]);

    let c = convert_with(&epub3, &with_hyphens(Hyphens::Auto)).unwrap();
    let out = entry(&c.epub, "OEBPS/Text/c0.xhtml");
    assert!(
        out.contains(&format!("{STYLE}<style>{auto}</style></head>")),
        "{out}"
    );

    let c = convert_with(&epub2, &with_hyphens(Hyphens::None)).unwrap();
    let out = entry(&c.epub, "OEBPS/Text/c0.xhtml");
    assert!(
        out.contains(&format!(
            "{STYLE_EPUB2}<style type=\"text/css\">{none}</style></head>"
        )),
        "{out}"
    );

    // Without the option, nothing about hyphenation is added.
    let out = entry(&convert(&epub3).unwrap().epub, "OEBPS/Text/c0.xhtml");
    assert!(!out.contains("hyphens"), "{out}");
}

#[test]
fn the_hyphens_rule_is_not_added_again() {
    // No text, so no span marks the document as converted: the rule text
    // itself shows that it is there, whichever value it was converted with.
    let doc = page(EPUB3_HEAD, "<hr/>");
    let once = convert_with(&book(true, &[&doc]), &with_hyphens(Hyphens::Auto)).unwrap();
    for h in [Hyphens::Auto, Hyphens::None] {
        let twice = convert_with(&once.epub, &with_hyphens(h)).unwrap();
        assert_eq!(twice.documents[0].outcome, Outcome::NothingToDo);
    }
}

fn with_css(css: &[&str]) -> Options {
    Options {
        css: css.iter().map(|c| c.to_string()).collect(),
        ..Default::default()
    }
}

#[test]
fn css_comes_last_in_one_style_element() {
    let auto = kepubverto::content::HYPHENS_AUTO;
    let options = Options {
        hyphens: Some(Hyphens::Auto),
        css: vec!["p { text-indent: 1.5em }".into(), "h1 { margin: 0 }".into()],
    };
    let epub3 = book(true, &[&page(EPUB3_HEAD, "<p>x</p>")]);
    let out = entry(
        &convert_with(&epub3, &options).unwrap().epub,
        "OEBPS/Text/c0.xhtml",
    );
    assert!(
        out.contains(&format!(
            "{STYLE}<style>{auto}</style><style>p {{text-indent:1.5em;}}\nh1 {{margin:0;}}</style></head>"
        )),
        "{out}"
    );

    let epub2 = book(false, &[&page(EPUB2_HEAD, "<p>x</p>")]);
    let out = entry(
        &convert_with(&epub2, &with_css(&["p { margin: 0 }"]))
            .unwrap()
            .epub,
        "OEBPS/Text/c0.xhtml",
    );
    assert!(
        out.contains(&format!(
            "{STYLE_EPUB2}<style type=\"text/css\">p {{margin:0;}}</style></head>"
        )),
        "{out}"
    );
}

#[test]
fn the_css_is_not_added_again() {
    let doc = page(EPUB3_HEAD, "<hr/>");
    let options = with_css(&["p { margin: 0 }"]);
    let once = convert_with(&book(true, &[&doc]), &options).unwrap();
    let twice = convert_with(&once.epub, &options).unwrap();
    assert_eq!(twice.documents[0].outcome, Outcome::NothingToDo);
}

#[test]
fn a_refused_css_value_converts_nothing() {
    let epub = book(true, &[&page(EPUB3_HEAD, "<p>x</p>")]);
    for css in [
        "body { background: url(bg.png) }",
        "@font-face { font-family: X; src: local(Georgia) }",
        "p { position: fixed }",
        "@media (width < 600px) { p { margin: 0 } }",
        "",
    ] {
        match convert_with(&epub, &with_css(&[css])) {
            Err(kepubverto::Error::Css(e)) => assert_eq!(e.value, 1, "{css}"),
            other => panic!("{css}: {other:?}"),
        }
    }
}

#[test]
fn css_is_checked_by_the_books_version() {
    // EPUB 3 flags `direction` (CSS-001); EPUB 2 does not.
    let options = with_css(&["p { direction: rtl }"]);
    assert!(convert_with(&book(false, &[&page(EPUB2_HEAD, "<p>x</p>")]), &options).is_ok());
    assert!(matches!(
        convert_with(&book(true, &[&page(EPUB3_HEAD, "<p>x</p>")]), &options),
        Err(kepubverto::Error::Css(_))
    ));
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
    let head = EPUB3_HEAD.replace("</head>", &format!("{KEPUBIFY_STYLE}</head>"));
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
        if f.name().unwrap() == "OEBPS/content.opf" {
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

/// `input` with every content document declared `text/html`.
fn as_text_html(input: &[u8]) -> Vec<u8> {
    let opf = entry(input, "OEBPS/content.opf").replace(
        r#"media-type="application/xhtml+xml""#,
        r#"media-type="text/html""#,
    );
    let mut src = ZipArchive::new(Cursor::new(input)).unwrap();
    let mut z = ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..src.len() {
        let f = src.by_index_raw(i).unwrap();
        if f.name().unwrap() == "OEBPS/content.opf" {
            drop(f);
            z.start_file("OEBPS/content.opf", SimpleFileOptions::default())
                .unwrap();
            z.write_all(opf.as_bytes()).unwrap();
        } else {
            z.raw_copy_file(f).unwrap();
        }
    }
    z.finish().unwrap().into_inner()
}

#[test]
fn text_html_items_are_converted_like_xhtml() {
    let input = as_text_html(&book(true, &[&page(EPUB3_HEAD, "<p>x</p>")]));
    let c = convert(&input).unwrap();
    assert_eq!(c.documents[0].outcome, Outcome::Converted { spans: 1 });
}

/// HTML parsing lowercases names; SVG and MathML need their camelCase
/// (`viewBox`, `linearGradient`, `definitionURL`). A `text/html` item is
/// read as XML like any other and `svg`/`math` are skipped, so their bytes
/// come out as they went in.
#[test]
fn svg_and_mathml_names_keep_their_case_in_text_html_items() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><defs><linearGradient id="g"><stop offset="0"/></linearGradient><clipPath id="k"><rect width="5" height="5"/></clipPath></defs><text><textPath href="#p">a. b</textPath></text><rect width="10" height="10" fill="url(#g)" clip-path="url(#k)"/></svg>"##;
    let math = r#"<math xmlns="http://www.w3.org/1998/Math/MathML"><mi definitionURL="http://example.org/x">x</mi></math>"#;
    let body = format!("<p>Before. It</p>{svg}<p>{math} after. It</p>");
    let input = as_text_html(&book(true, &[&page(EPUB3_HEAD, &body)]));
    let c = convert(&input).unwrap();
    assert_eq!(c.documents[0].outcome, Outcome::Converted { spans: 4 });
    let out = entry(&c.epub, "OEBPS/Text/c0.xhtml");
    assert!(out.contains(svg), "{out}");
    assert!(out.contains(math), "{out}");
}

/// The reason a document is left alone for, and the code `--format json`
/// gives it (FORMATS §1.3: epubcheck's ID where it grades the state at
/// `error` or above, a `KEPUB-` code otherwise).
fn left_alone(doc: &[u8], text_html: bool) -> (Reason, &'static str, &'static str) {
    let mut input = book_bytes(true, &[doc]);
    if text_html {
        input = as_text_html(&input);
    }
    let c = convert(&input).unwrap();
    let d = &c.documents[0];
    let Outcome::LeftAlone(reason) = &d.outcome else {
        panic!("converted: {:?}", d.outcome);
    };
    let (code, severity) = reason.code(&d.media_type);
    (reason.clone(), code, severity.as_str())
}

fn utf16le(text: &str, bom: bool) -> Vec<u8> {
    let mut v = if bom { vec![0xFF, 0xFE] } else { Vec::new() };
    v.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    v
}

#[test]
fn utf16_is_htm_058_in_xhtml_and_ours_in_text_html() {
    let doc = page(EPUB3_HEAD, "<p>x</p>").replace("utf-8", "UTF-16");
    for bom in [true, false] {
        let (r, code, sev) = left_alone(&utf16le(&doc, bom), false);
        assert_eq!((r, code, sev), (Reason::Utf16, "HTM_058", "error"));
        // epubcheck grades UTF-16 outside application/xhtml+xml as a
        // warning (RSC-027), below what a part left untouched must carry.
        let (r, code, sev) = left_alone(&utf16le(&doc, bom), true);
        assert_eq!((r, code, sev), (Reason::Utf16, "KEPUB-006", "error"));
    }
}

#[test]
fn another_declared_encoding_is_rsc_028_or_rsc_016() {
    let latin1 = |name: &str| {
        let mut v = page(EPUB3_HEAD, "<p>caf_</p>")
            .replace("utf-8", name)
            .into_bytes();
        let i = v.iter().position(|&b| b == b'_').unwrap();
        v[i] = 0xE9; // é in ISO-8859-1, not UTF-8
        v
    };
    let (r, code, sev) = left_alone(&latin1("ISO-8859-1"), false);
    assert_eq!(
        r,
        Reason::Encoding {
            name: "ISO-8859-1".into(),
            parser_accepts: true
        }
    );
    assert_eq!((code, sev), ("RSC-028", "error"));
    // A name epubcheck's parser does not take stops it at the declaration.
    let (r, code, sev) = left_alone(&latin1("SJIS"), false);
    assert!(matches!(
        r,
        Reason::Encoding {
            parser_accepts: false,
            ..
        }
    ));
    assert_eq!((code, sev), ("RSC-016", "fatal"));
}

#[test]
fn a_declared_encoding_over_utf8_bytes_is_still_converted() {
    // ASCII is the same bytes in both; this is what 0.1 did, and still does.
    let doc = page(EPUB3_HEAD, "<p>x</p>").replace("utf-8", "ISO-8859-1");
    let c = convert(&book(true, &[&doc])).unwrap();
    assert_eq!(c.documents[0].outcome, Outcome::Converted { spans: 1 });
}

#[test]
fn every_other_reason_has_its_code() {
    let (_, code, sev) = left_alone(b"\xC3(not utf-8 <p>x</p>", false);
    assert_eq!((code, sev), ("KEPUB-001", "error"));
    let (_, code, sev) = left_alone(page(EPUB3_HEAD, "<p>open").as_bytes(), false);
    assert_eq!((code, sev), ("RSC-016", "fatal"));
    let no_body = "<?xml version=\"1.0\"?><html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>T</title></head></html>";
    let (_, code, sev) = left_alone(no_body.as_bytes(), false);
    assert_eq!((code, sev), ("RSC-005", "error"));
    let taken = page(EPUB3_HEAD, r#"<p id="kobo.1.1">x</p>"#);
    let (_, code, sev) = left_alone(taken.as_bytes(), false);
    assert_eq!((code, sev), ("KEPUB-003", "error"));
    assert_eq!(Reason::Missing.code("application/xhtml+xml").0, "RSC-001");
    for (r, want) in [
        (Reason::TooLarge("x".into()), "KEPUB-002"),
        (Reason::MarkupInEntity, "KEPUB-004"),
        (Reason::Unreadable("x".into()), "KEPUB-005"),
    ] {
        let (code, sev) = r.code("application/xhtml+xml");
        assert_eq!((code, sev.as_str()), (want, "error"));
    }
}

#[test]
fn a_file_that_is_not_a_zip_is_an_error() {
    assert!(matches!(
        convert(b"not a zip"),
        Err(kepubverto::Error::NotAnEpub(_))
    ));
}

/// Overwrite `bytes` at `local_at` in `name`'s local header and at
/// `central_at` in its central directory record.
fn patch_headers(epub: &mut [u8], name: &str, local_at: usize, central_at: usize, bytes: &[u8]) {
    for (sig, name_at, at) in [
        (&b"PK\x03\x04"[..], 30, local_at),
        (&b"PK\x01\x02"[..], 46, central_at),
    ] {
        let i = (0..epub.len() - name_at)
            .find(|&i| {
                epub[i..].starts_with(sig) && epub[i + name_at..].starts_with(name.as_bytes())
            })
            .unwrap();
        epub[i + at..i + at + bytes.len()].copy_from_slice(bytes);
    }
}

/// zip 9 refuses both by default; epubcheck and epubveri read them, so the
/// book still converts (src/container.rs, `Container::inflate`).
#[test]
fn a_wrong_crc_or_an_understated_size_still_converts() {
    let doc = page(EPUB3_HEAD, "<p>x</p>");
    let mut wrong_crc = book(true, &[&doc]);
    patch_headers(&mut wrong_crc, "OEBPS/Text/c0.xhtml", 14, 16, &[0; 4]);
    let mut undersized = book(true, &[&doc]);
    patch_headers(
        &mut undersized,
        "OEBPS/Text/c0.xhtml",
        22,
        24,
        &[1, 0, 0, 0],
    );
    for input in [wrong_crc, undersized] {
        let c = convert(&input).unwrap();
        assert!(c.complete());
        assert!(entry(&c.epub, "OEBPS/Text/c0.xhtml").contains("kobo.1.1"));
    }
}

/// An untouched entry is copied verbatim, its extra fields with it. Before
/// 0.3.1, zip's writer dropped them from both headers; zip 9's `raw_copy_file`
/// refuses an Info-ZIP Unix field (0x7875) outright, which once stopped a
/// shelf book from being written.
#[test]
fn an_untouched_entry_with_a_unix_extra_field_is_copied_with_it() {
    let doc = page(EPUB3_HEAD, "<p>x</p>");
    let mut input = book(true, &[&doc]);
    // Rebuild with a 0x5455 field on the stylesheet, then re-tag it 0x7875.
    {
        let mut src = ZipArchive::new(Cursor::new(input.clone())).unwrap();
        let mut z = ZipWriter::new(Cursor::new(Vec::new()));
        for i in 0..src.len() {
            let f = src.by_index_raw(i).unwrap();
            if f.name().unwrap() != "OEBPS/style.css" {
                z.raw_copy_file(f).unwrap();
                continue;
            }
            drop(f);
            let mut opts = zip::write::FullFileOptions::default()
                .compression_method(CompressionMethod::Deflated);
            opts.add_extra_field(0x5455, [1u8, 0, 0, 0, 0], false)
                .unwrap();
            z.start_file("OEBPS/style.css", opts).unwrap();
            z.write_all(b"p { margin: 0 }").unwrap();
        }
        input = z.finish().unwrap().into_inner();
    }
    let extra_lengths = |epub: &[u8]| {
        let mut out = Vec::new();
        for (sig, name_at, len_at) in [(&b"PK\x03\x04"[..], 30, 28), (&b"PK\x01\x02"[..], 46, 30)] {
            let i = (0..epub.len() - name_at)
                .find(|&i| {
                    epub[i..].starts_with(sig)
                        && epub[i + name_at..].starts_with(b"OEBPS/style.css")
                })
                .unwrap();
            out.push(u16::from_le_bytes([epub[i + len_at], epub[i + len_at + 1]]));
        }
        out
    };
    let name_len = "OEBPS/style.css".len();
    patch_headers(
        &mut input,
        "OEBPS/style.css",
        30 + name_len,
        46 + name_len,
        &0x7875u16.to_le_bytes(),
    );
    assert_eq!(extra_lengths(&input), [9, 9]);
    let c = convert(&input).unwrap();
    assert_eq!(extra_lengths(&c.epub), [9, 9]);
    assert_eq!(
        local_part(&c.epub, "OEBPS/style.css"),
        local_part(&input, "OEBPS/style.css")
    );
    assert_eq!(entry(&c.epub, "OEBPS/style.css"), "p { margin: 0 }");
}

/// `name`'s local header and data, as they lie in `epub`.
fn local_part(epub: &[u8], name: &str) -> Vec<u8> {
    let mut z = ZipArchive::new(Cursor::new(epub)).unwrap();
    let f = z.by_name(name).unwrap();
    let (start, end) = (
        f.header_start(),
        f.data_start().unwrap() + f.compressed_size(),
    );
    epub[start as usize..end as usize].to_vec()
}

/// A book the conversion changes nothing in (its one document is not
/// well-formed) comes back byte-identical, written as `zip` writes or as a
/// stream, with a data descriptor after every entry (src/zipwrite.rs).
#[test]
fn a_book_nothing_changed_in_is_written_byte_identically() {
    let plain = book(true, &[&page(EPUB3_HEAD, "<p>open")]);
    let mut streamed = Vec::new();
    {
        let mut src = ZipArchive::new(Cursor::new(plain.as_slice())).unwrap();
        let mut z = ZipWriter::new_stream(&mut streamed);
        for i in 0..src.len() {
            let mut f = src.by_index(i).unwrap();
            let opts = SimpleFileOptions::default().compression_method(f.compression());
            let mut data = Vec::new();
            f.read_to_end(&mut data).unwrap();
            z.start_file(f.name().unwrap().into_owned(), opts).unwrap();
            z.write_all(&data).unwrap();
        }
        z.set_comment("an archive comment").unwrap();
        z.finish().unwrap();
    }
    assert!(streamed.windows(4).any(|w| w == b"PK\x07\x08"));
    for input in [plain, streamed] {
        let c = convert(&input).unwrap();
        assert!(!c.complete());
        assert_eq!(c.epub, input);
    }
}

/// A converted entry keeps every header field but those its new data decides
/// (CRC, sizes): time, attributes, extra fields and comment stay, and so does
/// the archive comment.
#[test]
fn a_converted_entry_keeps_its_header_fields() {
    let doc = page(EPUB3_HEAD, "<p>x</p>");
    let input = {
        let src_bytes = book(true, &[&doc]);
        let mut src = ZipArchive::new(Cursor::new(src_bytes.as_slice())).unwrap();
        let mut z = ZipWriter::new(Cursor::new(Vec::new()));
        for i in 0..src.len() {
            let f = src.by_index_raw(i).unwrap();
            if f.name().unwrap() != "OEBPS/Text/c0.xhtml" {
                z.raw_copy_file(f).unwrap();
                continue;
            }
            drop(f);
            let mut opts = zip::write::FullFileOptions::default()
                .compression_method(CompressionMethod::Deflated)
                .unix_permissions(0o600)
                .with_file_comment("an entry comment");
            opts.add_extra_field(0x5455, [1u8, 1, 2, 3, 4], false)
                .unwrap();
            opts.add_extra_field(0x6666, [9u8, 8, 7], true).unwrap();
            z.start_file("OEBPS/Text/c0.xhtml", opts).unwrap();
            z.write_all(doc.as_bytes()).unwrap();
        }
        z.set_comment("an archive comment").unwrap();
        z.finish().unwrap().into_inner()
    };
    let c = convert(&input).unwrap();
    assert_eq!(c.documents[0].outcome, Outcome::Converted { spans: 1 });
    let kept = |epub: &[u8]| {
        let name = b"OEBPS/Text/c0.xhtml";
        let find = |sig: &[u8], name_at: usize| {
            (0..epub.len() - name_at)
                .find(|&i| epub[i..].starts_with(sig) && epub[i + name_at..].starts_with(name))
                .unwrap()
        };
        let (l, c) = (find(b"PK\x03\x04", 30), find(b"PK\x01\x02", 46));
        let u16_at = |at: usize| u16::from_le_bytes([epub[at], epub[at + 1]]) as usize;
        let (ln, lx) = (u16_at(l + 26), u16_at(l + 28));
        let (cn, cx, cc) = (u16_at(c + 28), u16_at(c + 30), u16_at(c + 32));
        // Local: version, flags, method, time, date; name and extra.
        let mut keep = epub[l + 4..l + 14].to_vec();
        keep.extend_from_slice(&epub[l + 30..l + 30 + ln + lx]);
        // Central: made by .. date, then attributes, name, extra, comment.
        keep.extend_from_slice(&epub[c + 4..c + 16]);
        keep.extend_from_slice(&epub[c + 34..c + 42]);
        keep.extend_from_slice(&epub[c + 46..c + 46 + cn + cx + cc]);
        keep
    };
    assert_eq!(kept(&c.epub), kept(&input));
    let z = ZipArchive::new(Cursor::new(c.epub.as_slice())).unwrap();
    assert_eq!(z.comment(), b"an archive comment");
    assert!(entry(&c.epub, "OEBPS/Text/c0.xhtml").contains("kobo.1.1"));
}
