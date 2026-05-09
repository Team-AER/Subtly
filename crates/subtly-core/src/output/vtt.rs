//! WebVTT writer.

use super::Segment;
use anyhow::Result;
use std::fs;
use std::path::Path;

pub fn write(path: &Path, segments: &[Segment]) -> Result<()> {
    let mut out = String::from("WEBVTT\n\n");
    for seg in segments {
        let trimmed = seg.text.trim();
        if trimmed.is_empty() {
            continue;
        }
        out.push_str(&format!(
            "{} --> {}\n",
            ms_to_timestamp(seg.start_ms),
            ms_to_timestamp(seg.end_ms)
        ));
        out.push_str(trimmed);
        out.push_str("\n\n");
    }
    fs::write(path, out.trim_end().to_string() + "\n")?;
    Ok(())
}

fn ms_to_timestamp(ms: i64) -> String {
    let mut remaining = ms.max(0);
    let hours = remaining / 3_600_000;
    remaining %= 3_600_000;
    let minutes = remaining / 60_000;
    remaining %= 60_000;
    let seconds = remaining / 1000;
    let millis = remaining % 1000;
    format!("{hours:02}:{minutes:02}:{seconds:02}.{millis:03}")
}
