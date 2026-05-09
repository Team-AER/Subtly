//! In-process Whisper inference via `whisper-rs` (FFI to whisper.cpp).
//!
//! Replaces the previous subprocess invocation of `whisper-cli`. The engine
//! exposes a single async-compatible entrypoint that runs the actual
//! inference on a blocking thread (`spawn_blocking`) so the UI's tokio
//! runtime stays responsive.

mod engine;
mod params;

pub use engine::transcribe_samples;
pub use params::WhisperConfig;
