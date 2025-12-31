Packaged runtime assets.

Copy platform-specific binaries and models into this folder before packaging:

- bin/whisper-cli (or whisper-cli.exe on Windows)
- bin/ffmpeg (or ffmpeg.exe on Windows)
- models/ggml-large-v3.bin
- models/ggml-silero-v6.2.0.bin

These will be bundled into the app at runtime/assets for the AIO experience.
