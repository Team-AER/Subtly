use anyhow::{anyhow, Result};
use std::fs::File;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::default::get_probe;

fn main() -> Result<()> {
    let path = std::env::args().nth(1).unwrap();
    let file = File::open(&path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp4");
    let probed = get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;
    let format = probed.format;

    let is_audio = |t: &symphonia::core::formats::Track| {
        t.codec_params.sample_rate.is_some() && t.codec_params.channels.is_some()
    };

    let dt = format.default_track();
    println!("default_track: id={:?}", dt.map(|t| t.id));
    if let Some(t) = dt {
        println!(
            "  default sample_rate={:?} channels={:?} is_audio={}",
            t.codec_params.sample_rate,
            t.codec_params.channels,
            is_audio(t)
        );
    }

    let track = format
        .default_track()
        .filter(|t| is_audio(t))
        .or_else(|| format.tracks().iter().find(|t| is_audio(t)))
        .ok_or_else(|| anyhow!("no audio track in {}", path))?;
    let track_id = track.id;
    let codec_params = track.codec_params.clone();
    println!(
        "selected track id={} sample_rate={:?} channels={:?}",
        track_id, codec_params.sample_rate, codec_params.channels
    );

    let _ = codec_params
        .sample_rate
        .ok_or_else(|| anyhow!("track {} missing sample rate", track_id))?;
    Ok(())
}
