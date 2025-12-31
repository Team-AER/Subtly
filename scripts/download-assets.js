const fs = require('fs');
const fsp = fs.promises;
const path = require('path');
const crypto = require('crypto');
const http = require('http');
const https = require('https');
const zlib = require('zlib');

const root = path.resolve(__dirname, '..');
const manifestPath = process.env.ASSET_MANIFEST
  ? path.resolve(process.env.ASSET_MANIFEST)
  : path.join(__dirname, 'assets-manifest.json');

function readManifest() {
  const raw = fs.readFileSync(manifestPath, 'utf8');
  return JSON.parse(raw);
}

function getPlatformKey() {
  return `${process.platform}-${process.arch}`;
}

function ensureDir(filePath) {
  return fsp.mkdir(path.dirname(filePath), { recursive: true });
}

function downloadFile(url, destPath) {
  return new Promise((resolve, reject) => {
    const client = url.startsWith('https') ? https : http;
    client.get(url, (res) => {
      if (res.statusCode && res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        return resolve(downloadFile(res.headers.location, destPath));
      }
      if (!res.statusCode || res.statusCode >= 400) {
        return reject(new Error(`Download failed (${res.statusCode}) for ${url}`));
      }

      const hash = crypto.createHash('sha256');
      const file = fs.createWriteStream(destPath);

      res.on('data', (chunk) => {
        hash.update(chunk);
      });

      res.pipe(file);

      file.on('finish', () => {
        file.close(() => resolve(hash.digest('hex')));
      });

      file.on('error', (err) => reject(err));
    }).on('error', reject);
  });
}

async function downloadAsset(asset) {
  const destPath = path.resolve(root, asset.dest);
  await ensureDir(destPath);

  console.log(`Downloading ${asset.name} -> ${asset.dest}`);
  const actualHash = await downloadFile(asset.url, destPath);
  if (asset.sha256 && asset.sha256 !== 'REPLACE_ME' && actualHash !== asset.sha256) {
    await fsp.rm(destPath, { force: true });
    throw new Error(`Checksum mismatch for ${asset.name}: expected ${asset.sha256}, got ${actualHash}`);
  }

  // Handle gzip extraction
  if (asset.extract === 'gunzip' && destPath.endsWith('.gz')) {
    const extractedPath = destPath.slice(0, -3); // Remove .gz
    console.log(`Extracting ${asset.name} -> ${extractedPath}`);
    const compressed = await fsp.readFile(destPath);
    const decompressed = zlib.gunzipSync(compressed);
    await fsp.writeFile(extractedPath, decompressed);
    await fsp.rm(destPath, { force: true });
    
    if (asset.mode && process.platform !== 'win32') {
      await fsp.chmod(extractedPath, parseInt(asset.mode, 8));
    }
  } else if (asset.mode && process.platform !== 'win32') {
    await fsp.chmod(destPath, parseInt(asset.mode, 8));
  }

  console.log(`Verified ${asset.name} (${actualHash})`);
}

async function main() {
  if (!fs.existsSync(manifestPath)) {
    throw new Error(`Manifest not found: ${manifestPath}`);
  }

  const manifest = readManifest();
  const platformKey = getPlatformKey();
  const assets = manifest.platforms?.[platformKey] ?? [];

  if (!assets.length) {
    console.log(`No assets configured for ${platformKey}. Update ${manifestPath}.`);
    return;
  }

  for (const asset of assets) {
    if (!asset.url || !asset.dest) {
      throw new Error(`Asset missing url/dest: ${JSON.stringify(asset)}`);
    }
    await downloadAsset(asset);
  }
}

main().catch((err) => {
  console.error(err.message || err);
  process.exit(1);
});
