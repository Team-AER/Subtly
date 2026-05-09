//! Downmix interleaved multi-channel samples to mono.
//!
//! Mirrors the previous ffmpeg `pan` filter:
//!   `c0 = 0.35*FL + 0.35*FR + 0.80*FC + 0.15*SL + 0.15*SR`
//! plus sane fallbacks for mono (passthrough), stereo (equal-power), and
//! anything else (per-channel weight by position; LFE/height channels are
//! dropped because they don't help speech recognition).

use super::decode::Decoded;
use symphonia::core::audio::Channels;

pub fn downmix_to_mono(decoded: &Decoded) -> Vec<f32> {
    let n_ch = decoded.channels.count();
    let frames = decoded.samples.len() / n_ch.max(1);

    if n_ch <= 1 {
        return decoded.samples.clone();
    }
    if n_ch == 2 && decoded.channels == (Channels::FRONT_LEFT | Channels::FRONT_RIGHT) {
        let mut out = Vec::with_capacity(frames);
        for f in 0..frames {
            let l = decoded.samples[f * 2];
            let r = decoded.samples[f * 2 + 1];
            out.push(0.5 * (l + r));
        }
        return out;
    }

    let mut weights: Vec<f32> = decoded.channels.iter().map(channel_weight).collect();
    let sum: f32 = weights.iter().sum();
    if sum <= 0.0 {
        let inv = 1.0 / n_ch as f32;
        for w in weights.iter_mut() {
            *w = inv;
        }
    }

    let mut out = Vec::with_capacity(frames);
    for f in 0..frames {
        let base = f * n_ch;
        let mut acc = 0.0f32;
        for (c, w) in weights.iter().enumerate() {
            acc += decoded.samples[base + c] * *w;
        }
        out.push(acc);
    }
    out
}

fn channel_weight(c: Channels) -> f32 {
    match c {
        Channels::FRONT_LEFT | Channels::FRONT_RIGHT => 0.35,
        Channels::FRONT_CENTRE => 0.80,
        Channels::SIDE_LEFT | Channels::SIDE_RIGHT => 0.15,
        Channels::REAR_LEFT | Channels::REAR_RIGHT => 0.15,
        Channels::FRONT_LEFT_CENTRE | Channels::FRONT_RIGHT_CENTRE => 0.20,
        Channels::REAR_CENTRE => 0.10,
        _ => 0.0,
    }
}
