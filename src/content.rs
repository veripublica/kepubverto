//! Converting one XHTML content document (docs/SPANS.md).
//!
//! The document is parsed only to find byte offsets. The output is the
//! original text with the spans, the two divs and the style element inserted
//! at those offsets; every other byte is the input's.

use std::ops::Range;

use roxmltree::{Document, Node, ParsingOptions};

use crate::segment;

const XHTML_NS: &str = "http://www.w3.org/1999/xhtml";

/// The element appended to `<head>`, byte-identical to kepubify 4.0.4's.
pub const STYLE: &str = r#"<style type="text/css" class="kobostylehacks">div#book-inner { margin-top: 0; margin-bottom: 0;}</style>"#;
const DIVS_OPEN: &str = r#"<div id="book-columns"><div id="book-inner">"#;
const DIVS_CLOSE: &str = "</div></div>";

/// What converting one document came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The document was rewritten.
    Converted {
        /// The `koboSpan` elements added.
        spans: usize,
    },
    /// It already holds a `koboSpan`, so it was converted before. Left as is.
    AlreadyConverted,
    /// There was nothing to add (no text, no image, divs and style present).
    NothingToDo,
    /// The document could not be converted and was left as it was.
    LeftAlone(Reason),
}

/// Why a document was left as it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// The bytes are not UTF-8.
    NotUtf8,
    /// The family's XML safety limits refused it before parsing.
    TooLarge(String),
    /// It is not well-formed XML.
    NotWellFormed(String),
    /// No `<html>` root with a `<body>` in the XHTML namespace.
    NoBody,
    /// It already uses an id the conversion would add.
    IdInUse(String),
    /// An entity it declares itself expands to markup, so the parsed tree
    /// does not map onto the bytes.
    MarkupInEntity,
    /// The manifest names it, but the container has no such entry.
    Missing,
    /// The entry could not be read out of the container.
    Unreadable(String),
}

impl std::fmt::Display for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Reason::NotUtf8 => f.write_str("it is not UTF-8"),
            Reason::TooLarge(why) => write!(f, "{why}"),
            Reason::NotWellFormed(why) => write!(f, "it is not well-formed XML: {why}"),
            Reason::NoBody => f.write_str("it has no XHTML <body>"),
            Reason::IdInUse(id) => write!(f, "it already uses the id '{id}'"),
            Reason::MarkupInEntity => f.write_str("an entity it declares expands to markup"),
            Reason::Missing => {
                f.write_str("the manifest names it, but the container has no such file")
            }
            Reason::Unreadable(why) => write!(f, "it could not be read: {why}"),
        }
    }
}

/// Convert one content document. `epub3` says which entity rules apply: an
/// EPUB 2 document may use the XHTML DTD's named entities. Returns the
/// outcome, and the new bytes when the document was rewritten.
pub fn convert(bytes: &[u8], epub3: bool) -> (Outcome, Option<Vec<u8>>) {
    match convert_text(bytes, epub3) {
        Ok(Some((out, spans))) => (Outcome::Converted { spans }, Some(out.into_bytes())),
        Ok(None) => (Outcome::NothingToDo, None),
        Err(Stop::Already) => (Outcome::AlreadyConverted, None),
        Err(Stop::Leave(reason)) => (Outcome::LeftAlone(reason), None),
    }
}

enum Stop {
    Already,
    Leave(Reason),
}

impl From<Reason> for Stop {
    fn from(r: Reason) -> Self {
        Stop::Leave(r)
    }
}

fn convert_text(bytes: &[u8], epub3: bool) -> Result<Option<(String, usize)>, Stop> {
    let original = std::str::from_utf8(bytes).map_err(|_| Reason::NotUtf8)?;
    // The XHTML DTD's entities are declared for parsing only. Offsets in the
    // parsed text past the insertion are mapped back by `shift`.
    let (text, shift) = epubveri::htm::declare_dtd_entities(original.to_string(), epub3);
    epubveri::xmlguard::check(&text).map_err(|r| Reason::TooLarge(r.to_string()))?;
    let opts = ParsingOptions {
        allow_dtd: true,
        ..ParsingOptions::default()
    };
    let doc = Document::parse_with_options(&text, opts)
        .map_err(|e| Reason::NotWellFormed(e.to_string()))?;

    let root = doc.root_element();
    if !is_xhtml(root, "html") {
        return Err(Reason::NoBody.into());
    }
    let head = root.children().find(|n| is_xhtml(*n, "head"));
    let body = root
        .children()
        .find(|n| is_xhtml(*n, "body"))
        .ok_or(Reason::NoBody)?;

    if root.descendants().any(|n| has_class(n, "koboSpan")) {
        return Err(Stop::Already);
    }
    let has_divs = body.children().any(|c| {
        is_div_with_id(c, "book-columns") && c.children().any(|g| is_div_with_id(g, "book-inner"))
    });
    for n in root.descendants() {
        if let Some(id) = attr(n, "id")
            && ((!has_divs && (id == "book-columns" || id == "book-inner")) || is_kobo_id(id))
        {
            return Err(Reason::IdInUse(id.to_string()).into());
        }
    }

    let mut w = Walk {
        text: &text,
        epub3,
        body_wrapped: !has_divs,
        p: 0,
        s: 0,
        pending: false,
        spans: 0,
        edits: Vec::new(),
    };
    let body_content = content_range(&text, body.range());
    if let Some(content) = &body_content {
        w.contents(body, content.clone())?;
        if !has_divs {
            w.edits
                .push(Edit::new(content.start, Order::First, DIVS_OPEN));
            w.edits
                .push(Edit::new(content.end, Order::Last, DIVS_CLOSE));
        }
    }
    if let Some(head) = head
        && !head.descendants().any(|n| has_class(n, "kobostylehacks"))
        && let Some(content) = content_range(&text, head.range())
    {
        w.edits.push(Edit::new(content.end, Order::Last, STYLE));
    }

    if w.edits.is_empty() {
        return Ok(None);
    }
    let spans = w.spans;
    let mut edits = w.edits;
    if let Some(shift) = shift {
        for e in &mut edits {
            // Every edit is in <head> or <body>, after the DOCTYPE that took the
            // declarations, so each one moves back by the same amount.
            debug_assert!(e.at >= shift.at + shift.len);
            e.at -= shift.len;
        }
    }
    // Stable: edits at one offset keep the order the walk made them in.
    edits.sort_by_key(|e| (e.at, e.order));
    let extra: usize = edits.iter().map(|e| e.text.len()).sum();
    let mut out = String::with_capacity(original.len() + extra);
    let mut from = 0;
    for e in &edits {
        out.push_str(&original[from..e.at]);
        out.push_str(&e.text);
        from = e.at;
    }
    out.push_str(&original[from..]);
    Ok(Some((out, spans)))
}

/// Where at one offset an insertion goes: the divs' opening tags before
/// everything else there, their closing tags after everything else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Order {
    First,
    Walk,
    Last,
}

struct Edit {
    at: usize,
    order: Order,
    text: String,
}

impl Edit {
    fn new(at: usize, order: Order, text: impl Into<String>) -> Self {
        Edit {
            at,
            order,
            text: text.into(),
        }
    }
}

/// Where a text run sits: directly in a `p`, and whether its parent may hold
/// a `span`.
#[derive(Clone, Copy)]
struct Text {
    in_p: bool,
    wrap: bool,
}

/// The state of the walk over `<body>` (docs/SPANS.md, "Span numbering").
struct Walk<'t> {
    text: &'t str,
    epub3: bool,
    /// The divs are added, so text directly in `<body>` ends up in a div.
    body_wrapped: bool,
    p: usize,
    s: usize,
    pending: bool,
    spans: usize,
    edits: Vec<Edit>,
}

impl Walk<'_> {
    fn element(&mut self, n: Node) -> Result<(), Reason> {
        let name = n.tag_name().name();
        // svg and math are skipped in whatever namespace they come.
        if matches!(name, "svg" | "math") {
            return Ok(());
        }
        if n.tag_name().namespace() == Some(XHTML_NS) {
            match name {
                "script" | "style" | "pre" | "audio" | "video" => {
                    return Ok(());
                }
                "img" => {
                    self.image(n);
                    return Ok(());
                }
                "p" | "ol" | "ul" | "table" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    self.pending = true;
                }
                _ => {}
            }
        }
        // `<p/>` sets pending like `<p></p>`; there is just nothing to walk.
        match content_range(self.text, n.range()) {
            Some(content) => self.contents(n, content),
            None => Ok(()),
        }
    }

    fn image(&mut self, n: Node) {
        self.p += 1;
        self.s = 1;
        self.pending = false;
        if n.parent().is_none_or(|p| self.holds_span(p)) {
            let r = n.range();
            self.open_span(r.start);
            self.close_span(r.end);
        }
    }

    /// Whether a `span` is allowed as a child of `n`. Where it is not, the
    /// counters still move as if it had been added, so every other id stays
    /// kepubify's, and nothing is inserted (docs/SPANS.md, "Where a span
    /// would be invalid").
    fn holds_span(&self, n: Node) -> bool {
        if n.tag_name().namespace() != Some(XHTML_NS) {
            return true;
        }
        match n.tag_name().name() {
            "option" | "textarea" | "select" | "optgroup" | "datalist" | "picture" | "table"
            | "thead" | "tbody" | "tfoot" | "tr" | "colgroup" | "ul" | "ol" | "dl" | "hgroup"
            | "html" | "head" => false,
            // XHTML 1.1 allows only block content in these.
            "blockquote" | "form" | "noscript" | "map" => self.epub3,
            "body" => self.epub3 || self.body_wrapped,
            _ => true,
        }
    }

    /// Walk the content of `n`, which spans `content`: its child elements in
    /// order, and the character data between them.
    fn contents(&mut self, n: Node, content: Range<usize>) -> Result<(), Reason> {
        let at_text = Text {
            in_p: is_xhtml(n, "p"),
            wrap: self.holds_span(n),
        };
        let mut at = content.start;
        for child in n.children().filter(Node::is_element) {
            let r = child.range();
            // An element whose range is not inside its parent's content, in
            // order, came from an entity's replacement text.
            if r.start < at || r.end > content.end {
                return Err(Reason::MarkupInEntity);
            }
            self.character_data(at..r.start, at_text);
            self.element(child)?;
            at = r.end;
        }
        self.character_data(at..content.end, at_text);
        Ok(())
    }

    /// Character data between two elements. Comments, CDATA sections and
    /// processing instructions end a text run and are never wrapped.
    fn character_data(&mut self, range: Range<usize>, t: Text) {
        let mut at = range.start;
        let mut run = at;
        while at < range.end {
            let rest = &self.text[at..range.end];
            let skip = [("<!--", "-->"), ("<![CDATA[", "]]>"), ("<?", "?>")]
                .iter()
                .find(|(open, _)| rest.starts_with(open))
                .map(|(open, close)| {
                    rest[open.len()..]
                        .find(close)
                        .map_or(rest.len(), |i| open.len() + i + close.len())
                });
            match skip {
                Some(len) => {
                    self.text_run(run..at, t);
                    at += len;
                    run = at;
                }
                None => at += rest.chars().next().map_or(1, char::len_utf8),
            }
        }
        self.text_run(run..range.end, t);
    }

    fn text_run(&mut self, range: Range<usize>, t: Text) {
        for seg in segment::segments(&self.text[range.clone()]) {
            if seg.blank && !t.in_p {
                continue;
            }
            if self.pending {
                self.p += 1;
                self.s = 0;
                self.pending = false;
            }
            self.s += 1;
            if !t.wrap {
                continue;
            }
            self.open_span(range.start + seg.range.start);
            self.close_span(range.start + seg.range.end);
        }
    }

    fn open_span(&mut self, at: usize) {
        self.spans += 1;
        let tag = format!(r#"<span class="koboSpan" id="kobo.{}.{}">"#, self.p, self.s);
        self.edits.push(Edit::new(at, Order::Walk, tag));
    }

    fn close_span(&mut self, at: usize) {
        self.edits.push(Edit::new(at, Order::Walk, "</span>"));
    }
}

/// The byte range between an element's start tag and its end tag, or `None`
/// for an empty-element tag (`<x/>`). The document is well-formed, so the
/// start tag ends at the first `>` outside a quoted attribute value, and the
/// end tag is the last `</` in the element.
fn content_range(text: &str, element: Range<usize>) -> Option<Range<usize>> {
    let tag = &text[element.clone()];
    let mut quote = None;
    let mut end = None;
    for (i, b) in tag.bytes().enumerate() {
        match (quote, b) {
            (None, b'"' | b'\'') => quote = Some(b),
            (Some(q), _) if q == b => quote = None,
            (None, b'>') => {
                end = Some(i);
                break;
            }
            _ => {}
        }
    }
    let end = end?;
    if tag.as_bytes()[end - 1] == b'/' {
        return None;
    }
    let close = tag.rfind("</")?;
    Some(element.start + end + 1..element.start + close)
}

fn is_xhtml(n: Node, name: &str) -> bool {
    n.is_element() && n.tag_name().name() == name && n.tag_name().namespace() == Some(XHTML_NS)
}

/// An attribute in no namespace (roxmltree's `attribute(name)` would also
/// match `xml:id`).
fn attr<'a>(n: Node<'a, '_>, name: &str) -> Option<&'a str> {
    n.attributes()
        .find(|a| a.namespace().is_none() && a.name() == name)
        .map(|a| a.value())
}

fn has_class(n: Node, token: &str) -> bool {
    attr(n, "class").is_some_and(|c| c.split_ascii_whitespace().any(|t| t == token))
}

fn is_div_with_id(n: Node, id: &str) -> bool {
    is_xhtml(n, "div") && attr(n, "id") == Some(id)
}

/// `kobo.N.N`, the shape of an id the conversion adds.
fn is_kobo_id(id: &str) -> bool {
    let Some(rest) = id.strip_prefix("kobo.") else {
        return false;
    };
    let mut parts = rest.split('.');
    let digits =
        |s: Option<&str>| s.is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()));
    digits(parts.next()) && digits(parts.next()) && parts.next().is_none()
}
