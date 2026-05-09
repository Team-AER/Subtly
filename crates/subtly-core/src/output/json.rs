//! JSON writer.
//!
//! Schema mirrors the shape produced by `whisper-cli -oj`: a top-level object
//! with `transcription` array of `{timestamps: {from, to}, offsets: {from, to}, text}`.

use super::Segment;
use anyhow::Result;
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Serialize)]
struct Doc<'a> {
    transcription: Vec<Item<'a>>,
}

#[derive(Serialize)]
struct Item<'a> {
    timestamps: Stamps,
    offsets: Offsets,
    text: &'a str,
}

#[derive(Serialize)]
struct Stamps {
    from: String,
    to: String,
}

#[derive(Serialize)]
struct Offsets {
    from: i64,
    to: i64,
}

pub fn write(path: &Path, segments: &[Segment]) -> Result<()> {
    let doc = Doc {
        transcription: segments
            .iter()
            .map(|s| Item {
                timestamps: Stamps {
                    from: ms_to_timestamp(s.start_ms),
                    to: ms_to_timestamp(s.end_ms),
                },
                offsets: Offsets {
                    from: s.start_ms,
                    to: s.end_ms,
                },
                text: s.text.trim(),
            })
            .collect(),
    };
    let body = serde_json::to_string_pretty(&doc)?;
    fs::write(path, body)?;
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
    format!("{hours:02}:{minutes:02}:{seconds:02},{millis:03}")
}
