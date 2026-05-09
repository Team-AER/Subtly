//! Audio decode + downmix + resample + loudness pipeline.
//!
//! Replaces the previous `ffmpeg → temp WAV` subprocess hop with an in-process
//! pipeline built on `symphonia` (decode), `rubato` (resample), `ebur128`
//! (loudness measurement). The output is a `Vec<f32>` of mono samples at
//! 16 kHz, ready to feed straight to whisper.

mod decode;
mod loudnorm;
mod mix;
mod resample;

use anyhow::Result;
use std::path::Path;

pub const TARGET_SAMPLE_RATE: u32 = 16_000;

/// Pipeline result: mono f32 samples at 16 kHz, normalized to ~-16 LUFS with
/// peak headroom.
pub fn decode_to_mono_16k<P: AsRef<Path>>(path: P) -> Result<Vec<f32>> {
    let decoded = decode::decode(path.as_ref())?;
    let mono = mix::downmix_to_mono(&decoded);
    let resampled = resample::resample_to(&mono, decoded.sample_rate, TARGET_SAMPLE_RATE)?;
    Ok(loudnorm::normalize(resampled, TARGET_SAMPLE_RATE))
}
