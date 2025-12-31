const path = require('path');
const fs = require('fs');
const https = require('https');
const http = require('http');
const { app, BrowserWindow, ipcMain, dialog } = require('electron');
const { spawn } = require('child_process');
const { RPC_METHODS } = require('./shared/rpc');
const { WHISPER_MODELS, VAD_MODEL } = require('./shared/models');
const { autoUpdater } = require('electron-updater');
const Sentry = require('@sentry/electron/main');

let mainWindow = null;
let runtimeProcess = null;
let rpcCounter = 1;
const pending = new Map();
const activeDownloads = new Map();

if (process.platform === 'win32') {
  app.setAppUserModelId('com.aer.subtitleforge');
}

if (process.env.SENTRY_DSN) {
  Sentry.init({
    dsn: process.env.SENTRY_DSN,
    tracesSampleRate: 0.2
  });
}

function getRuntimeBinaryName() {
  if (process.platform === 'win32') {
    return 'gpu-runtime.exe';
  }
  return 'gpu-runtime';
}

function resolveRuntimePath() {
  if (process.env.AER_RUNTIME_PATH) {
    return process.env.AER_RUNTIME_PATH;
  }

  const binaryName = getRuntimeBinaryName();

  if (app.isPackaged) {
    return path.join(process.resourcesPath, 'runtime', binaryName);
  }

  const appRoot = app.getAppPath();
  return path.join(appRoot, 'runtime', 'gpu-runtime', 'target', 'debug', binaryName);
}

function getModelsDirectory() {
  if (app.isPackaged) {
    return path.join(app.getPath('userData'), 'models');
  }
  return path.join(app.getAppPath(), 'runtime', 'assets', 'models');
}

function ensureModelsDirectory() {
  const modelsDir = getModelsDirectory();
  if (!fs.existsSync(modelsDir)) {
    fs.mkdirSync(modelsDir, { recursive: true });
  }
  return modelsDir;
}

function getInstalledModels() {
  const modelsDir = getModelsDirectory();
  if (!fs.existsSync(modelsDir)) {
    return [];
  }

  const files = fs.readdirSync(modelsDir);
  const installed = [];

  for (const model of WHISPER_MODELS) {
    if (files.includes(model.filename)) {
      const filePath = path.join(modelsDir, model.filename);
      const stats = fs.statSync(filePath);
      installed.push({
        ...model,
        path: filePath,
        installedSize: stats.size,
        complete: stats.size === model.sizeBytes
      });
    }
  }

  // Check VAD model
  if (files.includes(VAD_MODEL.filename)) {
    const filePath = path.join(modelsDir, VAD_MODEL.filename);
    const stats = fs.statSync(filePath);
    installed.push({
      ...VAD_MODEL,
      path: filePath,
      installedSize: stats.size,
      complete: stats.size === VAD_MODEL.sizeBytes
    });
  }

  return installed;
}

function downloadModel(modelId, onProgress) {
  const model = modelId === 'silero-vad'
    ? VAD_MODEL
    : WHISPER_MODELS.find((m) => m.id === modelId);

  if (!model) {
    return Promise.reject(new Error(`Unknown model: ${modelId}`));
  }

  const modelsDir = ensureModelsDirectory();
  const destPath = path.join(modelsDir, model.filename);
  const tempPath = `${destPath}.download`;

  return new Promise((resolve, reject) => {
    const client = model.url.startsWith('https') ? https : http;

    const makeRequest = (url) => {
      client.get(url, (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          makeRequest(res.headers.location);
          return;
        }

        if (res.statusCode >= 400) {
          reject(new Error(`Download failed with status ${res.statusCode}`));
          return;
        }

        const totalBytes = parseInt(res.headers['content-length'], 10) || model.sizeBytes;
        let downloadedBytes = 0;

        const file = fs.createWriteStream(tempPath);
        activeDownloads.set(modelId, { abort: () => res.destroy() });

        res.on('data', (chunk) => {
          downloadedBytes += chunk.length;
          const progress = Math.round((downloadedBytes / totalBytes) * 100);
          onProgress({ modelId, progress, downloadedBytes, totalBytes });
        });

        res.pipe(file);

        file.on('finish', () => {
          file.close(() => {
            fs.renameSync(tempPath, destPath);
            activeDownloads.delete(modelId);
            resolve({ modelId, path: destPath });
          });
        });

        file.on('error', (err) => {
          fs.unlinkSync(tempPath);
          activeDownloads.delete(modelId);
          reject(err);
        });

        res.on('error', (err) => {
          fs.unlinkSync(tempPath);
          activeDownloads.delete(modelId);
          reject(err);
        });
      }).on('error', reject);
    };

    makeRequest(model.url);
  });
}

function cancelDownload(modelId) {
  const download = activeDownloads.get(modelId);
  if (download) {
    download.abort();
    activeDownloads.delete(modelId);
    return true;
  }
  return false;
}

function deleteModel(modelId) {
  const model = modelId === 'silero-vad'
    ? VAD_MODEL
    : WHISPER_MODELS.find((m) => m.id === modelId);

  if (!model) {
    throw new Error(`Unknown model: ${modelId}`);
  }

  const modelsDir = getModelsDirectory();
  const filePath = path.join(modelsDir, model.filename);

  if (fs.existsSync(filePath)) {
    fs.unlinkSync(filePath);
    return true;
  }
  return false;
}

function startRuntime() {
  const runtimePath = resolveRuntimePath();
  runtimeProcess = spawn(runtimePath, [], {
    stdio: ['pipe', 'pipe', 'pipe']
  });

  runtimeProcess.stdout.setEncoding('utf8');
  runtimeProcess.stdout.on('data', (chunk) => {
    const lines = chunk.split('\n').filter(Boolean);
    for (const line of lines) {
      try {
        const message = JSON.parse(line);
        if (message.event && mainWindow) {
          mainWindow.webContents.send('runtime:event', message);
          continue;
        }
        if (message.id && pending.has(message.id)) {
          const { resolve, reject } = pending.get(message.id);
          pending.delete(message.id);
          if (message.error) {
            reject(new Error(message.error.message || 'Runtime error'));
          } else {
            resolve(message.result);
          }
        }
      } catch (err) {
        console.error('Failed to parse runtime output:', err);
      }
    }
  });

  runtimeProcess.stderr.setEncoding('utf8');
  runtimeProcess.stderr.on('data', (chunk) => {
    console.error(`[runtime] ${chunk}`);
  });

  runtimeProcess.on('error', (err) => {
    dialog.showErrorBox('Runtime error', err.message);
  });

  runtimeProcess.on('exit', (code) => {
    console.warn(`Runtime exited with code ${code}`);
    runtimeProcess = null;
  });
}

function sendRpc(method, params = {}) {
  if (!runtimeProcess) {
    throw new Error('Runtime process is not running');
  }

  const id = rpcCounter++;
  const payload = JSON.stringify({ id, method, params });

  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    runtimeProcess.stdin.write(`${payload}\n`);
  });
}

function createWindow() {
  mainWindow = new BrowserWindow({
    width: 1100,
    height: 720,
    backgroundColor: '#0b0f1a',
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false
    }
  });

  const devServerUrl = process.env.VITE_DEV_SERVER_URL;
  if (devServerUrl) {
    mainWindow.loadURL(devServerUrl);
  } else {
    mainWindow.loadFile(path.join(__dirname, 'renderer', 'index.html'));
  }
}

app.whenReady().then(() => {
  try {
    startRuntime();
  } catch (err) {
    dialog.showErrorBox('Runtime startup failed', err.message);
  }

  ipcMain.handle('runtime:ping', () => sendRpc(RPC_METHODS.PING));
  ipcMain.handle('runtime:list-devices', () => sendRpc(RPC_METHODS.LIST_DEVICES));
  ipcMain.handle('runtime:smoke-test', () => sendRpc(RPC_METHODS.SMOKE_TEST));
  ipcMain.handle('runtime:transcribe', (_event, payload) =>
    sendRpc(RPC_METHODS.TRANSCRIBE, payload)
  );

  ipcMain.handle('dialog:open-file', async () => {
    const result = await dialog.showOpenDialog({
      properties: ['openFile'],
      filters: [
        { name: 'Media', extensions: ['mp4', 'mkv', 'mov', 'wav', 'mp3', 'm4a'] }
      ]
    });
    return result.canceled ? null : result.filePaths[0];
  });

  ipcMain.handle('dialog:open-directory', async () => {
    const result = await dialog.showOpenDialog({
      properties: ['openDirectory']
    });
    return result.canceled ? null : result.filePaths[0];
  });

  // Model management IPC handlers
  ipcMain.handle('models:list-available', () => ({
    whisperModels: WHISPER_MODELS,
    vadModel: VAD_MODEL
  }));

  ipcMain.handle('models:list-installed', () => getInstalledModels());

  ipcMain.handle('models:get-path', (_, modelId) => {
    const model = modelId === 'silero-vad'
      ? VAD_MODEL
      : WHISPER_MODELS.find((m) => m.id === modelId);
    if (!model) return null;
    const modelsDir = getModelsDirectory();
    const filePath = path.join(modelsDir, model.filename);
    return fs.existsSync(filePath) ? filePath : null;
  });

  ipcMain.handle('models:download', async (event, modelId) => {
    try {
      const result = await downloadModel(modelId, (progress) => {
        if (mainWindow && !mainWindow.isDestroyed()) {
          mainWindow.webContents.send('models:download-progress', progress);
        }
      });
      return { success: true, ...result };
    } catch (err) {
      return { success: false, error: err.message };
    }
  });

  ipcMain.handle('models:cancel-download', (_, modelId) => cancelDownload(modelId));

  ipcMain.handle('models:delete', (_, modelId) => {
    try {
      deleteModel(modelId);
      return { success: true };
    } catch (err) {
      return { success: false, error: err.message };
    }
  });

  ipcMain.handle('models:get-directory', () => getModelsDirectory());

  createWindow();

  if (app.isPackaged) {
    autoUpdater.checkForUpdatesAndNotify();
  }

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
});

app.on('window-all-closed', () => {
  if (runtimeProcess) {
    runtimeProcess.kill();
  }
  if (process.platform !== 'darwin') {
    app.quit();
  }
});
