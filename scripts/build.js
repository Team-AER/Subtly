const fs = require('fs');
const fsp = fs.promises;
const path = require('path');
const { spawnSync } = require('child_process');

const root = path.resolve(__dirname, '..');
const distDir = path.join(root, 'dist');
const srcMain = path.join(root, 'src', 'main');
const srcShared = path.join(root, 'src', 'shared');

async function copyDir(src, dest, clean = true) {
  if (clean) {
    await fsp.rm(dest, { recursive: true, force: true });
  }
  await fsp.mkdir(dest, { recursive: true });
  const entries = await fsp.readdir(src, { withFileTypes: true });
  for (const entry of entries) {
    const from = path.join(src, entry.name);
    const to = path.join(dest, entry.name);
    if (entry.isDirectory()) {
      await copyDir(from, to);
    } else {
      await fsp.copyFile(from, to);
    }
  }
}

async function buildRenderer() {
  let viteBin;
  try {
    // Resolve the vite package directory, then construct the path to the binary
    const vitePackage = require.resolve('vite/package.json', { paths: [root] });
    const viteDir = path.dirname(vitePackage);
    viteBin = path.join(viteDir, 'bin', 'vite.js');

    // Verify the binary exists
    if (!fs.existsSync(viteBin)) {
      throw new Error('Vite binary not found at: ' + viteBin);
    }
  } catch (err) {
    console.error('Vite is not installed. Ensure devDependencies are installed (avoid NODE_ENV=production during install).');
    throw err;
  }

  const result = spawnSync(process.execPath, [viteBin, 'build'], {
    cwd: root,
    stdio: 'inherit'
  });
  if (result.error) {
    console.error(result.error);
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

function buildWhisperCpp() {
  const scriptPath = path.join(root, 'scripts', 'build-whisper.sh');
  if (!fs.existsSync(scriptPath)) {
    throw new Error(`Whisper build script not found at ${scriptPath}`);
  }

  const result = spawnSync('bash', [scriptPath], {
    cwd: root,
    stdio: 'inherit',
    env: {
      ...process.env
    }
  });

  if (result.error) {
    if (result.error.code === 'ENOENT') {
      console.error('bash not found. Install bash (macOS/Linux or Git Bash on Windows) to build whisper.cpp.');
    }
    throw result.error;
  }

  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

async function buildAll() {
  await fsp.mkdir(distDir, { recursive: true });
  await buildRenderer();
  await copyDir(srcMain, distDir, false);
  await copyDir(srcShared, path.join(distDir, 'shared'));
  await copyWhisperCli();
}

async function findFiles(dir, predicate) {
  const results = [];
  const entries = await fsp.readdir(dir, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      results.push(...await findFiles(fullPath, predicate));
    } else if (predicate(entry.name, fullPath)) {
      results.push(fullPath);
    }
  }
  return results;
}

function ensureLoaderPathRpath(targetPath) {
  const result = spawnSync('install_name_tool', ['-add_rpath', '@loader_path', targetPath], {
    stdio: 'pipe'
  });
  if (result.status === 0) {
    return;
  }

  const stderr = String(result.stderr || '');
  if (stderr.includes('already has rpath') || stderr.includes('file already has LC_RPATH')) {
    return;
  }

  throw new Error(`install_name_tool failed for ${targetPath}: ${stderr || result.error || 'unknown error'}`);
}

async function copyWhisperCli() {
  buildWhisperCpp();

  const isWindows = process.platform === 'win32';
  const isMac = process.platform === 'darwin';
  const whisperCliName = isWindows ? 'whisper-cli.exe' : 'whisper-cli';
  const ggmlMetalName = 'ggml-metal.metal';

  const whisperCliCandidates = isWindows
    ? [
        path.join(root, 'deps', 'whisper.cpp', 'build', 'bin', 'Release', whisperCliName),
        path.join(root, 'deps', 'whisper.cpp', 'build', 'bin', whisperCliName)
      ]
    : [
        path.join(root, 'deps', 'whisper.cpp', 'build', 'bin', whisperCliName),
        path.join(root, 'deps', 'whisper.cpp', 'build', 'bin', 'Release', whisperCliName)
      ];

  const sourceWhisperCli = whisperCliCandidates.find((candidate) => fs.existsSync(candidate));
  const sourceGgmlMetalCandidates = [
    path.join(root, 'deps', 'whisper.cpp', 'build', 'bin', ggmlMetalName),
    path.join(root, 'deps', 'whisper.cpp', 'build', 'bin', 'Release', ggmlMetalName)
  ];
  const sourceGgmlMetal = sourceGgmlMetalCandidates.find((candidate) => fs.existsSync(candidate));
  const destBinDir = path.join(root, 'resources', 'runtime-assets', 'bin');
  const destWhisperCli = path.join(destBinDir, whisperCliName);
  const destGgmlMetal = path.join(destBinDir, ggmlMetalName);

  // Ensure destination directory exists
  await fsp.mkdir(destBinDir, { recursive: true });

  // Check if whisper-cli exists
  if (!sourceWhisperCli) {
    throw new Error(`whisper-cli not found. Checked: ${whisperCliCandidates.join(', ')}`);
  }

  // Copy whisper-cli
  await fsp.copyFile(sourceWhisperCli, destWhisperCli);
  console.log(`Copied whisper-cli to ${destWhisperCli}`);

  // Set executable permissions on macOS/Linux
  if (!isWindows) {
    await fsp.chmod(destWhisperCli, 0o755);
  }

  if (isWindows) {
    const sourceDir = path.dirname(sourceWhisperCli);
    const entries = await fsp.readdir(sourceDir);
    const dlls = entries.filter((entry) => entry.toLowerCase().endsWith('.dll'));
    for (const dll of dlls) {
      const src = path.join(sourceDir, dll);
      const dest = path.join(destBinDir, dll);
      await fsp.copyFile(src, dest);
      console.log(`Copied ${dll} to ${dest}`);
    }
  }

  // Copy ggml-metal.metal if it exists (required on macOS)
  if (sourceGgmlMetal) {
    await fsp.copyFile(sourceGgmlMetal, destGgmlMetal);
    console.log(`Copied ggml-metal.metal to ${destGgmlMetal}`);
  }

  if (isMac) {
    const whisperBuildDir = process.env.WHISPER_CPP_BUILD_DIR
      ? path.resolve(process.env.WHISPER_CPP_BUILD_DIR)
      : path.join(root, 'deps', 'whisper.cpp', 'build');
    const dylibPattern = /^lib(whisper|ggml).*\.dylib$/;
    const dylibSources = fs.existsSync(whisperBuildDir)
      ? await findFiles(whisperBuildDir, (name) => dylibPattern.test(name))
      : [];

    if (dylibSources.length === 0) {
      console.warn(`No whisper dylibs found under ${whisperBuildDir}; skipping dylib copy.`);
      return;
    }

    for (const dylibSource of dylibSources) {
      const destDylib = path.join(destBinDir, path.basename(dylibSource));
      await fsp.copyFile(dylibSource, destDylib);
      console.log(`Copied ${path.basename(dylibSource)} to ${destDylib}`);
      ensureLoaderPathRpath(destDylib);
    }

    ensureLoaderPathRpath(destWhisperCli);
  }
}


buildAll().catch((err) => {
  console.error(err);
  process.exit(1);
});
