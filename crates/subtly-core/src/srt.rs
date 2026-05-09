//! SRT post-processing: dedup adjacent identical subtitles.
//! Lifted from `runtime/gpu-runtime/src/main.rs` (`dedup_srt`).

use anyhow::{anyhow, Result};
use std::fs;
use std::path::Path;

#[derive(Debug)]
struct SubtitleItem {
    start_ms: i64,
    end_ms: i64,
    text: String,
    norm: String,
}

pub fn dedup_srt(path: &Path, merge_gap_sec: f32) -> Result<()> {
    let content = match fs::read_to_string(path) {
        Ok(data) => data,
        Err(_) => return Ok(()),
    };
    let normalized = content.replace("\r\n", "\n");
    if normalized.trim().is_empty() {
        return Ok(());
    }

    let mut items = Vec::new();
    for block in normalized.split("\n\n") {
        let mut lines = block.lines();
        let _index = lines.next();
        let times = match lines.next() {
            Some(line) if line.contains("-->") => line,
            _ => continue,
        };
        let mut time_parts = times.split("-->").map(|s| s.trim());
        let start = time_parts.next().unwrap_or_default();
        let end = time_parts.next().unwrap_or_default();
        let start_ms = timestamp_to_ms(start)?;
        let end_ms = timestamp_to_ms(end)?;
        let text = lines.collect::<Vec<_>>().join("\n").trim().to_string();
        if text.is_empty() {
            continue;
        }
        let norm = text
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        items.push(SubtitleItem {
            start_ms,
            end_ms,
            text,
            norm,
        });
    }

    let merge_gap_ms = (merge_gap_sec * 1000.0) as i64;
    let mut merged: Vec<SubtitleItem> = Vec::new();
    for item in items {
        if let Some(prev) = merged.last_mut() {
            if prev.norm == item.norm && item.start_ms <= prev.end_ms + merge_gap_ms {
                prev.end_ms = prev.end_ms.max(item.end_ms);
                continue;
            }
        }
        merged.push(item);
    }

    let mut out = String::new();
    for (idx, item) in merged.iter().enumerate() {
        out.push_str(&(idx + 1).to_string());
        out.push('\n');
        out.push_str(&format!(
            "{} --> {}\n",
            ms_to_timestamp(item.start_ms),
            ms_to_timestamp(item.end_ms)
        ));
        out.push_str(&item.text);
        out.push_str("\n\n");
    }
    fs::write(path, out.trim_end().to_string() + "\n")?;
    Ok(())
}

fn timestamp_to_ms(ts: &str) -> Result<i64> {
    let mut parts = ts.split(':');
    let hours = parts
        .next()
        .ok_or_else(|| anyhow!("Invalid timestamp"))?
        .parse::<i64>()?;
    let minutes = parts
        .next()
        .ok_or_else(|| anyhow!("Invalid timestamp"))?
        .parse::<i64>()?;
    let seconds_ms = parts
        .next()
        .ok_or_else(|| anyhow!("Invalid timestamp"))?;
    let mut seconds_parts = seconds_ms.split(',');
    let seconds = seconds_parts
        .next()
        .ok_or_else(|| anyhow!("Invalid timestamp"))?
        .parse::<i64>()?;
    let millis = seconds_parts
        .next()
        .ok_or_else(|| anyhow!("Invalid timestamp"))?
        .parse::<i64>()?;
    Ok((hours * 3600 + minutes * 60 + seconds) * 1000 + millis)
}

fn ms_to_timestamp(ms: i64) -> String {
    let mut remaining = ms;
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

    #[test]
    fn timestamp_round_trip() {
        let ms = timestamp_to_ms("01:02:03,456").unwrap();
        assert_eq!(ms, 3_723_456);
        assert_eq!(ms_to_timestamp(ms), "01:02:03,456");
    }
}
