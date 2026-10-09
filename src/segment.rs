//! Cutting a run of raw character data into sentence segments
//! (docs/SPANS.md, "Sentence segments").
//!
//! The run is the document's own bytes, references and all. Characters are
//! classified after references are decoded, but every cut is a byte offset in
//! the raw run that falls between two characters or references, so a
//! reference is never split and never rewritten.

use std::ops::Range;

/// One segment of a run: its byte range within the run, and whether it holds
/// only whitespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub range: Range<usize>,
    /// Every character is Unicode `White_Space` (NO-BREAK SPACE included).
    /// This is a wider set than the one a cut needs, on purpose: it is the
    /// set kepubify uses to decide whether a segment is wrapped.
    pub blank: bool,
}

/// A character of the run, decoded, with the bytes it came from. `None` is a
/// reference whose meaning is unknown here (an entity the document declares
/// itself): it is treated as an ordinary letter.
struct Unit {
    ch: Option<char>,
    range: Range<usize>,
}

/// Cut `raw` (character data with no markup in it) into segments that,
/// joined, reproduce it exactly. An empty run has no segments.
pub fn segments(raw: &str) -> Vec<Segment> {
    let units = decode(raw);
    let mut cuts = Vec::new();
    let mut i = 0;
    while i < units.len() {
        if !is_terminal(units[i].ch) {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < units.len() && is_terminal(units[j].ch) {
            j += 1;
        }
        let mut k = j;
        if k < units.len() && is_closer(units[k].ch) {
            k += 1;
        }
        let mut m = k;
        while m < units.len() && is_cut_space(units[m].ch) {
            m += 1;
        }
        if m > k && m < units.len() {
            cuts.push(units[m].range.start);
            i = m;
        } else {
            i = j;
        }
    }

    let mut out = Vec::with_capacity(cuts.len() + 1);
    let mut start = 0;
    for end in cuts
        .into_iter()
        .chain((!raw.is_empty()).then_some(raw.len()))
    {
        let blank = units
            .iter()
            .filter(|u| u.range.start >= start && u.range.end <= end)
            .all(|u| u.ch.is_some_and(char::is_whitespace));
        out.push(Segment {
            range: start..end,
            blank,
        });
        start = end;
    }
    out
}

/// `.` `!` `?`: a run of these may end a sentence.
fn is_terminal(c: Option<char>) -> bool {
    matches!(c, Some('.' | '!' | '?'))
}

/// At most one of these may follow the terminal run.
fn is_closer(c: Option<char>) -> bool {
    matches!(c, Some('\'' | '"' | '”' | '’' | '“' | '…'))
}

/// The whitespace a cut needs: ASCII only, NO-BREAK SPACE not included.
fn is_cut_space(c: Option<char>) -> bool {
    matches!(c, Some(' ' | '\t' | '\n' | '\r' | '\x0C'))
}

fn decode(raw: &str) -> Vec<Unit> {
    let mut units = Vec::with_capacity(raw.len());
    let mut chars = raw.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        if c == '&'
            && let Some(len) = raw[at..].find(';')
        {
            let body = &raw[at + 1..at + len];
            let end = at + len + 1;
            while chars.peek().is_some_and(|&(p, _)| p < end) {
                chars.next();
            }
            units.push(Unit {
                ch: reference(body),
                range: at..end,
            });
            continue;
        }
        units.push(Unit {
            ch: Some(c),
            range: at..at + c.len_utf8(),
        });
    }
    units
}

/// What `&body;` stands for. The document parsed, so the reference is well
/// formed; a named one that is not an XHTML entity was declared by the
/// document and is left unknown.
fn reference(body: &str) -> Option<char> {
    let code = if let Some(hex) = body.strip_prefix("#x").or_else(|| body.strip_prefix("#X")) {
        u32::from_str_radix(hex, 16).ok()
    } else if let Some(dec) = body.strip_prefix('#') {
        dec.parse().ok()
    } else {
        return epubveri::htm::xhtml_entity(body);
    };
    code.and_then(char::from_u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cut(raw: &str) -> Vec<&str> {
        segments(raw)
            .iter()
            .map(|s| &raw[s.range.clone()])
            .collect()
    }

    // The table in docs/SPANS.md, row by row.
    #[test]
    fn the_spec_table() {
        assert_eq!(cut("One. Two."), ["One. ", "Two."]);
        assert_eq!(cut("3.14 is pi."), ["3.14 is pi."]);
        assert_eq!(cut("“Stop!” she said."), ["“Stop!” ", "she said."]);
        assert_eq!(cut("Wait...\"' Then"), ["Wait...\"' Then"]);
        assert_eq!(cut("Really?! Yes."), ["Really?! ", "Yes."]);
        assert_eq!(cut("End.\u{a0}Next"), ["End.\u{a0}Next"]);
        assert_eq!(cut("End.\u{2003}Next"), ["End.\u{2003}Next"]);
        assert_eq!(cut("Mr. Smith. A.B. c"), ["Mr. ", "Smith. ", "A.B. ", "c"]);
        assert_eq!(cut("A!!! B?? C.?! D"), ["A!!! ", "B?? ", "C.?! ", "D"]);
        assert_eq!(
            cut("A… B. C’ D.’ E.“ F.” G"),
            ["A… B. ", "C’ D.’ ", "E.“ ", "F.” ", "G"]
        );
        assert_eq!(cut("End. "), ["End. "]);
    }

    #[test]
    fn whitespace_runs_stay_with_the_earlier_segment() {
        assert_eq!(cut("a.  b.\tc.\nd"), ["a.  ", "b.\t", "c.\n", "d"]);
        assert_eq!(cut("E.\r\nF"), ["E.\r\n", "F"]);
    }

    #[test]
    fn references_are_decoded_for_classification_and_kept_whole() {
        assert_eq!(cut("End.&#160;Next"), ["End.&#160;Next"]);
        assert_eq!(cut("End.&nbsp;Next"), ["End.&nbsp;Next"]);
        assert_eq!(cut("End.&#46; Z"), ["End.&#46; ", "Z"]);
        assert_eq!(cut("End.&#x2E;&#32;Z"), ["End.&#x2E;&#32;", "Z"]);
        assert_eq!(cut("Wait...&quot;' Then"), ["Wait...&quot;' Then"]);
        assert_eq!(cut("x &amp; y. z"), ["x &amp; y. ", "z"]);
        assert_eq!(cut("Hi.&rdquo; There"), ["Hi.&rdquo; ", "There"]);
    }

    #[test]
    fn an_unknown_entity_is_a_letter() {
        assert_eq!(cut("A. &custom; b"), ["A. ", "&custom; b"]);
        assert!(!segments("&custom;")[0].blank);
    }

    #[test]
    fn blank_uses_unicode_whitespace() {
        let s = segments("x. &#160;");
        assert_eq!(s.len(), 2);
        assert!(!s[0].blank);
        assert!(s[1].blank, "a segment of only NO-BREAK SPACE is blank");
        assert!(segments(" \n\t")[0].blank);
        assert!(segments("\u{2003}")[0].blank);
    }

    #[test]
    fn empty_run_has_no_segments() {
        assert!(segments("").is_empty());
    }
}
