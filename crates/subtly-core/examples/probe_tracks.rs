use std::fs::File;
use symphonia::core::codecs::CODEC_TYPE_NULL;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::default::{get_codecs, get_probe};

fn main() -> anyhow::Result<()> {
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
    println!(
        "default track id: {:?}",
        format.default_track().map(|t| t.id)
    );
    for t in format.tracks() {
        let cp = &t.codec_params;
        let supported = get_codecs().get_codec(cp.codec).is_some();
        println!(
            "track id={} codec={:?} (null? {}) sample_rate={:?} channels={:?} time_base={:?} supported_audio_codec={}",
            t.id, cp.codec, cp.codec == CODEC_TYPE_NULL, cp.sample_rate, cp.channels, cp.time_base, supported
        );
    }
    Ok(())
}
