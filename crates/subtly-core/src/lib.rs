//! Subtly core: device enumeration, model catalog/download, transcribe orchestration.
//!
//! Lifted from `runtime/gpu-runtime/src/main.rs` and de-coupled from the JSON-RPC
//! stdio loop. Functions return strongly-typed values; progress is emitted through
//! a `tokio::sync::mpsc` channel of [`Event`]s instead of newline-delimited JSON.

pub mod devices;
pub mod download;
pub mod events;
pub mod models;
pub mod paths;
pub mod settings;
pub mod srt;
pub mod transcribe;

pub use devices::{list_devices, ping, select_best_device, DeviceInfo, PingResult};
pub use download::{download_model, DownloadProgress};
pub use events::{Event, ProgressUpdate};
pub use models::{installed_models, InstalledModel, ModelDescriptor, VAD_MODEL, WHISPER_MODELS};
pub use paths::{models_directory, resolve_asset_dir};
pub use settings::{Settings, ValidationError};
pub use transcribe::{transcribe, TranscribeConfig, TranscribeOutcome, TranscribeParams};
