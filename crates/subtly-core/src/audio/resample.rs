//! Resample a mono f32 stream to 16 kHz using rubato's FFT resampler.
//!
//! The FFT resampler produces good quality at low CPU cost. It accepts a
//! fixed input chunk length and yields a variable output length per call.

use anyhow::{anyhow, Result};
use rubato::{FftFixedIn, Resampler};

const CHUNK_FRAMES: usize = 1024;
const SUB_CHUNKS: usize = 2;

pub fn resample_to(samples: &[f32], from_rate: u32, to_rate: u32) -> Result<Vec<f32>> {
    if from_rate == to_rate {
        return Ok(samples.to_vec());
    }
    if samples.is_empty() {
        return Ok(Vec::new());
    }

    let mut resampler = FftFixedIn::<f32>::new(
        from_rate as usize,
        to_rate as usize,
        CHUNK_FRAMES,
        SUB_CHUNKS,
        1,
    )
    .map_err(|e| anyhow!("rubato init: {e}"))?;

    let chunk_len = resampler.input_frames_next();
    let mut out: Vec<f32> = Vec::with_capacity(
        (samples.len() as u64 * to_rate as u64 / from_rate as u64) as usize + chunk_len,
    );
    let mut input_buf = vec![vec![0.0f32; chunk_len]];
    let mut output_buf = resampler.output_buffer_allocate(true);

    let mut consumed = 0usize;
    while consumed + chunk_len <= samples.len() {
        input_buf[0].copy_from_slice(&samples[consumed..consumed + chunk_len]);
        let (_, written) = resampler
            .process_into_buffer(&input_buf, &mut output_buf, None)
            .map_err(|e| anyhow!("rubato process: {e}"))?;
        out.extend_from_slice(&output_buf[0][..written]);
        consumed += chunk_len;
    }

    if consumed < samples.len() {
        let tail: Vec<f32> = samples[consumed..].to_vec();
        let tail_in = vec![tail];
        let (_, written) = resampler
            .process_partial_into_buffer(Some(&tail_in), &mut output_buf, None)
            .map_err(|e| anyhow!("rubato flush: {e}"))?;
        out.extend_from_slice(&output_buf[0][..written]);
    }

    Ok(out)
}
