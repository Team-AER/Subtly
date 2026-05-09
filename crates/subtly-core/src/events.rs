use serde::{Deserialize, Serialize};

/// Events emitted by long-running orchestration tasks (transcribe, download).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    Log(String),
    Progress(ProgressUpdate),
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
