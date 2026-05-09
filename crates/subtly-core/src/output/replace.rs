//! Find/replace dictionary applied to segment text before writers run.
//!
//! Each rule has an optional case-sensitivity and whole-word toggle.
//! Defaults (`case_sensitive: false`, `whole_word: true`) match what users
//! typically want for fixing proper-noun mistranscriptions like
//! "Eryka palm" → "Areca palm" without also rewriting substrings inside
//! unrelated words. Substitutions are applied in order; an earlier rule's
//! replacement is itself eligible for matching by later rules.

use super::Segment;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaceRule {
    pub from: String,
    pub to: String,
    /// When false, matching is ASCII-case-insensitive. Non-ASCII characters
    /// match exactly regardless — full Unicode case folding would require
    /// an extra dependency for negligible gain in subtitle workflows.
    #[serde(default)]
    pub case_sensitive: bool,
    /// When true, matches must be flanked by non-letter/non-digit characters
    /// (or string boundaries). Prevents `"is"` from rewriting the inside of
    /// `"this"`. Defaults to true via `default_whole_word`.
    #[serde(default = "default_whole_word")]
    pub whole_word: bool,
}

fn default_whole_word() -> bool {
    true
}

impl Default for ReplaceRule {
    fn default() -> Self {
        Self {
            from: String::new(),
            to: String::new(),
            case_sensitive: false,
            whole_word: true,
        }
    }
}

impl ReplaceRule {
    fn is_active(&self) -> bool {
        !self.from.is_empty()
    }
}

/// Apply every rule to every segment in place. Words inherit replacements
/// too so word-aligned downstream features (resegmenter) stay consistent
/// with segment text.
pub fn apply(segments: &mut [Segment], rules: &[ReplaceRule]) {
    let active: Vec<&ReplaceRule> = rules.iter().filter(|r| r.is_active()).collect();
    if active.is_empty() {
        return;
    }
    for seg in segments {
        for rule in &active {
            seg.text = replace_in(&seg.text, rule);
        }
        for w in &mut seg.words {
            for rule in &active {
                w.text = replace_in(&w.text, rule);
            }
        }
    }
}

/// Apply replacements to a single text run — used by callers that want
/// to correct streamed-segment text live (e.g. UI previews).
pub fn apply_to_text(text: &str, rules: &[ReplaceRule]) -> String {
    let mut out = text.to_string();
    for rule in rules.iter().filter(|r| r.is_active()) {
        out = replace_in(&out, rule);
    }
    out
}

fn replace_in(haystack: &str, rule: &ReplaceRule) -> String {
    if rule.from.is_empty() {
        return haystack.to_string();
    }
    let mut out = String::with_capacity(haystack.len());
    let bytes = haystack.as_bytes();
    let needle_bytes = rule.from.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + needle_bytes.len() <= bytes.len()
            && segment_matches(&haystack[i..], &rule.from, rule.case_sensitive)
            && (!rule.whole_word || is_word_boundary_at(haystack, i, needle_bytes.len()))
        {
            out.push_str(&rule.to);
            i += needle_bytes.len();
        } else {
            // Advance by one full UTF-8 char so we don't slice mid-codepoint.
            let ch_len = utf8_char_len(bytes[i]);
            out.push_str(&haystack[i..i + ch_len]);
            i += ch_len;
        }
    }
    out
}

fn utf8_char_len(b: u8) -> usize {
    if b < 0x80 {
        1
    } else if b < 0xC0 {
        // continuation byte — shouldn't happen at boundary; advance 1 to
        // stay live rather than panic.
        1
    } else if b < 0xE0 {
        2
    } else if b < 0xF0 {
        3
    } else {
        4
    }
}

fn segment_matches(haystack_at: &str, needle: &str, case_sensitive: bool) -> bool {
    let h = haystack_at.as_bytes();
    let n = needle.as_bytes();
    if h.len() < n.len() {
        return false;
    }
    if case_sensitive {
        &h[..n.len()] == n
    } else {
        // ASCII-fold both sides; non-ASCII bytes are compared exactly.
        for (a, b) in h[..n.len()].iter().zip(n.iter()) {
            if !ascii_eq_ci(*a, *b) {
                return false;
            }
        }
        true
    }
}

fn ascii_eq_ci(a: u8, b: u8) -> bool {
    a.eq_ignore_ascii_case(&b)
}

fn is_word_boundary_at(haystack: &str, start: usize, len: usize) -> bool {
    let before_ok = if start == 0 {
        true
    } else {
        // walk back to the previous char start
        let mut j = start;
        while j > 0 {
            j -= 1;
            if (haystack.as_bytes()[j] & 0xC0) != 0x80 {
                break;
            }
        }
        let prev_ch = haystack[j..].chars().next().unwrap_or(' ');
        !prev_ch.is_alphanumeric() && prev_ch != '_'
    };
    let end = start + len;
    let after_ok = if end >= haystack.len() {
        true
    } else {
        let next_ch = haystack[end..].chars().next().unwrap_or(' ');
        !next_ch.is_alphanumeric() && next_ch != '_'
    };
    before_ok && after_ok
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(from: &str, to: &str, cs: bool, ww: bool) -> ReplaceRule {
        ReplaceRule {
            from: from.into(),
            to: to.into(),
            case_sensitive: cs,
            whole_word: ww,
        }
    }

    #[test]
    fn case_insensitive_whole_word_replaces() {
        let r = rule("eryka palm", "Areca palm", false, true);
        assert_eq!(replace_in("The Eryka palm.", &r), "The Areca palm.");
        assert_eq!(replace_in("ERYKA PALM!", &r), "Areca palm!");
    }

    #[test]
    fn whole_word_skips_substrings() {
        let r = rule("is", "IS", false, true);
        assert_eq!(replace_in("This is a test", &r), "This IS a test");
    }

    #[test]
    fn case_sensitive_skips_wrong_case() {
        let r = rule("Aiko", "Subtly", true, true);
        assert_eq!(replace_in("aiko vs Aiko", &r), "aiko vs Subtly");
    }

    #[test]
    fn unicode_passthrough_is_safe() {
        let r = rule("café", "cafe", false, true);
        assert_eq!(replace_in("a café here", &r), "a cafe here");
    }

    #[test]
    fn empty_from_is_noop() {
        let r = rule("", "x", false, true);
        assert_eq!(replace_in("hello", &r), "hello");
    }

    #[test]
    fn applies_to_segments_and_words() {
        use super::super::Word;
        let mut segs = vec![Segment {
            start_ms: 0,
            end_ms: 1000,
            text: "An eryka palm".into(),
            words: vec![Word {
                start_ms: 100,
                end_ms: 500,
                text: "eryka".into(),
            }],
        }];
        apply(&mut segs, &[rule("eryka", "Areca", false, true)]);
        assert_eq!(segs[0].text, "An Areca palm");
        assert_eq!(segs[0].words[0].text, "Areca");
    }
}
