# Development runtime assets

Subtly links whisper.cpp via `whisper-rs` and decodes audio through Symphonia in-process. No standalone whisper-cli or FFmpeg binaries are staged here.

From the repository root:

```sh
cargo run -p xtask -- download-assets
cargo run -p xtask -- sync-assets
```

The first command downloads and SHA256-checks Silero VAD into `resources/runtime-assets/models/silero_vad.bin`; the second mirrors it into `runtime/assets/models/silero_vad.bin`. Both are needed for the normal development resolver, which checks this directory first. Alternatively set `AER_ASSET_DIR` to a populated asset root.

Whisper models are downloaded separately in the Models screen and stored in the OS-specific application data directory. See [settings and models](../../README.md#settings-and-models) for paths.
