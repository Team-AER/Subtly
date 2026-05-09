Packaged runtime assets.

Only the Silero VAD model is bundled now — whisper.cpp is linked into the app
binary via `whisper-rs`, and audio decoding goes through `symphonia` in-process,
so there are no `whisper-cli` or `ffmpeg` binaries to ship.

Layout:

- `models/silero_vad.bin` — populated by `cargo run -p xtask -- download-assets`
  from `scripts/assets-manifest.json` (SHA256-verified).

User-downloaded Whisper models live in `${data_dir}/app.aer.Subtly/models/`,
not here.
