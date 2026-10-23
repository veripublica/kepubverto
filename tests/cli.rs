//! The CLI's `--format json` envelope (FORMATS §1), run as a user runs it.

use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

const HEAD: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE html>\n<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>T</title></head>";

/// An EPUB 3 book with one content document per body.
fn book(bodies: &[&str]) -> Vec<u8> {
    let items: String = (0..bodies.len())
        .map(|i| {
            format!(r#"<item id="c{i}" href="c{i}.xhtml" media-type="application/xhtml+xml"/>"#)
        })
        .collect();
    let spine: String = (0..bodies.len())
        .map(|i| format!(r#"<itemref idref="c{i}"/>"#))
        .collect();
    let opf = format!(
        r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">urn:x</dc:identifier><dc:title>T</dc:title><dc:language>en</dc:language></metadata><manifest>{items}</manifest><spine>{spine}</spine></package>"#
    );
    let mut z = ZipWriter::new(Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    z.start_file("mimetype", stored).unwrap();
    z.write_all(b"application/epub+zip").unwrap();
    z.start_file("META-INF/container.xml", SimpleFileOptions::default())
        .unwrap();
    z.write_all(br#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#).unwrap();
    z.start_file("OEBPS/content.opf", SimpleFileOptions::default())
        .unwrap();
    z.write_all(opf.as_bytes()).unwrap();
    for (i, body) in bodies.iter().enumerate() {
        z.start_file(format!("OEBPS/c{i}.xhtml"), SimpleFileOptions::default())
            .unwrap();
        z.write_all(format!("{HEAD}<body>{body}</body></html>\n").as_bytes())
            .unwrap();
    }
    z.finish().unwrap().into_inner()
}

/// A fresh directory for one test, under Cargo's per-target temp dir.
fn dir(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kepubverto"))
        .args(args)
        .output()
        .unwrap()
}

fn envelope(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).expect("stdout is one JSON object")
}

#[test]
fn a_converted_book_is_ok_with_its_output_and_summary() {
    let d = dir("json-ok");
    let input = d.join("book.epub");
    std::fs::write(&input, book(&["<p>One. Two.</p>"])).unwrap();
    let out = run(&["-i", input.to_str().unwrap(), "--format", "json"]);
    assert_eq!(out.status.code(), Some(0));
    let env = envelope(&out);
    assert_eq!(env["tool"], "kepubverto");
    assert_eq!(env["convention"], "0.7");
    assert_eq!(env["status"], "ok");
    assert!(env.get("dry_run").is_none(), "absent means false");
    let i = &env["inputs"][0];
    assert_eq!(i["status"], "ok");
    assert_eq!(i["output"], d.join("book.kepub.epub").to_str().unwrap());
    assert_eq!(
        i["summary"],
        serde_json::json!({
            "documents": 1, "converted": 1, "already_converted": 0,
            "nothing_to_do": 0, "left_alone": 0, "spans": 2
        })
    );
    assert_eq!(i["items"], serde_json::json!([]));
}

#[test]
fn a_document_left_alone_is_a_finding_and_exit_1() {
    let d = dir("json-problems");
    let input = d.join("book.epub");
    std::fs::write(&input, book(&["<p>open", "<p>x</p>"])).unwrap();
    let out = run(&["-i", input.to_str().unwrap(), "--format", "json"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stderr.is_empty(), "the envelope carries it");
    let env = envelope(&out);
    assert_eq!(env["status"], "problems");
    let i = &env["inputs"][0];
    assert_eq!(i["status"], "problems");
    assert_eq!(i["summary"]["left_alone"], 1);
    assert_eq!(i["summary"]["converted"], 1);
    let items = i["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["type"], "finding");
    assert!(items[0].get("outcome").is_none());
    assert_eq!(items[0]["code"], "RSC-016");
    assert_eq!(items[0]["severity"], "fatal");
    assert_eq!(items[0]["location"], "OEBPS/c0.xhtml");
    let message = items[0]["message"].as_str().unwrap();
    assert!(message.starts_with("Left as it was: "), "{message}");
}

#[test]
fn a_file_that_is_not_an_epub_is_an_error_input_and_exit_2() {
    let d = dir("json-error");
    let input = d.join("book.epub");
    std::fs::write(&input, b"not a zip").unwrap();
    let out = run(&["-i", input.to_str().unwrap(), "--format", "json"]);
    assert_eq!(out.status.code(), Some(2));
    let env = envelope(&out);
    assert_eq!(env["status"], "error");
    let i = &env["inputs"][0];
    assert_eq!(i["status"], "error");
    assert!(i["error"].as_str().unwrap().contains("not a ZIP"));
    assert!(i.get("summary").is_none(), "no verdict, no counters");
    assert!(i.get("output").is_none());
    assert_eq!(i["items"], serde_json::json!([]));
}

#[test]
fn a_refusal_or_a_usage_error_prints_no_envelope() {
    let d = dir("json-refused");
    let input = d.join("book.epub");
    std::fs::write(&input, book(&["<p>x</p>"])).unwrap();
    std::fs::write(d.join("book.kepub.epub"), b"exists").unwrap();
    let i = input.to_str().unwrap();
    for args in [
        vec!["-i", i, "--format", "json"], // output exists, no -f
        vec!["-i", i, "-o", i, "-f", "--format", "json"], // output = input
        vec!["-i", i, "--format", "json", "--hyphens", "sometimes"], // usage error
        vec![
            "-i",
            i,
            "-f",
            "--format",
            "json",
            "--css",
            "p { position: fixed }",
        ], // refused css
    ] {
        let out = run(&args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(out.stdout.is_empty(), "{args:?}");
        assert!(out.stderr.starts_with(b"error: "), "{args:?}");
    }
}

#[test]
fn help_claims_convention_0_7() {
    let out = run(&["--help"]);
    let help = String::from_utf8(out.stdout).unwrap();
    assert!(help.contains("Conforms to veripublica conventions v0.7."));
    assert!(help.contains("--format <FORMAT>"));
}

#[test]
fn a_refused_css_writes_nothing_and_says_why() {
    let d = dir("css-refused");
    let input = d.join("book.epub");
    std::fs::write(&input, book(&["<p>x</p>"])).unwrap();
    let i = input.to_str().unwrap();
    let out = run(&[
        "-i",
        i,
        "--css",
        "p { margin: 0 }",
        "--css",
        "a { background: url(x.png) }",
    ]);
    assert_eq!(out.status.code(), Some(2));
    assert!(!d.join("book.kepub.epub").exists());
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "error: --css value 2: line 1, column 17: refers to 'x.png'; extra CSS cannot refer to files\n"
    );
}
