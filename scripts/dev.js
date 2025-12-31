const { spawn } = require('child_process');
const fs = require('fs');
const fsp = fs.promises;
const http = require('http');
const path = require('path');

const root = path.resolve(__dirname, '..');
const viteBin = path.join(root, 'node_modules', '.bin', process.platform === 'win32' ? 'vite.cmd' : 'vite');
const electronBin = path.join(root, 'node_modules', '.bin', process.platform === 'win32' ? 'electron.cmd' : 'electron');
const distDir = path.join(root, 'dist');
const srcMain = path.join(root, 'src', 'main');
const srcShared = path.join(root, 'src', 'shared');

const VITE_URL = 'http://localhost:5173';

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
      await copyDir(from, to, clean);
    } else {
      await fsp.copyFile(from, to);
    }
  }
}

async function syncMain() {
  await fsp.mkdir(distDir, { recursive: true });
  await copyDir(srcMain, distDir, false);
  await copyDir(srcShared, path.join(distDir, 'shared'));
}

function watchMain() {
  const schedule = (() => {
    let timer = null;
    return () => {
      clearTimeout(timer);
      timer = setTimeout(() => {
        syncMain().catch((err) => console.error('Main sync failed', err));
      }, 100);
    };
  })();

  [srcMain, srcShared].forEach((dir) => {
    try {
      fs.watch(dir, { recursive: true }, schedule);
    } catch (err) {
      console.warn(`Watch not supported for ${dir}:`, err.message);
    }
  });
}

function waitForVite() {
  return new Promise((resolve) => {
    const start = Date.now();
    const interval = setInterval(() => {
      http
        .get(VITE_URL, (res) => {
          if (res.statusCode && res.statusCode >= 200 && res.statusCode < 500) {
            clearInterval(interval);
            resolve();
          }
        })
        .on('error', () => {
          if (Date.now() - start > 20000) {
            clearInterval(interval);
            resolve();
          }
        });
    }, 300);
  });
}

syncMain()
  .then(() => {
    watchMain();

    const vite = spawn(viteBin, ['--strictPort'], {
      cwd: root,
      stdio: 'inherit',
      env: {
        ...process.env
      }
    });

    vite.on('exit', (code) => {
      process.exit(code ?? 0);
    });

    waitForVite().then(() => {
      const electron = spawn(electronBin, ['.'], {
        cwd: root,
        stdio: 'inherit',
        env: {
          ...process.env,
          VITE_DEV_SERVER_URL: VITE_URL
        }
      });

      electron.on('exit', (code) => {
        vite.kill();
        process.exit(code ?? 0);
      });
    });
  })
  .catch((err) => {
    console.error(err);
    process.exit(1);
  });
