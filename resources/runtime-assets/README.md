# Packaged runtime assets

The bundled model is `models/silero_vad.bin`. From the repository root, `cargo run -p xtask -- download-assets` downloads it using [assets-manifest.json](../../scripts/assets-manifest.json) and verifies its SHA256. Packaging copies it into the platform resource directory.

For a development run, also execute `cargo run -p xtask -- sync-assets` to populate `runtime/assets`, or point `AER_ASSET_DIR` to this directory. See [the development asset guide](../../runtime/assets/README.md).

whisper.cpp is linked through `whisper-rs`, and decoding uses Symphonia in-process; no whisper-cli or FFmpeg binaries are bundled. Whisper transcription models are separate user downloads stored in the OS-specific data directory, as described in [README.md](../../README.md#settings-and-models).
