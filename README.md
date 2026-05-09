# Subtly

Desktop app for GPU-accelerated Whisper subtitle generation. Single Rust binary built with [Iced](https://iced.rs); shells out to bundled `whisper-cli` and `ffmpeg` for the actual compute. ~13 MB release binary.

## Layout

```
crates/
  subtly-core/    transcription orchestration, model catalog, downloads, settings
  subtly-ui/      Iced application (workspace / models / advanced screens)
  xtask/          build helper (asset download, sync, packaging, notarization)
resources/        icons, entitlements, NSIS installer script
runtime/assets/   bundled whisper-cli, ffmpeg, model files (populated by xtask download-assets)
scripts/          assets-manifest.json, build-whisper.sh, code-signing helpers
```

## Dev flow

```sh
# Pull bundled binaries + VAD model for the current platform
cargo run -p xtask -- download-assets

# Run the GUI in dev mode
cargo run -p subtly-ui

# Run the parity-test CLI
cargo run -p subtly-core --bin subtly-cli -- ping
cargo run -p subtly-core --bin subtly-cli -- transcribe /path/to/file.mp4 --dry-run

# Run unit tests
cargo test --workspace
```

## Release build

```sh
# Single-binary release
cargo build --release -p subtly-ui
./target/release/subtly
```

## Packaging

```sh
# One-time: install cargo-packager
cargo install cargo-packager --locked

# 1. Pull bundleable assets for the host platform (ffmpeg, VAD model, etc.)
cargo run -p xtask -- download-assets

# 2. Build the release binary
cargo build --release -p subtly-ui

# 3. Package — outputs in ./release/
cargo packager --release                     # default formats for host OS
cargo packager --release -f dmg              # macOS .dmg
cargo packager --release -f app              # macOS .app bundle only
cargo packager --release -f nsis             # Windows .exe installer (run on Windows)
cargo packager --release -f deb              # Linux .deb (run on Linux)
cargo packager --release -f appimage         # Linux .AppImage (run on Linux)
```

cargo-packager can only target the host OS — cross-platform builds need a CI matrix (one runner per OS). The `release/` directory is shared by all formats; clean it between runs if you switch.

### macOS code signing + notarization

```sh
# Sign during packaging (cargo-packager picks up APPLE_SIGNING_IDENTITY)
APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAM)" \
  cargo packager --release -f dmg

# Notarize + staple the resulting .app (or .dmg)
APPLE_ID=you@example.com \
APPLE_APP_SPECIFIC_PASSWORD=xxxx-xxxx-xxxx-xxxx \
APPLE_TEAM_ID=ABCDE12345 \
  cargo run -p xtask -- notarize release/Subtly.app
```

### What gets bundled

cargo-packager copies the following into the platform's resource directory:

| Path | Purpose |
|---|---|
| `bin/ffmpeg`     | Audio decode/normalize before whisper-cli |
| `bin/whisper-cli` | Whisper.cpp inference binary (build with `scripts/build-whisper.sh`) |
| `models/silero_vad.bin` | Required VAD model |

Whisper models are not bundled — users download them through the Models tab on first run. The bundled `whisper-cli` and `ffmpeg` come from `resources/runtime-assets/`, which `xtask download-assets` populates from `scripts/assets-manifest.json` (verified by SHA256).

## Auto-update / signed releases

Auto-update is wired through [cargo-dist](https://github.com/axodotdev/cargo-dist) + [axoupdater](https://github.com/axodotdev/axoupdater) (gated behind the `auto-update` feature on `subtly-ui`). Set up the release pipeline once with `cargo dist init`; tag pushes then produce signed artifacts and a `dist-manifest.json` the running app reads on startup.

macOS notarization (run inside CI after signing):

```sh
APPLE_ID=... APPLE_APP_SPECIFIC_PASSWORD=... APPLE_TEAM_ID=... \
  cargo run -p xtask -- notarize path/to/Subtly.app
```

## Optional features

| Feature | Effect |
|---|---|
| `crash-reporting` | Initialize Sentry from `SENTRY_DSN` |
| `auto-update`     | Run axoupdater on startup |

## Settings

Persisted to `${config_dir}/app.aer.Subtly/settings.json`. Models live in
`${data_dir}/app.aer.Subtly/models/`.

## Architecture note

Earlier versions ran the GPU code as a separate `gpu-runtime` child process so a
Vulkan ICD panic couldn't crash the React UI. The Iced rewrite merges device
enumeration into the app process — but `whisper-cli` and `ffmpeg` are still
spawned as subprocesses, so the heavy compute remains crash-isolated. Enumeration
and the smoke test are wrapped in `catch_unwind` to harden against flaky drivers.
