# GEMINI.md - Context & Instructions

## Project Overview

**Subtly** is an Electron-based desktop application designed to generate subtitles for audio and video files using the Whisper model. It leverages a split architecture to ensure stability and performance:
*   **Frontend/UI:** Electron + React + Vite (handles user interaction and orchestration).
*   **Backend/Runtime:** A native Rust "sidecar" process (`gpu-runtime`) that manages GPU acceleration (wgpu via Vulkan/Metal) and the Whisper inference pipeline.

This architecture isolates the GPU-heavy inference tasks from the UI, preventing crashes in the rendering layer.

## Architecture & Design

*   **Electron Main Process:** Spawns and manages the Rust sidecar.
*   **Rust Sidecar (`gpu-runtime`):** Communicates with the main process via JSON-RPC over stdio. It handles device enumeration, model loading, and compute jobs.
*   **IPC Protocol:**
    *   Requests: `{"id": 1, "method": "...", "params": {...}}\n`
    *   Responses: `{"id": 1, "result": {...}}\n` or `{"id": 1, "error": {...}}\n`
*   **GPU Abstraction:** Uses `wgpu`, mapping to Vulkan on Windows/Linux and Metal on macOS.

## Building and Running

### Prerequisites
*   **Node.js** (and `yarn`)
*   **Rust** (Cargo)
*   **FFmpeg** (runtime dependency, usually expected in PATH or bundled)
*   **Whisper CLI** (built from `whisper.cpp`, expected in `runtime/assets` or build dir)

### Key Commands

**1. Setup & Installation**
```bash
yarn install
```

**2. Development**
Runs Vite (Renderer) and Electron (Main), spawning the Rust runtime in debug mode.
```bash
# Build the runtime first
yarn build:runtime

# Start the app
yarn dev
```
*Note: You can override the runtime path with `AER_RUNTIME_PATH=/path/to/runtime yarn dev`.*

**3. Building for Production**
Builds the release version of the runtime and packages the Electron app.
```bash
# Build Rust runtime (release)
yarn build:runtime

# Build Renderer & Main
yarn build

# Package for current OS
yarn pack

# Package for all platforms (requires cross-compilation env)
yarn pack:all
```

**4. Asset Management**
Downloads platform-specific binaries/models (ffmpeg, whisper-cli, models) based on `scripts/assets-manifest.json`.
```bash
yarn assets:download
```

**5. Testing**
```bash
# Run JS/React unit tests
yarn test

# Run Rust runtime tests
yarn test:rust

# Run End-to-End tests
yarn test:e2e

# Run all tests
yarn test:all
```

## Project Structure

*   `src/`
    *   `main/`: Electron main process (lifecycle, IPC, window management).
    *   `renderer/`: React UI (Vite entry point).
    *   `preload/`: Context bridge for renderer-main communication.
    *   `shared/`: Utilities shared between main and renderer.
*   `runtime/`
    *   `gpu-runtime/`: Rust crate for the GPU/Whisper sidecar.
    *   `assets/`: Directory where runtime looks for bundled binaries/models.
*   `resources/`
    *   `runtime-assets/`: Source location for assets to be bundled by electron-builder.
*   `scripts/`: Build, dev, and asset management scripts.
*   `dist/`: Compiled frontend output.
*   `release/`: Final packaged installers/binaries.

## Development Conventions

*   **Languages:**
    *   **JavaScript/JSX:** 2-space indent, single quotes, camelCase. React components use PascalCase filenames.
    *   **Rust:** Standard `rustfmt` style. `snake_case` for modules/functions.
*   **Commits:** Use short, imperative subjects (e.g., "Fix runtime build").
*   **Testing:**
    *   Maintain 100% coverage for JS/React logic (`vitest`).
    *   Maintain 100% coverage for Rust runtime (`cargo llvm-cov`).
*   **Versioning:** Follows Semantic Versioning in `package.json`.

## Key Files

*   `package.json`: Project scripts, dependencies, and electron-builder configuration.
*   `runtime/gpu-runtime/Cargo.toml`: Rust workspace definition and dependencies.
*   `scripts/assets-manifest.json`: Configuration for downloading external binaries and models.
*   `AGENTS.md`: Detailed module organization and coding guidelines.
