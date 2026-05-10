# Subtly

Desktop app for GPU-accelerated Whisper subtitle generation. Single Rust binary built with [Iced](https://iced.rs); whisper.cpp is linked in directly via [whisper-rs](https://github.com/tazz4843/whisper-rs) and audio decoding/resampling/loudness all run in-process via [symphonia](https://github.com/pdeljanov/Symphonia) + [rubato](https://github.com/HEnquist/rubato) + [ebur128](https://github.com/sdroege/ebur128). No subprocesses at runtime.

## Layout

```
crates/
  subtly-core/    transcription orchestration, model catalog, downloads, settings
  subtly-ui/      Iced application (workspace / models / advanced screens)
  xtask/          build helper (asset download, sync, packaging, notarization)
resources/        icons, entitlements, NSIS installer script
scripts/          assets-manifest.json, code-signing helpers
```

## Supported platforms

| OS | Versions | Notes |
|---|---|---|
| Windows | 10 (build 1809 / "October 2018 Update", x64) and 11 | Vulkan-capable GPU driver recommended; falls back to CPU if Vulkan is missing. Windows 7 / 8.1 are **not** supported (the binary depends on the in-box Universal CRT, which Win7/8 ship without). ARM64 Windows is not currently supported. |
| macOS | 10.15+ (x64 / arm64) | Metal acceleration on Apple Silicon. |
| Linux | glibc 2.31+ (Ubuntu 20.04+ or equivalent) | Needs a Vulkan loader (`libvulkan1`). |

The Windows installer ships the VC++ runtime DLLs (`vcruntime140`, `vcruntime140_1`, `msvcp140`) app-locally so machines without the Visual C++ Redistributable still launch.

## Build prerequisites

- Rust 1.75+
- A C/C++ toolchain (Xcode CLT on macOS, MSVC on Windows, gcc/clang on Linux) — required by `whisper-rs` to compile whisper.cpp
- `cmake` ≥ 3.10
- macOS: Metal SDK (bundled with Xcode)
- Linux/Windows: a Vulkan loader + headers (`libvulkan-dev` on Debian/Ubuntu)

## Dev flow

```sh
# Pull the VAD model (only bundled asset)
cargo run -p xtask -- download-assets

# Run the GUI in dev mode
cargo run -p subtly-ui

# Parity-test CLI
cargo run -p subtly-core --bin subtly-cli -- ping
cargo run -p subtly-core --bin subtly-cli -- transcribe /path/to/file.mp4 --dry-run

# Unit tests
cargo test --workspace
```

## Release build

```sh
cargo build --release -p subtly-ui
./target/release/subtly
```

## Packaging

```sh
# One-time: install cargo-packager
cargo install cargo-packager --locked

# 1. Pull the bundled VAD model for the host platform
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
| `models/silero_vad.bin` | Required VAD model |

Whisper models are not bundled — users download them through the Models tab on first run. The default/recommended Whisper model is `large-v2`, matching Aiko's documented macOS accuracy-first model choice. The Silero VAD model comes from `resources/runtime-assets/`, which `xtask download-assets` populates from `scripts/assets-manifest.json` (verified by SHA256).

## Auto-update / signed releases

Auto-update is wired through [cargo-dist](https://github.com/axodotdev/cargo-dist) + [axoupdater](https://github.com/axodotdev/axoupdater) (gated behind the `auto-update` feature on `subtly-ui`). Set up the release pipeline once with `cargo dist init`; tag pushes then produce signed artifacts and a `dist-manifest.json` the running app reads on startup.

## Optional features

| Feature | Effect |
|---|---|
| `crash-reporting` | Initialize Sentry from `SENTRY_DSN` |
| `auto-update`     | Run axoupdater on startup |

## Settings

Persisted to `${config_dir}/app.aer.Subtly/settings.json`. Models live in `${data_dir}/app.aer.Subtly/models/`.

Default transcription behavior favors same-language transcription over translation. Enable “Translate to English” only when you explicitly want Whisper's English translation mode.

## Architecture note

Earlier versions shelled out to bundled `whisper-cli` and `ffmpeg` binaries to keep heavy compute crash-isolated from the UI. The current build links whisper.cpp in via `whisper-rs` and runs all decoding through `symphonia`/`rubato` in-process — inference happens on a `tokio::task::spawn_blocking` thread so the UI stays responsive, and the abort callback wired to `tokio::sync::watch` lets users cancel mid-file. Device enumeration and the smoke test are wrapped in `catch_unwind` to harden against flaky drivers.
