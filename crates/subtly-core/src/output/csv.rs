//! CSV writer. Columns mirror whisper-cli's `-ocsv`: `start,end,text`.

use super::Segment;
use anyhow::Result;
use std::fs;
use std::path::Path;

pub fn write(path: &Path, segments: &[Segment]) -> Result<()> {
    let mut out = String::from("start,end,text\n");
    for seg in segments {
        let trimmed = seg.text.trim();
        if trimmed.is_empty() {
            continue;
        }
        out.push_str(&format!(
            "{},{},\"{}\"\n",
            seg.start_ms,
            seg.end_ms,
            escape(trimmed)
        ));
    }
    fs::write(path, out)?;
    Ok(())
}

fn escape(text: &str) -> String {
    text.replace('"', "\"\"")
}
