//! Cutting a run of raw character data into sentence segments
//! (docs/SPANS.md, "Sentence segments").
//!
//! The run is the document's own bytes, references and CDATA sections and
//! all. Characters are classified after references are decoded, but every
//! cut is a byte offset in the raw run that falls between two characters or
//! references, so a reference is never split and never rewritten. A CDATA
//! section's content is plain characters; a cut may fall inside it, and the
//! segment says so, since a tag cannot be written there as it is.

use std::ops::Range;

/// One segment of a run: its byte range within the run, whether it holds
/// only whitespace, and whether its start or end falls inside a CDATA
/// section's content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub range: Range<usize>,
    pub start_in_cdata: bool,
    pub end_in_cdata: bool,
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
    /// Where a cut before this unit goes, and whether that is inside a CDATA
    /// section. A cut before the first character of a section goes before
    /// the section, so the section is not split there.
    cut: (usize, bool),
}

/// Cut `raw` (character data and CDATA sections, no other markup) into
/// segments that, joined, reproduce it exactly. A run with no character in
/// it (empty, or only empty CDATA sections) has no segments.
pub fn segments(raw: &str) -> Vec<Segment> {
    let units = decode(raw);
    if units.is_empty() {
        return Vec::new();
    }
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
            cuts.push(units[m].cut);
            i = m;
        } else {
            i = j;
        }
    }

    let mut out = Vec::with_capacity(cuts.len() + 1);
    let mut start = (0, false);
    for end in cuts.into_iter().chain([(raw.len(), false)]) {
        let blank = units
            .iter()
            .filter(|u| u.range.start >= start.0 && u.range.end <= end.0)
            .all(|u| u.ch.is_some_and(char::is_whitespace));
        out.push(Segment {
            range: start.0..end.0,
            start_in_cdata: start.1,
            end_in_cdata: end.1,
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

const CDATA_OPEN: &str = "<![CDATA[";
const CDATA_CLOSE: &str = "]]>";

fn decode(raw: &str) -> Vec<Unit> {
    let mut units = Vec::with_capacity(raw.len());
    let mut chars = raw.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        if raw[at..].starts_with(CDATA_OPEN) {
            let content = at + CDATA_OPEN.len();
            let end = raw[content..]
                .find(CDATA_CLOSE)
                .map_or(raw.len(), |i| content + i);
            for (i, c) in raw[content..end].char_indices() {
                let p = content + i;
                units.push(Unit {
                    ch: Some(c),
                    range: p..p + c.len_utf8(),
                    cut: if i == 0 { (at, false) } else { (p, true) },
                });
            }
            let after = (end + CDATA_CLOSE.len()).min(raw.len());
            while chars.peek().is_some_and(|&(p, _)| p < after) {
                chars.next();
            }
            continue;
        }
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
                cut: (at, false),
            });
            continue;
        }
        units.push(Unit {
            ch: Some(c),
            range: at..at + c.len_utf8(),
            cut: (at, false),
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
        assert!(segments("<![CDATA[]]>").is_empty());
        assert!(segments("<![CDATA[]]><![CDATA[]]>").is_empty());
    }

    #[test]
    fn a_cdata_section_is_plain_text() {
        let raw = "abc<![CDATA[x. y]]>def. g";
        assert_eq!(cut(raw), ["abc<![CDATA[x. ", "y]]>def. ", "g"]);
        let s = segments(raw);
        assert_eq!(
            s.iter()
                .map(|s| (s.start_in_cdata, s.end_in_cdata))
                .collect::<Vec<_>>(),
            [(false, true), (true, false), (false, false)]
        );
        // `&` inside a section is a letter, not a reference.
        assert_eq!(
            cut("<![CDATA[a &#46; b. c]]>"),
            ["<![CDATA[a &#46; b. ", "c]]>"]
        );
    }

    #[test]
    fn a_cut_before_a_section_goes_before_it() {
        let raw = "a. <![CDATA[b. c]]> d";
        assert_eq!(cut(raw), ["a. ", "<![CDATA[b. ", "c]]> d"]);
        let s = segments(raw);
        assert!(!s[1].start_in_cdata && s[1].end_in_cdata && s[2].start_in_cdata);
    }

    #[test]
    fn whitespace_in_a_section_is_blank() {
        assert!(segments("<![CDATA[   ]]>")[0].blank);
        assert!(!segments("<![CDATA[ x ]]>")[0].blank);
    }
}
