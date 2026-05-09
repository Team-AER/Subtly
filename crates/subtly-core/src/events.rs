use serde::{Deserialize, Serialize};

/// Events emitted by long-running orchestration tasks (transcribe, download).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    Log(String),
    Progress(ProgressUpdate),
    /// A finalized transcript segment, emitted as inference produces it.
    /// Lets the UI stream subtitles into a live preview pane while a file
    /// is still being processed.
    Segment(SegmentEvent),
    /// The language whisper picked for this file. Emitted once per file
    /// after inference returns. When `Settings::language` is "auto" this
    /// reflects the actual detection; for an explicit language it just
    /// echoes that code.
    DetectedLanguage(DetectedLanguage),
    OutputWritten(String),
    Done,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProgressUpdate {
    pub progress: Option<u8>,
    pub current: Option<u64>,
    pub total: Option<u64>,
    pub phase: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentEvent {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedLanguage {
    /// ISO 639-1 short code, e.g. `"en"`, `"hi"`. Whisper's full set.
    pub code: String,
    /// Human-readable name, e.g. `"english"`. Already lower-cased upstream.
    pub name: String,
}
