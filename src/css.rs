//! The `--css` values (docs/SPANS.md, "Extra CSS"): checked, then written
//! back as text an XHTML `<style>` can hold as it stands.
//!
//! The check is on the argument, never on the book. Its rules are epubveri's
//! (`check_style_text`), so the CSS kepubverto writes is graded by the same
//! code that grades the converted book.

use epubveri::report::Position;

/// The label every finding about a value is located at.
const LABEL: &str = "--css";

/// A `--css` value kepubverto will not write into a book.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CssError {
    /// Which value, counting from 1, in the order given.
    pub value: usize,
    pub reason: CssReason,
}

/// Why a value was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CssReason {
    /// No CSS at all: empty, or only whitespace and comments.
    Empty,
    /// A `<` or `&` that is CSS syntax, a `<!--` or a `]]>`: it cannot be
    /// written without XML escaping. `what` is styloria's name for it.
    NotXmlSafe { at: (u32, u32), what: String },
    /// A url (`url()`, `src()`, `@import`, `image-set()`).
    Url { at: (u32, u32), url: String },
    /// An `@font-face` rule.
    FontFace { at: Option<(u32, u32)> },
    /// What epubveri reports about the value.
    Findings(Vec<CssFinding>),
}

/// One finding epubveri reports about a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CssFinding {
    /// epubveri's id (`CSS-008`, `ADV-001`, …).
    pub id: String,
    /// `error`, `warning`, `usage`, …
    pub severity: String,
    pub text: String,
    /// Line and column in the value.
    pub at: Option<(u32, u32)>,
}

impl std::fmt::Display for CssError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "--css value {}: {}", self.value, self.reason)
    }
}

impl std::error::Error for CssError {}

/// The reason alone, with no value number: for a caller that names the value
/// its own way (the web page's "Extra CSS").
impl std::fmt::Display for CssReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let at = |(line, column): (u32, u32)| format!("line {line}, column {column}: ");
        match self {
            CssReason::Empty => f.write_str("holds no CSS"),
            CssReason::NotXmlSafe { at: p, what } => write!(
                f,
                "{}{what} cannot be written inside a <style> without XML escaping",
                at(*p)
            ),
            CssReason::Url { at: p, url } => write!(
                f,
                "{}refers to '{url}'; extra CSS cannot refer to files",
                at(*p)
            ),
            CssReason::FontFace { at: p } => write!(
                f,
                "{}@font-face is not accepted; fonts come from the book or the \
                 device, so use font-family with a font the book or the device has",
                p.map(at).unwrap_or_default()
            ),
            CssReason::Findings(findings) => {
                f.write_str("would add a finding to the book")?;
                for x in findings {
                    write!(
                        f,
                        "\n  {}{} {}: {}",
                        x.at.map(at).unwrap_or_default(),
                        x.severity.to_uppercase(),
                        x.id,
                        x.text
                    )?;
                }
                Ok(())
            }
        }
    }
}

/// The text of the one `<style>` the values go into, or why a value is
/// refused. `epub3` picks epubveri's rules: EPUB 3 flags properties that
/// EPUB 2 does not.
pub fn prepare(values: &[String], epub3: bool) -> Result<String, CssError> {
    let mut out = Vec::with_capacity(values.len());
    for (i, value) in values.iter().enumerate() {
        let refuse = |reason| CssError {
            value: i + 1,
            reason,
        };
        out.push(prepare_one(value, epub3).map_err(refuse)?);
    }
    Ok(out.join("\n"))
}

fn prepare_one(css: &str, epub3: bool) -> Result<String, CssReason> {
    let (sheet, _) = styloria::parse_stylesheet(css);
    let written = styloria::serialize_stylesheet_for_xml(&sheet).map_err(|e| {
        let what = match e.kind {
            styloria::XmlSafeErrorKind::Delimiter(c) => format!("a '{c}'"),
            styloria::XmlSafeErrorKind::Cdo => "a '<!--'".to_string(),
            styloria::XmlSafeErrorKind::BadUrl => "a malformed url".to_string(),
            styloria::XmlSafeErrorKind::CdataEnd => "a ']]>'".to_string(),
        };
        CssReason::NotXmlSafe {
            at: e.span.start_line_col(css),
            what,
        }
    })?;
    if written.trim().is_empty() {
        return Err(CssReason::Empty);
    }
    check(css, epub3)?;
    // What is written is checked too. The serialization means the same CSS
    // (styloria's promise); this makes sure of it before it goes into a book.
    check(&written, epub3)?;
    Ok(written)
}

/// Refuse a url, then epubveri's findings about `css` as a `<style>`,
/// advisory checks included.
fn check(css: &str, epub3: bool) -> Result<(), CssReason> {
    if let Some(u) = epubveri::style_text_urls(css).into_iter().next() {
        return Err(CssReason::Url {
            at: pair(u.position),
            url: u.url,
        });
    }
    findings(css, epub3)
}

fn findings(css: &str, epub3: bool) -> Result<(), CssReason> {
    let options = epubveri::Options {
        advisory: true,
        epub_version: Some(if epub3 { "3" } else { "2" }.to_string()),
        ..epubveri::Options::default()
    };
    let report = epubveri::check_style_text(css, LABEL, &options);
    if report.messages.is_empty() {
        return Ok(());
    }
    // CSS-028 notes every @font-face, CSS-019 an empty one.
    if let Some(m) = report
        .messages
        .iter()
        .find(|m| matches!(m.id, "CSS-028" | "CSS-019"))
    {
        return Err(CssReason::FontFace {
            at: m.position.map(pair),
        });
    }
    Err(CssReason::Findings(
        report
            .messages
            .into_iter()
            .map(|m| CssFinding {
                id: m.id.to_string(),
                severity: m.severity.as_str().to_string(),
                text: m.text,
                at: m.position.map(pair),
            })
            .collect(),
    ))
}

fn pair(p: Position) -> (u32, u32) {
    (p.line, p.column)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(css: &str) -> Result<String, CssReason> {
        prepare_one(css, true)
    }

    #[test]
    fn plain_css_is_written_back_as_the_same_css() {
        assert_eq!(
            one("p { text-indent: 1em; }  /* a note */").unwrap(),
            "p {text-indent:1em;}"
        );
    }

    #[test]
    fn values_share_one_element_in_order() {
        let values = [
            "p { margin: 0 }".to_string(),
            "h1 { color: red }".to_string(),
        ];
        assert_eq!(
            prepare(&values, true).unwrap(),
            "p {margin:0;}\nh1 {color:red;}"
        );
    }

    #[test]
    fn markup_characters_in_text_are_css_escaped() {
        let w = one(r#"p::before { content: "a<b & c" }"#).unwrap();
        assert!(!w.contains('<') && !w.contains('&'), "{w}");
        assert!(w.contains(r"\3c ") && w.contains(r"\26 "), "{w}");
        // A comment holding them is dropped, so it cannot break the document.
        assert_eq!(
            one("p { margin: 0 } /* & </style> */").unwrap(),
            "p {margin:0;}"
        );
    }

    #[test]
    fn markup_characters_that_are_syntax_are_refused() {
        for css in [
            "@media (width < 600px) { p { margin: 0 } }",
            "p { & span { margin: 0 } }",
        ] {
            assert!(
                matches!(one(css), Err(CssReason::NotXmlSafe { .. })),
                "{css}: {:?}",
                one(css)
            );
        }
    }

    #[test]
    fn a_url_is_refused() {
        assert_eq!(
            one("body { background: url(bg.png) }"),
            Err(CssReason::Url {
                at: (1, 20),
                url: "bg.png".into()
            })
        );
        assert!(matches!(
            one("@import 'a.css';"),
            Err(CssReason::Url { .. })
        ));
    }

    #[test]
    fn font_face_is_refused_with_its_own_reason() {
        assert!(matches!(
            one("@font-face { font-family: X; src: local(Georgia) }"),
            Err(CssReason::FontFace { .. })
        ));
    }

    #[test]
    fn a_finding_is_refused_and_shown() {
        let Err(CssReason::Findings(f)) = one("p { position: fixed }") else {
            panic!("{:?}", one("p { position: fixed }"));
        };
        assert_eq!(f[0].id, "CSS-006");
        // Advisory checks are on: an unknown property is a finding.
        assert!(matches!(
            one("p { colour: red }"),
            Err(CssReason::Findings(_))
        ));
    }

    #[test]
    fn the_rules_follow_the_epub_version() {
        // EPUB 3 flags `direction` (CSS-001); EPUB 2 does not.
        assert!(prepare_one("p { direction: rtl }", false).is_ok());
        assert!(matches!(
            prepare_one("p { direction: rtl }", true),
            Err(CssReason::Findings(_))
        ));
    }

    #[test]
    fn nothing_to_write_is_refused() {
        for css in ["", "   ", "/* only a comment */"] {
            assert_eq!(one(css), Err(CssReason::Empty), "{css:?}");
        }
    }

    #[test]
    fn the_message_names_the_value_and_the_place() {
        let values = [
            "p { margin: 0 }".to_string(),
            "p { background: url(a.png) }".to_string(),
        ];
        let e = prepare(&values, true).unwrap_err();
        assert_eq!(
            e.to_string(),
            "--css value 2: line 1, column 17: refers to 'a.png'; extra CSS cannot refer to files"
        );
    }
}
