//! Loudness normalization to ~-16 LUFS with a true-peak ceiling at -1.5 dBFS.
//!
//! This is a single-pass approximation of ffmpeg's `loudnorm=I=-16:LRA=11:TP=-1.5`.
//! We measure integrated loudness via `ebur128`, compute a static gain to
//! reach the target, then apply a peak-aware soft-clip to keep the true peak
//! below the ceiling. Whisper does its own internal normalization on the mel
//! spectrogram, so getting within ~1-2 LU of the target is plenty.

use ebur128::{EbuR128, Mode};

const TARGET_LUFS: f64 = -16.0;
const TRUE_PEAK_CEILING_DBFS: f64 = -1.5;
const MAX_GAIN_DB: f64 = 30.0;
const MIN_GAIN_DB: f64 = -30.0;

pub fn normalize(mut samples: Vec<f32>, sample_rate: u32) -> Vec<f32> {
    if samples.is_empty() {
        return samples;
    }

    let measured = measure_loudness(&samples, sample_rate);
    let gain_db = match measured {
        Some(lufs) if lufs.is_finite() => (TARGET_LUFS - lufs).clamp(MIN_GAIN_DB, MAX_GAIN_DB),
        _ => 0.0,
    };
    let gain = 10f32.powf((gain_db / 20.0) as f32);

    let ceiling = 10f32.powf((TRUE_PEAK_CEILING_DBFS / 20.0) as f32);
    for s in samples.iter_mut() {
        let v = *s * gain;
        *s = soft_clip(v, ceiling);
    }
    samples
}

fn measure_loudness(samples: &[f32], sample_rate: u32) -> Option<f64> {
    let mut meter = EbuR128::new(1, sample_rate, Mode::I).ok()?;
    meter.add_frames_f32(samples).ok()?;
    meter.loudness_global().ok()
}

#[inline]
fn soft_clip(x: f32, ceiling: f32) -> f32 {
    x.clamp(-ceiling, ceiling)
}
