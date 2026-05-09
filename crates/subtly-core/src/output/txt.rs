//! Plain-text transcript: one segment per line, no timestamps.

use super::Segment;
use anyhow::Result;
use std::fs;
use std::path::Path;

pub fn write(path: &Path, segments: &[Segment]) -> Result<()> {
    let mut out = String::new();
    for seg in segments {
        let trimmed = seg.text.trim();
        if trimmed.is_empty() {
            continue;
        }
        out.push_str(trimmed);
        out.push('\n');
    }
    fs::write(path, out)?;
    Ok(())
}
