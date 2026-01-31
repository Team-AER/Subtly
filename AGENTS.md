# Repository Guidelines

## Project Structure & Module Organization
- `src/main`: Electron main process code (app lifecycle, windows, IPC).
- `src/preload`: Preload bridge exposed to the renderer.
- `src/renderer`: React UI and state; entry at `src/renderer/main.jsx`.
- `src/shared`: Shared utilities used by main/renderer.
- `runtime/gpu-runtime`: Rust JSON-RPC sidecar for GPU/Whisper processing.
- `resources/runtime-assets`: Bundled binaries/models for all-in-one packaging.
- Build outputs: `dist/` for app bundles, `release/` for electron-builder artifacts.

## Build, Test, and Development Commands
- `pnpm dev`: Start Vite + Electron for local development.
- `pnpm build`: Build the renderer and copy main/shared into `dist/`.
- `pnpm build:runtime`: Build the Rust sidecar (`runtime/gpu-runtime`).
- `pnpm pack`: Package the app for the current platform.
- `pnpm pack:all`: Build + package macOS/Windows/Linux in one command.
- `pnpm assets:download`: Download platform assets using `scripts/assets-manifest.json`.

## Coding Style & Naming Conventions
- JavaScript/JSX: 2-space indentation, single quotes, `camelCase` for variables/functions.
- React components: `PascalCase` filenames and exports (e.g., `App.jsx`).
- Rust: follow `rustfmt` defaults; prefer `snake_case` for functions/modules.
- No lint/format scripts are defined; keep diffs minimal and consistent with existing files.

## Testing Guidelines
- No automated tests are currently set up.
- If you add tests, document the framework and add a `pnpm test` script.

## Commit & Pull Request Guidelines
- There is no Git history yet, so no established commit style.
- Use short, imperative commit subjects (e.g., “Fix runtime build”).
- PRs should include: a clear description, build/pack steps run, and UI screenshots when renderer changes.

## Configuration & Packaging Notes
- Runtime binaries/models can be bundled under `resources/runtime-assets`.
- Use `AER_RUNTIME_PATH` to point the app at a custom runtime binary during dev.
- Cross-platform packaging may require CI/Docker; runtime binaries must match target OS.
