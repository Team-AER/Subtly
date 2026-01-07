# Subtly

This document provides a technical overview of the Subtly application, intended for developers and contributors.

## Project Overview

Subtly is a desktop application for generating subtitles from audio and video files. It uses a hybrid architecture combining a web-based user interface with a high-performance, native backend for GPU-accelerated transcription.

### Core Technologies

*   **Frontend:** The user interface is built with **React** and **Vite**. Styling is handled by **Tailwind CSS**.
*   **Application Shell:** **Electron** is used to wrap the web frontend into a cross-platform desktop application.
*   **Backend/Runtime:** A **Rust** sidecar process (`gpu-runtime`) manages transcription tasks. It leverages **wgpu** to interface with the GPU (Vulkan on Windows/Linux, Metal on macOS) and runs **Whisper.cpp** for the actual speech-to-text conversion.
*   **Communication:** The Electron main process and the Rust sidecar communicate via **JSON-RPC** over `stdio`.

## Getting Started

### Prerequisites

*   Node.js and Yarn
*   Rust and Cargo
*   `ffmpeg` (must be in the system's `PATH`)

### Development

1.  **Install Dependencies:**
    ```bash
    yarn install
    ```

2.  **Build the Rust Runtime:**
    ```bash
    cargo build --manifest-path runtime/gpu-runtime/Cargo.toml
    ```

3.  **Run the Application:**
    This command starts the Vite development server and launches the Electron app.
    ```bash
    yarn dev
    ```

### Building for Production

1.  **Build the Renderer and Main Process:**
    ```bash
    yarn build
    ```

2.  **Build the Release Runtime:**
    ```bash
    yarn build:runtime
    ```

3.  **Package the Application:**
    This will create distributable artifacts (e.g., `.dmg`, `.exe`, `.AppImage`) in the `release` directory.
    ```bash
    yarn pack
    ```

### Testing

The project has a comprehensive test suite.

*   **Run All Tests (JS and Rust):**
    ```bash
    yarn test:all
    ```

*   **Run JavaScript Tests (Vitest):**
    ```bash
    yarn test
    ```

*   **Run Rust Tests:**
    ```bash
    yarn test:rust
    ```

## Project Structure

```
/
├─── src/                     # Electron application source
│   ├─── main/                # Main process
│   ├─── renderer/            # UI (React)
│   └─── shared/              # Code shared between main and renderer
├─── runtime/
│   └─── gpu-runtime/         # Rust sidecar for GPU tasks
├─── scripts/                 # Build and development scripts
├─── resources/               # Assets for the packaged application
└─── deps/                    # Third-party dependencies (like whisper.cpp)
```

## Development Conventions

### Styling

*   **Tailwind CSS:** The project uses Tailwind CSS for all styling.
*   **Custom Theme:** A custom theme is defined in `tailwind.config.js`. Key design tokens include:
    *   **Fonts:** `Space Grotesk` for display text, `IBM Plex Sans` for body text.
    *   **Colors:** A custom palette with `base`, `accent`, and `plasma` colors.

### State Management

*   **Zustand:** The React application uses Zustand for global state management. Store logic can be found in `src/renderer/state/`.

### Inter-Process Communication (IPC)

*   **JSON-RPC:** The Electron main process communicates with the Rust `gpu-runtime` via JSON-RPC messages passed over `stdio`. The defined RPC methods are in `src/shared/rpc.js`.
*   **Electron IPC:** The renderer process communicates with the main process using Electron's `ipcRenderer` and `ipcMain` modules. A preload script (`src/main/preload.js`) exposes safe, asynchronous APIs to the renderer.
