//! Decode an audio/video container into interleaved f32 samples using
//! symphonia. Picks the default audio track; ignores video. Returns the
//! native sample rate and channel layout — downmixing and resampling happen
//! in later stages.

use anyhow::{anyhow, Context, Result};
use std::fs::File;
use std::path::Path;
use symphonia::core::audio::{Channels, SampleBuffer};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::default::{get_codecs, get_probe};

pub struct Decoded {
    /// Interleaved samples in `channels` order.
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: Channels,
}

pub fn decode(path: &Path) -> Result<Decoded> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        hint.with_extension(ext);
    }

    let probed = get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .with_context(|| format!("probing {}", path.display()))?;

    let mut format = probed.format;

    // For containers like mp4/mkv, `default_track()` often returns the video
    // track which has no `sample_rate`. Only accept the default if it looks
    // like audio; otherwise scan for the first track with audio params.
    let is_audio = |t: &symphonia::core::formats::Track| {
        t.codec_params.sample_rate.is_some() && t.codec_params.channels.is_some()
    };
    let track = format
        .default_track()
        .filter(|t| is_audio(t))
        .or_else(|| format.tracks().iter().find(|t| is_audio(t)))
        .ok_or_else(|| anyhow!("no audio track in {}", path.display()))?;
    let track_id = track.id;
    let codec_params = track.codec_params.clone();

    let sample_rate = codec_params
        .sample_rate
        .ok_or_else(|| anyhow!("track {} missing sample rate", track_id))?;
    let channels = codec_params
        .channels
        .ok_or_else(|| anyhow!("track {} missing channel layout", track_id))?;

    let mut decoder = get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .context("no decoder for codec")?;

    let mut interleaved: Vec<f32> = Vec::new();
    let mut sample_buf: Option<SampleBuffer<f32>> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(SymphoniaError::ResetRequired) => {
                return Err(anyhow!("decoder reset requested mid-stream"));
            }
            Err(e) => return Err(e).context("reading packet"),
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(audio_buf) => {
                let spec = *audio_buf.spec();
                let duration = audio_buf.capacity() as u64;
                let buf =
                    sample_buf.get_or_insert_with(|| SampleBuffer::<f32>::new(duration, spec));
                buf.copy_interleaved_ref(audio_buf);
                interleaved.extend_from_slice(buf.samples());
            }
            Err(SymphoniaError::IoError(_)) => break,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(e).context("decoding packet"),
        }
    }

    if interleaved.is_empty() {
        return Err(anyhow!("decoded zero samples from {}", path.display()));
    }

    Ok(Decoded {
        samples: interleaved,
        sample_rate,
        channels,
    })
}
