const path = require('path');
const fs = require('fs');
let https = require('https');
let http = require('http');
let { app, BrowserWindow, ipcMain, dialog } = require('electron');
let { spawn } = require('child_process');
const sharedRoot = fs.existsSync(path.join(__dirname, 'shared'))
  ? path.join(__dirname, 'shared')
  : path.join(__dirname, '..', 'shared');
const { RPC_METHODS } = require(path.join(sharedRoot, 'rpc'));
const { WHISPER_MODELS, VAD_MODEL } = require(path.join(sharedRoot, 'models'));

let mainWindow = null;
let runtimeProcess = null;
let rpcCounter = 1;
const pending = new Map();
const activeDownloads = new Map();
const shouldAutoStart = process.env.AER_DISABLE_AUTO_START !== '1';

function maybeSetAppUserModelId(platform = process.platform, appRef = app) {
  if (platform === 'win32') {
    appRef.setAppUserModelId('app.aer.subtly');
  }
}

function maybeInitSentry(env = process.env, sentry = null) {
  if (!env.SENTRY_DSN) {
    return;
  }
  const sentryClient = sentry || require('@sentry/electron/main');
  sentryClient.init({
    dsn: env.SENTRY_DSN,
    tracesSampleRate: 0.2
  });
}

function getRuntimeBinaryName(platform = process.platform) {
  if (platform === 'win32') {
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

  // Helper to check if file size is within acceptable range (within 5% or exact match)
  const isComplete = (actualSize, expectedSize) => {
    if (actualSize === expectedSize) return true;
    // Allow 5% tolerance for size variations
    const minSize = expectedSize * 0.95;
    const maxSize = expectedSize * 1.05;
    return actualSize >= minSize && actualSize <= maxSize;
  };

  for (const model of WHISPER_MODELS) {
    if (files.includes(model.filename)) {
      const filePath = path.join(modelsDir, model.filename);
      const stats = fs.statSync(filePath);
      installed.push({
        ...model,
        path: filePath,
        installedSize: stats.size,
        complete: isComplete(stats.size, model.sizeBytes)
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
      complete: isComplete(stats.size, VAD_MODEL.sizeBytes)
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
  if (!fs.existsSync(runtimePath)) {
    dialog.showErrorBox(
      'Runtime missing',
      `Runtime binary not found at ${runtimePath}. Build it with pnpm build:runtime.`
    );
    return;
  }
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
  if (!runtimeProcess || runtimeProcess.exitCode !== null) {
    if (shouldAutoStart) {
      startRuntime();
    }
  }
  if (!runtimeProcess || runtimeProcess.exitCode !== null) {
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

function initApp() {
  maybeSetAppUserModelId();
  maybeInitSentry();
  return app.whenReady().then(() => {
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

  const shouldCheckUpdates = app.isPackaged && process.env.NODE_ENV !== 'test' && process.env.VITEST !== 'true';
  if (shouldCheckUpdates) {
    try {
      const { autoUpdater } = require('electron-updater');
      autoUpdater.checkForUpdatesAndNotify();
    } catch (err) {
      console.warn('Auto update unavailable:', err.message);
    }
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
}

if (shouldAutoStart) {
  initApp();
}

function setMainWindow(windowRef) {
  mainWindow = windowRef;
}

function setRuntimeProcess(processRef) {
  runtimeProcess = processRef;
}

function setElectronModule(electronModule) {
  ({ app, BrowserWindow, ipcMain, dialog } = electronModule);
}

function setSpawnFunction(spawnFn) {
  spawn = spawnFn;
}

function setHttpClients(clients = {}) {
  if (clients.https) {
    https = clients.https;
  }
  if (clients.http) {
    http = clients.http;
  }
}

module.exports = {
  maybeSetAppUserModelId,
  maybeInitSentry,
  getRuntimeBinaryName,
  resolveRuntimePath,
  getModelsDirectory,
  ensureModelsDirectory,
  getInstalledModels,
  downloadModel,
  cancelDownload,
  deleteModel,
  startRuntime,
  sendRpc,
  createWindow,
  initApp,
  setMainWindow,
  setRuntimeProcess,
  setElectronModule,
  setSpawnFunction,
  setHttpClients,
  __test: {
    pending,
    activeDownloads
  }
};
