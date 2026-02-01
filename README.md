# Subtly (Electron + GPU Sidecar)

![Build Status](https://github.com/Team-AER/Subtly/actions/workflows/build.yml/badge.svg)

This repo is a working scaffold for an Electron desktop app that generates subtitles for audio/video using Whisper. The UI is Electron + React + Vite; the runtime is Rust + wgpu (Vulkan on Windows/Linux, Metal on macOS). It is intentionally split so GPU faults don't crash the UI.

## Layout

- `src`: Electron app (main, preload, renderer via Vite)
- `runtime/gpu-runtime`: Rust JSON-RPC sidecar (device enumeration + Whisper pipeline)

## Dev flow

1) Build the runtime

```
cargo build --manifest-path runtime/gpu-runtime/Cargo.toml
```

2) Install dependencies

```
pnpm install
```

3) Run the app (Vite + Electron)

```
pnpm dev
```

The Electron main process will spawn the runtime from:

```
./runtime/gpu-runtime/target/debug/gpu-runtime
```

You can override the path with:

```
AER_RUNTIME_PATH=/absolute/path/to/gpu-runtime pnpm dev
```

## Packaging (per platform)

Build the native runtime first, then package the Electron app:

```
pnpm build:runtime
pnpm build
pnpm pack

## Packaging (all platforms, single command)

Build runtime + renderer and package macOS, Windows, and Linux in one shot:

```
pnpm pack:all
```

Notes:
- This runs `electron-builder -mwl`. Cross-platform builds require the right tooling (e.g., Windows/Linux usually need CI or Docker on macOS).
- The runtime binary is built for the host OS only; for true multi-platform releases, build runtime per OS (CI matrix recommended).
```

Electron Builder will bundle the release runtime from:

```
./runtime/gpu-runtime/target/release
```

and emit platform-specific artifacts:

- Windows: NSIS installer (`.exe`)
- macOS: `.dmg`
- Linux: AppImage + `.deb`

## Runtime IPC (JSON-RPC over stdio)

Requests are newline-delimited JSON:

```
{ "id": 1, "method": "list_devices", "params": {} }
```

Responses include `result` or `error`:

```
{ "id": 1, "result": { "devices": [...] } }
```

## Next integration steps

- Add model registry + caching into `runtime/gpu-runtime`.
- Add streaming outputs to IPC (token streaming, progress events).
- Wire per-platform CI that builds and signs artifacts.
- Configure updates + crash reporting (Sentry DSN + electron-updater publish targets).

## Whisper prerequisites

The runtime assumes the following binaries/models are available on the host:

- `ffmpeg` available in `PATH` (or set in the UI).
- `whisper-cli` built from `whisper.cpp` (default: `./build/bin/whisper-cli`).
- Models at:
  - `models/ggml-large-v3.bin`
  - `models/ggml-silero-v6.2.0.bin`

You can override these paths in the UI before running a job.
If you need to force a specific Vulkan ICD, set `VK_ICD_FILENAMES` in the settings panel.

## All-in-one packaging (AIO)

To ship a single installer with everything bundled, place these assets per platform:

- `resources/runtime-assets/bin/whisper-cli` (or `.exe`)
- `resources/runtime-assets/bin/ffmpeg` (or `.exe`)
- `resources/runtime-assets/models/ggml-large-v3.bin`
- `resources/runtime-assets/models/ggml-silero-v6.2.0.bin`

During packaging, electron-builder copies them into `runtime/assets` next to the runtime binary. The runtime auto-discovers bundled assets, so end users do not need to install dependencies manually.

## CI asset download (checksummed)

Use the manifest-driven downloader to fetch binaries/models per platform:

```
pnpm assets:download
```

Edit `scripts/assets-manifest.json` with platform-specific URLs and SHA256 values. The script verifies checksums and sets executable bits on macOS/Linux.

## Supported inputs

- Video: `.mp4`, `.mkv`, `.mov`
- Audio: `.wav`, `.mp3`, `.m4a`

The runtime writes `.srt` files alongside the input file by default (or to the output directory you specify).

## macOS note

wgpu uses Metal on macOS. This preserves the "GPU acceleration everywhere" goal, while still using Vulkan on Windows/Linux. If you need strict Vulkan on macOS, swap to MoltenVK and manage the loader + dylibs in the app bundle.

## Observability

- Main process Sentry uses `SENTRY_DSN`.
- Renderer Sentry uses `VITE_SENTRY_DSN`.

## Testing & coverage

JS/renderer/main/preload coverage is enforced at 100% with Vitest + React Testing Library (jsdom):

```
pnpm test
```

Rust runtime coverage is enforced at 100% via `cargo llvm-cov` (install once with `cargo install cargo-llvm-cov`):

```
pnpm test:rust
```

Run both in one go:

```
pnpm test:all
```

The existing end-to-end pipeline check is still available:

```
pnpm test:e2e
```
