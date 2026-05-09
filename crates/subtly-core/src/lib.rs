//! Subtly core: device enumeration, model catalog/download, transcribe orchestration.
//!
//! All inference runs in-process via `whisper-rs` (FFI to whisper.cpp) and
//! audio decoding/resampling/loudness all run through `symphonia` + `rubato`
//! + `ebur128`. There is no longer any subprocess (whisper-cli, ffmpeg)
//! involved at runtime.

pub mod audio;
pub mod devices;
pub mod download;
pub mod events;
pub mod models;
pub mod output;
pub mod paths;
pub mod settings;
pub mod transcribe;
pub mod whisper;

pub use devices::{list_devices, ping, select_best_device, DeviceInfo, PingResult};
pub use download::{download_model, DownloadProgress};
pub use events::{Event, ProgressUpdate, SegmentEvent};
pub use models::{installed_models, InstalledModel, ModelDescriptor, VAD_MODEL, WHISPER_MODELS};
pub use output::Segment;
pub use paths::{models_directory, resolve_asset_dir};
pub use settings::{Settings, ValidationError};
pub use transcribe::{transcribe, TranscribeConfig, TranscribeOutcome, TranscribeParams};
