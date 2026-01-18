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
    viteBin = require.resolve('vite/bin/vite.js', { paths: [root] });
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

async function buildAll() {
  await fsp.mkdir(distDir, { recursive: true });
  await buildRenderer();
  await copyDir(srcMain, distDir, false);
  await copyDir(srcShared, path.join(distDir, 'shared'));
}

buildAll().catch((err) => {
  console.error(err);
  process.exit(1);
});
