const fs = require('fs');
const fsp = fs.promises;
const path = require('path');

const root = path.resolve(__dirname, '..');
const sourceDir = path.join(root, 'resources', 'runtime-assets');
const targetDir = path.join(root, 'runtime', 'assets');

async function copyDir(src, dest) {
  await fsp.mkdir(dest, { recursive: true });
  const entries = await fsp.readdir(src, { withFileTypes: true });

  for (const entry of entries) {
    const from = path.join(src, entry.name);
    const to = path.join(dest, entry.name);

    if (entry.isDirectory()) {
      await copyDir(from, to);
      continue;
    }

    await fsp.copyFile(from, to);

    if (process.platform !== 'win32') {
      const stats = await fsp.stat(from);
      await fsp.chmod(to, stats.mode);
    }
  }
}

async function main() {
  if (!fs.existsSync(sourceDir)) {
    throw new Error(`Source assets not found at ${sourceDir}`);
  }

  await fsp.rm(targetDir, { recursive: true, force: true });
  await copyDir(sourceDir, targetDir);
  console.log(`Synced assets to ${targetDir}`);
}

main().catch((err) => {
  console.error(err.message || err);
  process.exit(1);
});
