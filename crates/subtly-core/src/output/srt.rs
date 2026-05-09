//! SRT writer + adjacent-duplicate dedup.
//!
//! Dedup logic ported from the legacy `srt.rs::dedup_srt`, which used to
//! re-parse a file written by whisper-cli. Now it runs on the in-memory
//! `Segment` stream before serialization.

use super::Segment;
use anyhow::Result;
use std::fs;
use std::path::Path;

pub fn write(path: &Path, segments: &[Segment], merge_gap_sec: f32) -> Result<()> {
    let merged = dedup(segments, merge_gap_sec);
    let mut out = String::new();
    for (idx, seg) in merged.iter().enumerate() {
        out.push_str(&(idx + 1).to_string());
        out.push('\n');
        out.push_str(&format!(
            "{} --> {}\n",
            ms_to_timestamp(seg.start_ms),
            ms_to_timestamp(seg.end_ms)
        ));
        out.push_str(seg.text.trim());
        out.push_str("\n\n");
    }
    fs::write(path, out.trim_end().to_string() + "\n")?;
    Ok(())
}

pub fn dedup(segments: &[Segment], merge_gap_sec: f32) -> Vec<Segment> {
    let merge_gap_ms = (merge_gap_sec * 1000.0) as i64;
    let mut merged: Vec<Segment> = Vec::with_capacity(segments.len());
    for seg in segments {
        let trimmed = seg.text.trim();
        if trimmed.is_empty() {
            continue;
        }
        let norm = normalize(trimmed);
        if let Some(prev) = merged.last_mut() {
            if normalize(&prev.text) == norm && seg.start_ms <= prev.end_ms + merge_gap_ms {
                prev.end_ms = prev.end_ms.max(seg.end_ms);
                continue;
            }
        }
        merged.push(Segment {
            start_ms: seg.start_ms,
            end_ms: seg.end_ms,
            text: trimmed.to_string(),
            words: seg.words.clone(),
        });
    }
    merged
}

fn normalize(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

pub fn ms_to_timestamp(ms: i64) -> String {
    let mut remaining = ms.max(0);
    let hours = remaining / 3_600_000;
    remaining %= 3_600_000;
    let minutes = remaining / 60_000;
    remaining %= 60_000;
    let seconds = remaining / 1000;
    let millis = remaining % 1000;
    format!("{hours:02}:{minutes:02}:{seconds:02},{millis:03}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(start: i64, end: i64, text: &str) -> Segment {
        Segment::new(start, end, text)
    }

    #[test]
    fn dedup_merges_adjacent_duplicates() {
        let input = vec![
            seg(0, 1000, "Hello world"),
            seg(1100, 2000, "hello world"),
            seg(3000, 4000, "Goodbye"),
        ];
        let out = dedup(&input, 0.6);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].end_ms, 2000);
    }

    #[test]
    fn dedup_keeps_far_apart_duplicates() {
        let input = vec![seg(0, 1000, "Hello"), seg(5000, 6000, "Hello")];
        let out = dedup(&input, 0.6);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn timestamp_format() {
        assert_eq!(ms_to_timestamp(3_723_456), "01:02:03,456");
        assert_eq!(ms_to_timestamp(0), "00:00:00,000");
    }
}
