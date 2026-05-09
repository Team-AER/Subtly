Local development assets staging directory.

Subtly no longer ships any standalone binaries — whisper.cpp links in via
`whisper-rs` and audio decoding goes through `symphonia` in-process. The only
bundled asset is the Silero VAD model, which `cargo run -p xtask -- sync-assets`
mirrors here from `resources/runtime-assets/`.

Whisper transcription models are downloaded by the user at runtime through the
Models tab and live in `${data_dir}/app.aer.Subtly/models/`.
