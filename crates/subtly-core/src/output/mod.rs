//! In-memory transcript representation + per-format file serializers.
//!
//! `Segment` is the canonical exchange type between the whisper engine and
//! every output writer (SRT/VTT/JSON/CSV/TXT). The engine fills a
//! `Vec<Segment>`, the SRT writer optionally runs it through dedup, and each
//! format writer renders its own file.

mod csv;
mod json;
mod srt;
mod txt;
mod vtt;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

/// Write all requested formats. Each format is keyed by extension
/// (`"srt"`, `"vtt"`, `"json"`, `"csv"`, `"txt"`); unknown values are
/// treated as SRT for parity with the previous whisper-cli default.
///
/// `merge_gap_sec` is only used by the SRT writer to merge adjacent
/// duplicates (mirrors the previous `dedup_srt` post-processing step).
pub fn write_all(
    segments: &[Segment],
    output_base: &Path,
    formats: &[String],
    merge_gap_sec: f32,
) -> Result<Vec<std::path::PathBuf>> {
    let mut written = Vec::with_capacity(formats.len());
    for fmt in formats {
        let ext = format_to_ext(fmt);
        let path = output_base.with_extension(ext);
        match ext {
            "srt" => srt::write(&path, segments, merge_gap_sec)?,
            "vtt" => vtt::write(&path, segments)?,
            "json" => json::write(&path, segments)?,
            "csv" => csv::write(&path, segments)?,
            "txt" => txt::write(&path, segments)?,
            _ => srt::write(&path, segments, merge_gap_sec)?,
        }
        written.push(path);
    }
    Ok(written)
}

pub fn format_to_ext(format: &str) -> &'static str {
    match format {
        "vtt" => "vtt",
        "json" => "json",
        "csv" => "csv",
        "txt" => "txt",
        _ => "srt",
    }
}
