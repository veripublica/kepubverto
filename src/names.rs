//! File names in the table of contents (docs/SPANS.md).
//!
//! Kobo's renderer matches the NCX and the navigation document to the manifest
//! by the file name as written. Where a reference and the manifest spell one
//! file differently, the book is counted twice. This writes both in one valid
//! spelling: the manifest's, with any character a URL must encode encoded.
//! Only those attribute values change; every other byte stays as it was.
//! Nothing is validated: a document that does not parse is left alone.

use std::collections::BTreeMap;
use std::ops::Range;

use roxmltree::{Document, Node, ParsingOptions};

use crate::container::{Container, percent_decode, rootfile};

const NCX_TYPE: &str = "application/x-dtbncx+xml";
const XHTML_NS: &str = "http://www.w3.org/1999/xhtml";

/// The package document, the NCX and the navigation document with their
/// references respelled: container path → new text. Empty when every name
/// already agrees, or when a document cannot be read or parsed.
pub(crate) fn respell(c: &mut Container) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let Ok(opf_path) = rootfile(c) else {
        return out;
    };
    let Some(opf) = utf8(c, &opf_path) else {
        return out;
    };
    let Some(doc) = parse(&opf) else {
        return out;
    };
    let root = doc.root_element();
    let epub3 = root
        .attribute("version")
        .is_some_and(|v| v.trim().starts_with('3'));
    let opf_base = dir(&opf_path);

    let manifest: Vec<Node> = root
        .descendants()
        .filter(|n| {
            n.has_tag_name("item") && n.parent().is_some_and(|p| p.has_tag_name("manifest"))
        })
        .collect();
    let toc_id = root
        .descendants()
        .find(|n| n.has_tag_name("spine"))
        .and_then(|s| s.attribute("toc"));
    let ncx = manifest
        .iter()
        .find(|i| toc_id.is_some() && i.attribute("id") == toc_id)
        .or_else(|| manifest.iter().find(|i| media_type(i) == NCX_TYPE))
        .filter(|i| media_type(i) == NCX_TYPE);
    let nav = epub3
        .then(|| {
            manifest.iter().find(|i| {
                i.attribute("properties")
                    .is_some_and(|p| p.split_ascii_whitespace().any(|t| t == "nav"))
            })
        })
        .flatten();

    // Each manifest item by its decoded path. Two items that decode to one
    // path are ambiguous, and neither is matched.
    let mut items: Vec<Item> = Vec::new();
    let mut by_path: BTreeMap<String, Option<usize>> = BTreeMap::new();
    for node in &manifest {
        let Some(attr) = node.attributes().find(|a| a.name() == "href") else {
            continue;
        };
        let (path, _) = split_fragment(attr.value());
        let raw = resolve(&opf_base, path);
        let Some(raw) = raw else { continue };
        let key = percent_decode(&raw.join("/"));
        let i = items.len();
        items.push(Item {
            href: attr.value().to_string(),
            range: attr.range_value(),
            raw,
        });
        by_path
            .entry(key)
            .and_modify(|v| *v = None)
            .or_insert(Some(i));
    }

    let mut toc_docs: Vec<TocDoc> = Vec::new();
    for (node, nav) in [(ncx, false), (nav, true)] {
        let Some(href) = node.and_then(|n| n.attribute("href")) else {
            continue;
        };
        let (path, _) = split_fragment(href);
        let Some(segs) = resolve(&opf_base, path) else {
            continue;
        };
        let path = percent_decode(&segs.join("/"));
        let Some(text) = utf8(c, &path) else {
            continue;
        };
        toc_docs.push(TocDoc { path, text, nav });
    }

    // References, by the item they name.
    let mut refs: Vec<Ref> = Vec::new();
    for (d, td) in toc_docs.iter().enumerate() {
        let Some(doc) = parse(&td.text) else { continue };
        let base = dir(&td.path);
        for n in doc.descendants().filter(|n| n.is_element()) {
            let attr = if td.nav {
                (n.tag_name().name() == "a"
                    && n.tag_name().namespace() == Some(XHTML_NS)
                    && n.ancestors().any(|a| {
                        a.tag_name().name() == "nav" && a.tag_name().namespace() == Some(XHTML_NS)
                    }))
                .then(|| {
                    n.attributes()
                        .find(|a| a.name() == "href" && a.namespace().is_none())
                })
                .flatten()
            } else {
                (n.tag_name().name() == "content")
                    .then(|| {
                        n.attributes()
                            .find(|a| a.name() == "src" && a.namespace().is_none())
                    })
                    .flatten()
            };
            let Some(attr) = attr else { continue };
            let (path, _) = split_fragment(attr.value());
            let Some(tracked) = resolve_tracked(&base, path) else {
                continue;
            };
            let raw: Vec<String> = tracked.iter().map(|(s, _)| s.clone()).collect();
            let key = percent_decode(&raw.join("/"));
            let Some(Some(item)) = by_path.get(&key).copied() else {
                continue;
            };
            refs.push(Ref {
                doc: d,
                value: attr.value().to_string(),
                range: attr.range_value(),
                tracked,
                item,
            });
        }
    }

    // Edits per document: (range, new value).
    let mut opf_edits: Vec<(Range<usize>, String)> = Vec::new();
    let mut doc_edits: Vec<Vec<(Range<usize>, String)>> = vec![Vec::new(); toc_docs.len()];
    for (i, item) in items.iter().enumerate() {
        let mine: Vec<&Ref> = refs.iter().filter(|r| r.item == i).collect();
        if !mine.iter().any(|r| r.raw() != item.raw) {
            continue;
        }
        let (path, fragment) = split_fragment(&item.href);
        let encoded = encode(path);
        let target = if encoded == path {
            item.raw.clone()
        } else {
            let Some(t) = resolve(&opf_base, &encoded) else {
                continue;
            };
            t
        };
        // Every reference must be respelled before anything is: one that
        // cannot be leaves the item as it was.
        let mut planned = Vec::new();
        let mut ok = true;
        for r in &mine {
            if r.raw() == target {
                continue;
            }
            match r.respelled(&target) {
                Some(v) => planned.push((r.doc, r.range.clone(), v)),
                None => {
                    ok = false;
                    break;
                }
            }
        }
        if !ok {
            continue;
        }
        if encoded != path {
            opf_edits.push((item.range.clone(), format!("{encoded}{fragment}")));
        }
        for (d, range, v) in planned {
            doc_edits[d].push((range, v));
        }
    }

    if !opf_edits.is_empty() {
        out.insert(opf_path, apply(&opf, opf_edits));
    }
    for (td, edits) in toc_docs.iter().zip(doc_edits) {
        if !edits.is_empty() {
            out.insert(td.path.clone(), apply(&td.text, edits));
        }
    }
    out
}

struct Item {
    href: String,
    range: Range<usize>,
    /// Resolved against the package document, as written.
    raw: Vec<String>,
}

struct TocDoc {
    path: String,
    text: String,
    nav: bool,
}

struct Ref {
    doc: usize,
    value: String,
    range: Range<usize>,
    /// The resolved path, as written: each segment, and the index of the
    /// reference's own segment it came from (`None` for one from its base).
    tracked: Vec<(String, Option<usize>)>,
    item: usize,
}

impl Ref {
    fn raw(&self) -> Vec<String> {
        self.tracked.iter().map(|(s, _)| s.clone()).collect()
    }

    /// The reference with each segment spelled as in `target`, its other
    /// segments, its `..` and its fragment as written. `None` when the two
    /// paths do not line up, or a segment that differs comes from the base.
    fn respelled(&self, target: &[String]) -> Option<String> {
        if target.len() != self.tracked.len() {
            return None;
        }
        let (path, fragment) = split_fragment(&self.value);
        let mut segs: Vec<String> = path.split('/').map(str::to_string).collect();
        for ((have, from), want) in self.tracked.iter().zip(target) {
            if have == want {
                continue;
            }
            segs[(*from)?] = want.clone();
        }
        Some(format!("{}{fragment}", segs.join("/")))
    }
}

fn media_type(n: &Node) -> String {
    n.attribute("media-type")
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
}

fn utf8(c: &mut Container, path: &str) -> Option<String> {
    let bytes = c.read(path).ok()??;
    String::from_utf8(bytes).ok()
}

fn parse(text: &str) -> Option<Document<'_>> {
    epubveri::xmlguard::check(text).ok()?;
    let opts = ParsingOptions {
        allow_dtd: true,
        ..ParsingOptions::default()
    };
    Document::parse_with_options(text, opts).ok()
}

fn dir(path: &str) -> String {
    path.rsplit_once('/')
        .map_or(String::new(), |(d, _)| d.to_string())
}

/// `path#fragment` → (`path`, `#fragment`).
fn split_fragment(s: &str) -> (&str, &str) {
    match s.find('#') {
        Some(i) => s.split_at(i),
        None => (s, ""),
    }
}

/// A relative reference resolved against `base`, as written (not decoded).
/// `None` for one with a scheme, or an empty one.
fn resolve(base: &str, path: &str) -> Option<Vec<String>> {
    resolve_tracked(base, path).map(|t| t.into_iter().map(|(s, _)| s).collect())
}

fn resolve_tracked(base: &str, path: &str) -> Option<Vec<(String, Option<usize>)>> {
    let path = path.trim();
    if path.is_empty() || has_scheme(path) {
        return None;
    }
    let mut out: Vec<(String, Option<usize>)> = if path.starts_with('/') {
        Vec::new()
    } else {
        base.split('/')
            .filter(|s| !s.is_empty())
            .map(|s| (s.to_string(), None))
            .collect()
    };
    for (i, seg) in path.split('/').enumerate() {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            s => out.push((s.to_string(), Some(i))),
        }
    }
    Some(out)
}

fn has_scheme(s: &str) -> bool {
    let Some(colon) = s.find(':') else {
        return false;
    };
    let scheme = &s[..colon];
    !scheme.is_empty()
        && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        && !scheme.contains('/')
}

/// `path` with every ASCII character a URL path may not hold as it is
/// percent-encoded: anything but letters, digits, `-._~!$&'()*+,;=:@/`, and a
/// `%` that starts an escape. Other characters, non-ASCII included (an IRI
/// allows them), stay.
fn encode(path: &str) -> String {
    let b = path.as_bytes();
    let mut out = String::with_capacity(path.len());
    for (i, ch) in path.char_indices() {
        let keep = !ch.is_ascii()
            || ch.is_ascii_alphanumeric()
            || "-._~!$&'()*+,;=:@/".contains(ch)
            || (ch == '%'
                && b.get(i + 1).is_some_and(u8::is_ascii_hexdigit)
                && b.get(i + 2).is_some_and(u8::is_ascii_hexdigit));
        if keep {
            out.push(ch);
        } else {
            out.push_str(&format!("%{:02X}", ch as u32));
        }
    }
    out
}

/// `text` with each range replaced by its value, written for the attribute's
/// own quotes.
fn apply(text: &str, mut edits: Vec<(Range<usize>, String)>) -> Vec<u8> {
    edits.sort_by_key(|(r, _)| r.start);
    let mut out = String::with_capacity(text.len() + 64);
    let mut at = 0;
    for (r, value) in edits {
        let quote = text.as_bytes().get(r.start.wrapping_sub(1)).copied();
        out.push_str(&text[at..r.start]);
        for ch in value.chars() {
            match ch {
                '&' => out.push_str("&#38;"),
                '<' => out.push_str("&#60;"),
                '"' if quote == Some(b'"') => out.push_str("&#34;"),
                '\'' if quote == Some(b'\'') => out.push_str("&#39;"),
                c => out.push(c),
            }
        }
        at = r.end;
    }
    out.push_str(&text[at..]);
    out.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_what_a_url_must_encode_is_encoded() {
        assert_eq!(encode("Text/a b.html"), "Text/a%20b.html");
        assert_eq!(encode("Agatha'nin_(1).html"), "Agatha'nin_(1).html");
        assert_eq!(encode("ç ü.html"), "ç%20ü.html");
        assert_eq!(encode("100%.html"), "100%25.html");
        assert_eq!(encode("a%20b.html"), "a%20b.html");
        assert_eq!(encode("a\"b|c.html"), "a%22b%7Cc.html");
    }

    #[test]
    fn a_reference_keeps_its_own_shape() {
        let r = Ref {
            doc: 0,
            value: "../Text/a%27b.html#x".into(),
            range: 0..0,
            tracked: resolve_tracked("OEBPS/Nav", "../Text/a%27b.html").unwrap(),
            item: 0,
        };
        let target = vec!["OEBPS".into(), "Text".into(), "a'b.html".into()];
        assert_eq!(r.respelled(&target).unwrap(), "../Text/a'b.html#x");
    }

    #[test]
    fn a_difference_in_the_base_is_not_respelled() {
        let r = Ref {
            doc: 0,
            value: "c.html".into(),
            range: 0..0,
            tracked: resolve_tracked("O E", "c.html").unwrap(),
            item: 0,
        };
        assert!(r.respelled(&["O%20E".into(), "c.html".into()]).is_none());
    }

    #[test]
    fn values_are_written_for_their_quotes() {
        let t = r#"<a b="x" c='y'/>"#;
        let b = t.find("x").unwrap();
        let c = t.find("y").unwrap();
        let out = apply(
            t,
            vec![(c..c + 1, "it's".into()), (b..b + 1, "a&\"'".into())],
        );
        assert_eq!(
            String::from_utf8(out).unwrap(),
            r#"<a b="a&#38;&#34;'" c='it&#39;s'/>"#
        );
    }
}
