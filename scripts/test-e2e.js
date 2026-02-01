#!/usr/bin/env node
/**
 * End-to-end test script for the audio/video transcription pipeline.
 * 
 * Usage: pnpm test
 * 
 * This script:
 * 1. Builds the Rust runtime (release mode)
 * 2. Downloads required models (Whisper + VAD)
 * 3. Downloads required binaries (whisper-cli, ffmpeg)
 * 4. Transcribes all media files in the sample/ directory
 * 5. Reports results
 */

const fs = require('fs');
const fsp = fs.promises;
const path = require('path');
const { spawn, execSync } = require('child_process');
const https = require('https');
const http = require('http');
const zlib = require('zlib');

const ROOT = path.resolve(__dirname, '..');
const SAMPLE_DIR = path.join(ROOT, 'sample');
const ASSETS_DIR = path.join(ROOT, 'runtime', 'assets');
const MODELS_DIR = path.join(ASSETS_DIR, 'models');
const BIN_DIR = path.join(ASSETS_DIR, 'bin');
const RUNTIME_PATH = path.join(ROOT, 'runtime', 'gpu-runtime', 'target', 'release', 'gpu-runtime');

// Determine platform
const PLATFORM = process.platform;
const ARCH = process.arch;
const IS_MAC = PLATFORM === 'darwin';
const IS_LINUX = PLATFORM === 'linux';
const IS_WINDOWS = PLATFORM === 'win32';
const IS_ARM = ARCH === 'arm64';

// Models to download for testing (using base for faster tests)
const TEST_MODELS = {
  whisper: {
    id: 'base',
    name: 'Whisper Base',
    url: 'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin',
    filename: 'ggml-base.bin',
    sizeBytes: 147951488
  },
  vad: {
    id: 'silero-vad',
    name: 'Silero VAD',
    url: 'https://huggingface.co/ggml-org/whisper-vad/resolve/main/ggml-silero-v5.1.2.bin',
    filename: 'silero_vad.bin',
    sizeBytes: 885098
  }
};

// Binary downloads
function getBinaryUrls() {
  const ffmpegBase = 'https://github.com/eugeneware/ffmpeg-static/releases/download/b6.0';

  if (IS_MAC && IS_ARM) {
    return {
      ffmpeg: {
        url: `${ffmpegBase}/ffmpeg-darwin-arm64.gz`,
        filename: 'ffmpeg',
        compressed: true
      },
      whisper: {
        // Build from source on macOS for Metal support
        buildFromSource: true
      }
    };
  } else if (IS_MAC) {
    return {
      ffmpeg: {
        url: `${ffmpegBase}/ffmpeg-darwin-x64.gz`,
        filename: 'ffmpeg',
        compressed: true
      },
      whisper: {
        buildFromSource: true
      }
    };
  } else if (IS_LINUX) {
    return {
      ffmpeg: {
        url: `${ffmpegBase}/ffmpeg-linux-x64.gz`,
        filename: 'ffmpeg',
        compressed: true
      },
      whisper: {
        buildFromSource: true
      }
    };
  } else if (IS_WINDOWS) {
    return {
      ffmpeg: {
        url: `${ffmpegBase}/ffmpeg-win32-x64.gz`,
        filename: 'ffmpeg.exe',
        compressed: true
      },
      whisper: {
        buildFromSource: true
      }
    };
  }

  throw new Error(`Unsupported platform: ${PLATFORM}-${ARCH}`);
}

const MEDIA_EXTENSIONS = ['mp4', 'mkv', 'mov', 'wav', 'mp3', 'm4a', 'webm', 'avi'];

// ─────────────────────────────────────────────────────────────────────────────
// Utilities
// ─────────────────────────────────────────────────────────────────────────────

function log(message) {
  console.log(`\x1b[36m[test]\x1b[0m ${message}`);
}

function success(message) {
  console.log(`\x1b[32m✓\x1b[0m ${message}`);
}

function error(message) {
  console.error(`\x1b[31m✗\x1b[0m ${message}`);
}

function formatBytes(bytes) {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

function runCommand(command, args, options = {}) {
  return new Promise((resolve, reject) => {
    log(`Running: ${command} ${args.join(' ')}`);
    const proc = spawn(command, args, {
      stdio: options.silent ? 'pipe' : 'inherit',
      cwd: options.cwd || ROOT,
      shell: IS_WINDOWS,
      ...options
    });

    let stdout = '';
    let stderr = '';

    if (options.silent) {
      proc.stdout?.on('data', (data) => { stdout += data.toString(); });
      proc.stderr?.on('data', (data) => { stderr += data.toString(); });
    }

    proc.on('error', reject);
    proc.on('close', (code) => {
      if (code !== 0) {
        reject(new Error(`Command failed with code ${code}\n${stderr}`));
      } else {
        resolve({ stdout, stderr });
      }
    });
  });
}

function downloadFile(url, destPath, onProgress) {
  return new Promise((resolve, reject) => {
    const client = url.startsWith('https') ? https : http;

    const makeRequest = (requestUrl) => {
      client.get(requestUrl, { headers: { 'User-Agent': 'AER-Test' } }, (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          makeRequest(res.headers.location);
          return;
        }

        if (res.statusCode >= 400) {
          reject(new Error(`Download failed with status ${res.statusCode} for ${url}`));
          return;
        }

        const totalBytes = parseInt(res.headers['content-length'], 10) || 0;
        let downloadedBytes = 0;

        const file = fs.createWriteStream(destPath);

        res.on('data', (chunk) => {
          downloadedBytes += chunk.length;
          if (onProgress && totalBytes > 0) {
            onProgress(downloadedBytes, totalBytes);
          }
        });

        res.pipe(file);

        file.on('finish', () => {
          file.close(() => resolve(destPath));
        });

        file.on('error', (err) => {
          fs.unlinkSync(destPath);
          reject(err);
        });

        res.on('error', (err) => {
          fs.unlinkSync(destPath);
          reject(err);
        });
      }).on('error', reject);
    };

    makeRequest(url);
  });
}

async function extractGzip(gzPath, destPath) {
  const compressed = await fsp.readFile(gzPath);
  const decompressed = zlib.gunzipSync(compressed);
  await fsp.writeFile(destPath, decompressed);
  await fsp.rm(gzPath, { force: true });
  if (!IS_WINDOWS) {
    await fsp.chmod(destPath, 0o755);
  }
}

async function extractZip(zipPath, destDir, binaryName) {
  // Use unzip command on macOS/Linux, PowerShell on Windows
  if (IS_WINDOWS) {
    execSync(`powershell -command "Expand-Archive -Path '${zipPath}' -DestinationPath '${destDir}' -Force"`, { stdio: 'inherit' });
  } else {
    execSync(`unzip -o "${zipPath}" -d "${destDir}"`, { stdio: 'inherit' });
  }
  await fsp.rm(zipPath, { force: true });

  // Find and move the binary to the expected location
  const destBinary = path.join(destDir, binaryName);
  if (!fs.existsSync(destBinary)) {
    // Look for the binary in subdirectories
    const files = await fsp.readdir(destDir, { recursive: true });
    for (const file of files) {
      if (path.basename(file) === binaryName || path.basename(file) === binaryName.replace('-cli', '')) {
        const srcPath = path.join(destDir, file);
        if (fs.statSync(srcPath).isFile()) {
          await fsp.rename(srcPath, destBinary);
          break;
        }
      }
    }
  }

  if (!IS_WINDOWS && fs.existsSync(destBinary)) {
    await fsp.chmod(destBinary, 0o755);
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// Build Steps
// ─────────────────────────────────────────────────────────────────────────────

async function buildRuntime() {
  log('Building Rust runtime (release mode)...');

  await runCommand('cargo', [
    'build',
    '--release',
    '--manifest-path',
    path.join(ROOT, 'runtime', 'gpu-runtime', 'Cargo.toml')
  ]);

  if (!fs.existsSync(RUNTIME_PATH)) {
    throw new Error(`Runtime binary not found at ${RUNTIME_PATH}`);
  }

  success('Runtime built successfully');
}

async function ensureDirectories() {
  await fsp.mkdir(MODELS_DIR, { recursive: true });
  await fsp.mkdir(BIN_DIR, { recursive: true });
}

async function downloadModel(model) {
  const destPath = path.join(MODELS_DIR, model.filename);

  // Check if already downloaded
  if (fs.existsSync(destPath)) {
    const stats = fs.statSync(destPath);
    if (stats.size === model.sizeBytes) {
      success(`${model.name} already downloaded`);
      return destPath;
    }
    // Incomplete download, remove and re-download
    fs.unlinkSync(destPath);
  }

  log(`Downloading ${model.name} (${formatBytes(model.sizeBytes)})...`);

  let lastProgress = 0;
  await downloadFile(model.url, destPath, (downloaded, total) => {
    const progress = Math.floor((downloaded / total) * 100);
    if (progress >= lastProgress + 10) {
      process.stdout.write(`\r  Progress: ${progress}% (${formatBytes(downloaded)} / ${formatBytes(total)})`);
      lastProgress = progress;
    }
  });

  console.log(''); // New line after progress
  success(`${model.name} downloaded`);
  return destPath;
}

async function downloadBinary(name, config) {
  const destPath = path.join(BIN_DIR, config.filename);

  // Check if already exists and is executable
  if (fs.existsSync(destPath)) {
    success(`${name} already available`);
    return destPath;
  }

  if (config.buildFromSource) {
    return null; // Signal that we need to build
  }

  log(`Downloading ${name}...`);

  if (config.compressed) {
    const gzPath = destPath + '.gz';
    await downloadFile(config.url, gzPath);
    await extractGzip(gzPath, destPath);
  } else if (config.archive === 'zip') {
    const zipPath = path.join(BIN_DIR, `${name}.zip`);
    await downloadFile(config.url, zipPath);
    await extractZip(zipPath, BIN_DIR, config.binaryInArchive || config.filename);
  } else {
    await downloadFile(config.url, destPath);
    if (!IS_WINDOWS) {
      await fsp.chmod(destPath, 0o755);
    }
  }

  success(`${name} downloaded`);
  return destPath;
}

async function buildWhisperCpp() {
  const whisperDir = path.join(ROOT, 'deps', 'whisper.cpp');
  const whisperBinary = IS_WINDOWS ? 'whisper-cli.exe' : 'whisper-cli';
  const destPath = path.join(BIN_DIR, whisperBinary);

  // Check if already built
  if (fs.existsSync(destPath)) {
    success('whisper-cli already built');
    return destPath;
  }

  log('Building whisper.cpp from source...');

  // Clone if not exists
  if (!fs.existsSync(whisperDir)) {
    await fsp.mkdir(path.join(ROOT, 'deps'), { recursive: true });
    log('Cloning whisper.cpp...');
    await runCommand('git', [
      'clone',
      '--depth', '1',
      'https://github.com/ggerganov/whisper.cpp.git',
      whisperDir
    ]);
  }

  // Build with CMake
  const buildDir = path.join(whisperDir, 'build');
  await fsp.mkdir(buildDir, { recursive: true });

  // Configure CMake with Metal support on macOS
  const cmakeArgs = [
    '-B', buildDir,
    '-S', whisperDir,
    '-DCMAKE_BUILD_TYPE=Release'
  ];

  if (IS_MAC) {
    cmakeArgs.push('-DGGML_METAL=ON');
  }

  await runCommand('cmake', cmakeArgs, { cwd: whisperDir });

  // Build
  await runCommand('cmake', [
    '--build', buildDir,
    '--config', 'Release',
    '-j', String(require('os').cpus().length)
  ], { cwd: whisperDir });

  // Copy binary to bin directory
  const builtBinary = path.join(buildDir, 'bin', whisperBinary);
  if (!fs.existsSync(builtBinary)) {
    throw new Error(`whisper-cli binary not found after build at ${builtBinary}`);
  }

  await fsp.copyFile(builtBinary, destPath);
  if (!IS_WINDOWS) {
    await fsp.chmod(destPath, 0o755);
  }

  // Copy Metal library on macOS if it exists
  if (IS_MAC) {
    const metalLib = path.join(buildDir, 'bin', 'ggml-metal.metal');
    if (fs.existsSync(metalLib)) {
      await fsp.copyFile(metalLib, path.join(BIN_DIR, 'ggml-metal.metal'));
    }
    const defaultMetal = path.join(buildDir, 'bin', 'default.metallib');
    if (fs.existsSync(defaultMetal)) {
      await fsp.copyFile(defaultMetal, path.join(BIN_DIR, 'default.metallib'));
    }
  }

  success('whisper-cli built successfully');
  return destPath;
}

async function downloadDependencies() {
  log('Checking/downloading required dependencies...');
  await ensureDirectories();

  // Download models
  const whisperModelPath = await downloadModel(TEST_MODELS.whisper);
  const vadPath = await downloadModel(TEST_MODELS.vad);

  // Download binaries
  const binaries = getBinaryUrls();
  const ffmpegPath = await downloadBinary('ffmpeg', binaries.ffmpeg);

  let whisperCliPath;

  // Try to download or build whisper-cli
  if (binaries.whisper.buildFromSource) {
    // First check common locations
    const possiblePaths = [
      path.join(BIN_DIR, IS_WINDOWS ? 'whisper-cli.exe' : 'whisper-cli'),
      path.join(ROOT, 'build', 'bin', IS_WINDOWS ? 'whisper-cli.exe' : 'whisper-cli'),
      '/usr/local/bin/whisper-cli',
      '/usr/bin/whisper-cli'
    ];

    for (const p of possiblePaths) {
      if (fs.existsSync(p)) {
        whisperCliPath = p;
        success(`Found whisper-cli at ${p}`);
        break;
      }
    }

    if (!whisperCliPath) {
      // Check if it's in PATH
      try {
        const cmd = IS_WINDOWS ? 'where whisper-cli 2>nul' : 'which whisper-cli 2>/dev/null';
        const result = execSync(cmd, { encoding: 'utf8' });
        if (result.trim()) {
          whisperCliPath = result.trim().split('\n')[0];
          success(`Found whisper-cli in PATH: ${whisperCliPath}`);
        }
      } catch {
        // Not in PATH, build from source
        whisperCliPath = await buildWhisperCpp();
      }
    }
  } else {
    whisperCliPath = await downloadBinary('whisper-cli', binaries.whisper);
  }

  if (!whisperCliPath) {
    throw new Error('whisper-cli not found and could not be built');
  }

  return {
    whisperModelPath,
    vadPath,
    ffmpegPath,
    whisperCliPath
  };
}

// ─────────────────────────────────────────────────────────────────────────────
// Runtime Communication
// ─────────────────────────────────────────────────────────────────────────────

class RuntimeClient {
  constructor(runtimePath) {
    this.runtimePath = runtimePath;
    this.process = null;
    this.pending = new Map();
    this.rpcCounter = 1;
    this.buffer = '';
  }

  start() {
    return new Promise((resolve, reject) => {
      this.process = spawn(this.runtimePath, [], {
        stdio: ['pipe', 'pipe', 'pipe']
      });

      this.process.stdout.setEncoding('utf8');
      this.process.stdout.on('data', (chunk) => this.handleOutput(chunk));

      this.process.stderr.setEncoding('utf8');
      this.process.stderr.on('data', (chunk) => {
        // Log runtime stderr for debugging
        process.stderr.write(`\x1b[33m[runtime]\x1b[0m ${chunk}`);
      });

      this.process.on('error', reject);
      this.process.on('spawn', () => {
        log('Runtime process started');
        resolve();
      });
    });
  }

  handleOutput(chunk) {
    this.buffer += chunk;
    const lines = this.buffer.split('\n');
    this.buffer = lines.pop() || '';

    for (const line of lines) {
      if (!line.trim()) continue;
      try {
        const message = JSON.parse(line);

        // Handle events (logs from transcription)
        if (message.event) {
          if (message.event === 'log') {
            log(`[runtime] ${message.payload}`);
          }
          continue;
        }

        // Handle RPC responses
        if (message.id && this.pending.has(message.id)) {
          const { resolve, reject } = this.pending.get(message.id);
          this.pending.delete(message.id);

          if (message.error) {
            reject(new Error(message.error.message));
          } else {
            resolve(message.result);
          }
        }
      } catch (err) {
        // Ignore parse errors for non-JSON output
      }
    }
  }

  call(method, params = {}) {
    return new Promise((resolve, reject) => {
      const id = this.rpcCounter++;
      const request = JSON.stringify({ id, method, params });

      this.pending.set(id, { resolve, reject });
      this.process.stdin.write(`${request}\n`);
    });
  }

  async ping() {
    return this.call('ping');
  }

  async transcribe(params) {
    return this.call('transcribe', params);
  }

  stop() {
    if (this.process) {
      this.process.kill();
      this.process = null;
    }
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// Test Runner
// ─────────────────────────────────────────────────────────────────────────────

async function findSampleFiles() {
  if (!fs.existsSync(SAMPLE_DIR)) {
    log('No sample directory found, creating it...');
    await fsp.mkdir(SAMPLE_DIR, { recursive: true });
    return [];
  }

  const files = await fsp.readdir(SAMPLE_DIR);
  const mediaFiles = files.filter((file) => {
    const ext = path.extname(file).slice(1).toLowerCase();
    return MEDIA_EXTENSIONS.includes(ext);
  });

  return mediaFiles.map((file) => path.join(SAMPLE_DIR, file));
}

async function runTranscriptionTest(client, inputPath, deps) {
  const filename = path.basename(inputPath);
  log(`Transcribing: ${filename}`);

  const startTime = Date.now();

  try {
    const result = await client.transcribe({
      input_path: inputPath,
      model_path: deps.whisperModelPath,
      vad_model_path: deps.vadPath,
      whisper_path: deps.whisperCliPath,
      ffmpeg_path: deps.ffmpegPath,
      threads: Math.max(1, require('os').cpus().length - 1),
      beam_size: 5,
      best_of: 5,
      max_len_chars: 60,
      split_on_word: true,
      vad_threshold: 0.35,
      vad_min_speech_ms: 200,
      vad_min_sil_ms: 250,
      vad_pad_ms: 80,
      no_speech_thold: 0.75,
      max_context: 0,
      dedup_merge_gap_sec: 0.6,
      translate: false,
      language: 'auto'
    });

    const duration = ((Date.now() - startTime) / 1000).toFixed(1);
    success(`Completed ${filename} in ${duration}s`);

    if (result.outputs && result.outputs.length > 0) {
      for (const output of result.outputs) {
        log(`  Output: ${output}`);
      }
    }

    return { success: true, file: filename, duration, outputs: result.outputs };
  } catch (err) {
    const duration = ((Date.now() - startTime) / 1000).toFixed(1);
    error(`Failed ${filename} after ${duration}s: ${err.message}`);
    return { success: false, file: filename, duration, error: err.message };
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// GPU Detection
// ─────────────────────────────────────────────────────────────────────────────

function detectGpuBackend() {
  if (IS_MAC) {
    return {
      backend: 'Metal',
      description: 'Apple Metal GPU acceleration',
      icon: '🍎'
    };
  } else if (IS_LINUX || IS_WINDOWS) {
    // Vulkan support would need additional setup
    return {
      backend: 'Vulkan',
      description: 'Vulkan GPU acceleration (if available)',
      icon: '🔺'
    };
  }
  return {
    backend: 'CPU',
    description: 'CPU fallback (no GPU acceleration)',
    icon: '💻'
  };
}

// ─────────────────────────────────────────────────────────────────────────────
// Main
// ─────────────────────────────────────────────────────────────────────────────

async function main() {
  console.log('\n\x1b[1m━━━ AER Subtly E2E Test ━━━\x1b[0m\n');

  const gpuInfo = detectGpuBackend();
  console.log(`Platform: ${PLATFORM}-${ARCH}`);
  console.log(`GPU Backend: ${gpuInfo.icon} ${gpuInfo.backend} - ${gpuInfo.description}\n`);

  const startTime = Date.now();
  let client = null;

  try {
    // Step 1: Build runtime
    await buildRuntime();

    // Step 2: Download models and binaries
    const deps = await downloadDependencies();
    log(`Whisper model: ${deps.whisperModelPath}`);
    log(`VAD model: ${deps.vadPath}`);
    log(`FFmpeg: ${deps.ffmpegPath}`);
    log(`Whisper CLI: ${deps.whisperCliPath}`);

    // Step 3: Find sample files
    const sampleFiles = await findSampleFiles();

    if (sampleFiles.length === 0) {
      log('No sample media files found in sample/ directory');
      log('Add .mp4, .mkv, .wav, .mp3 or other media files to sample/ to test transcription');
      console.log('\n\x1b[33m⚠ Test completed with no files to process\x1b[0m\n');
      process.exit(0);
    }

    log(`Found ${sampleFiles.length} sample file(s)`);

    // Step 4: Start runtime
    client = new RuntimeClient(RUNTIME_PATH);
    await client.start();

    // Ping to verify runtime is ready and check GPU status
    const pingResult = await client.ping();

    // Validate ping response structure matches frontend schema
    if (!pingResult.message || typeof pingResult.message !== 'string') {
      throw new Error('Ping response missing required "message" field');
    }
    if (!pingResult.gpu_backend || typeof pingResult.gpu_backend !== 'string') {
      throw new Error('Ping response missing required "gpu_backend" field');
    }

    if (pingResult.gpu_enabled) {
      success(`Runtime ready with GPU: ${pingResult.gpu_name} (${pingResult.gpu_backend})`);
    } else {
      log(`\x1b[33m⚠ Runtime running on CPU (no GPU detected)\x1b[0m`);
    }

    // Step 5: Run transcription on each file
    const results = [];
    for (const file of sampleFiles) {
      const result = await runTranscriptionTest(client, file, deps);
      results.push(result);
    }

    // Step 6: Report results
    const totalDuration = ((Date.now() - startTime) / 1000).toFixed(1);
    const passed = results.filter((r) => r.success).length;
    const failed = results.filter((r) => !r.success).length;

    console.log('\n\x1b[1m━━━ Test Results ━━━\x1b[0m\n');
    console.log(`  Total files:  ${results.length}`);
    console.log(`  \x1b[32mPassed:\x1b[0m       ${passed}`);
    console.log(`  \x1b[31mFailed:\x1b[0m       ${failed}`);
    console.log(`  Total time:   ${totalDuration}s`);

    if (failed > 0) {
      console.log('\n\x1b[31mFailed tests:\x1b[0m');
      for (const result of results.filter((r) => !r.success)) {
        console.log(`  - ${result.file}: ${result.error}`);
      }
      process.exit(1);
    }

    console.log('\n\x1b[32m✓ All tests passed!\x1b[0m\n');
    process.exit(0);

  } catch (err) {
    error(err.message);
    console.error(err.stack);
    process.exit(1);
  } finally {
    if (client) {
      client.stop();
    }
  }
}

main();
