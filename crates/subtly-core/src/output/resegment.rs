//! Word-greedy cue resegmentation.
//!
//! Whisper's native segments are wildly variable in length — sometimes one
//! cue runs 11 seconds, the next 0.5. That's fine for raw text but bad for
//! subtitles, which want consistent reading time and a comfortable two-line
//! limit. This pass walks word-aligned timing (collected by the engine when
//! token timestamps are on) and packs words into new cues bounded by:
//!
//! * `max_chars` — typical 84 (≈ two 42-char lines)
//! * `max_ms` — upper bound per cue
//! * `min_ms` — minimum visible time; short tails get their end_ms padded
//!   up to this floor without overlapping the next cue
//! * a "natural pause" gap between consecutive words (>= 700 ms) — used
//!   as a soft preferred break point
//!
//! When a segment has no words (token timestamps disabled or model returned
//! none), it passes through untouched. That keeps the feature safe to enable
//! by default.

use super::{Segment, Word};

/// Tunables. All fields are `u32` so they can be wired straight to
/// `Settings` text inputs without conversion gymnastics.
#[derive(Debug, Clone, Copy)]
pub struct ResegmentConfig {
    pub max_chars: u32,
    pub max_ms: u32,
    pub min_ms: u32,
}

impl Default for ResegmentConfig {
    fn default() -> Self {
        Self {
            max_chars: 84,
            max_ms: 6000,
            min_ms: 800,
        }
    }
}

/// Soft break threshold: a word gap larger than this ends a cue when the
/// pack already has any content. Tunable later if needed; not user-facing
/// today to keep the settings surface small.
const NATURAL_PAUSE_MS: i64 = 700;

pub fn run(segments: &[Segment], cfg: ResegmentConfig) -> Vec<Segment> {
    // Collect every word from every segment in order. If the engine produced
    // no words, return the original segments untouched.
    let total_words: usize = segments.iter().map(|s| s.words.len()).sum();
    if total_words == 0 {
        return segments.to_vec();
    }
    let mut all: Vec<Word> = Vec::with_capacity(total_words);
    for seg in segments {
        for w in &seg.words {
            if w.text.trim().is_empty() {
                continue;
            }
            all.push(w.clone());
        }
    }
    if all.is_empty() {
        return segments.to_vec();
    }

    let max_chars = cfg.max_chars.max(20) as usize;
    let max_ms = cfg.max_ms.max(1500) as i64;
    let min_ms = cfg.min_ms as i64;

    let mut out: Vec<Segment> = Vec::new();
    let mut cur: Vec<Word> = Vec::new();
    let mut cur_chars: usize = 0;

    for (idx, w) in all.iter().enumerate() {
        let word_chars = w.text.chars().count();
        let prev_end = cur.last().map(|p| p.end_ms);
        let cur_start = cur.first().map(|p| p.start_ms);

        let projected_chars = if cur.is_empty() {
            word_chars
        } else {
            cur_chars + 1 + word_chars
        };
        let projected_dur = match cur_start {
            Some(s) => w.end_ms - s,
            None => 0,
        };
        let big_gap = matches!(prev_end, Some(e) if w.start_ms - e >= NATURAL_PAUSE_MS);

        let must_break = !cur.is_empty()
            && (projected_chars > max_chars
                || projected_dur > max_ms
                || (big_gap && current_ends_with_terminal(&cur)));

        if must_break {
            out.push(make_cue(&cur));
            cur.clear();
            cur_chars = 0;
        }

        if cur.is_empty() {
            cur_chars = word_chars;
        } else {
            cur_chars += 1 + word_chars;
        }
        cur.push(w.clone());

        // Hard break: we've hit the limit exactly with this word and the
        // text ends in terminal punctuation. Flush so the next cue starts
        // clean rather than dragging a stranded word.
        let last_terminal = current_ends_with_terminal(&cur);
        if last_terminal && (cur_chars >= max_chars * 3 / 4 || idx + 1 == all.len()) {
            out.push(make_cue(&cur));
            cur.clear();
            cur_chars = 0;
        }
    }
    if !cur.is_empty() {
        out.push(make_cue(&cur));
    }

    enforce_min_duration(&mut out, min_ms);
    out
}

fn make_cue(words: &[Word]) -> Segment {
    let start_ms = words.first().map(|w| w.start_ms).unwrap_or(0);
    let end_ms = words.last().map(|w| w.end_ms).unwrap_or(start_ms);
    let text = words
        .iter()
        .map(|w| w.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    Segment {
        start_ms,
        end_ms: end_ms.max(start_ms),
        text,
        words: words.to_vec(),
    }
}

fn current_ends_with_terminal(words: &[Word]) -> bool {
    let Some(last) = words.last() else {
        return false;
    };
    last.text
        .chars()
        .next_back()
        .map(|c| matches!(c, '.' | '?' | '!' | '…' | '。' | '？' | '！'))
        .unwrap_or(false)
}

/// Pad each cue's end so it stays visible at least `min_ms`, without
/// overlapping the next cue's start. Cues longer than `min_ms` are
/// untouched.
fn enforce_min_duration(out: &mut [Segment], min_ms: i64) {
    if min_ms <= 0 {
        return;
    }
    let n = out.len();
    for i in 0..n {
        let cur_end = out[i].end_ms;
        let cur_start = out[i].start_ms;
        let dur = cur_end - cur_start;
        if dur >= min_ms {
            continue;
        }
        let next_start = out.get(i + 1).map(|s| s.start_ms);
        let want = cur_start + min_ms;
        out[i].end_ms = match next_start {
            Some(ns) => want.min(ns),
            None => want,
        };
        if out[i].end_ms < cur_end {
            out[i].end_ms = cur_end;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(start: i64, end: i64, t: &str) -> Word {
        Word {
            start_ms: start,
            end_ms: end,
            text: t.into(),
        }
    }

    fn seg_with(words: Vec<Word>) -> Segment {
        let start = words.first().map(|x| x.start_ms).unwrap_or(0);
        let end = words.last().map(|x| x.end_ms).unwrap_or(0);
        Segment {
            start_ms: start,
            end_ms: end,
            text: words
                .iter()
                .map(|x| x.text.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            words,
        }
    }

    #[test]
    fn passes_through_when_no_words() {
        let segs = vec![Segment::new(0, 1000, "hello world")];
        let out = run(&segs, ResegmentConfig::default());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].text, "hello world");
    }

    #[test]
    fn breaks_on_max_chars() {
        let words = (0..40)
            .map(|i| w(i * 200, i * 200 + 180, "hello"))
            .collect::<Vec<_>>();
        let segs = vec![seg_with(words)];
        let out = run(
            &segs,
            ResegmentConfig {
                max_chars: 20,
                max_ms: 60000,
                min_ms: 0,
            },
        );
        assert!(out.len() > 1);
        for cue in &out {
            assert!(cue.text.chars().count() <= 26, "cue too long: {}", cue.text);
        }
    }

    #[test]
    fn breaks_on_max_duration() {
        let words = (0..10)
            .map(|i| w(i * 1000, i * 1000 + 900, "tick"))
            .collect();
        let segs = vec![seg_with(words)];
        let out = run(
            &segs,
            ResegmentConfig {
                max_chars: 1000,
                max_ms: 3000,
                min_ms: 0,
            },
        );
        assert!(out.len() >= 3);
    }

    #[test]
    fn min_duration_is_enforced_without_overlap() {
        // Force a hard break by exceeding the 1500 ms gap rule (NATURAL_PAUSE_MS)
        // so the two short tail-cues don't get coalesced into one.
        let segs = vec![
            seg_with(vec![w(0, 200, "Hi.")]),
            seg_with(vec![w(2000, 2200, "There.")]),
        ];
        let out = run(
            &segs,
            ResegmentConfig {
                max_chars: 84,
                max_ms: 5000,
                min_ms: 1500,
            },
        );
        assert_eq!(out.len(), 2, "expected one cue per word, got {out:?}");
        // First cue's end must not overlap the second cue's start.
        assert!(out[0].end_ms <= out[1].start_ms);
        // Padded by min_ms but capped at the next cue's start (2000 ms).
        assert_eq!(out[0].end_ms, 1500);
    }
}
