const path = require('path');
const { app, BrowserWindow, ipcMain, dialog } = require('electron');
const { spawn } = require('child_process');
const { RPC_METHODS } = require('../shared/rpc');
const { autoUpdater } = require('electron-updater');
const Sentry = require('@sentry/electron/main');

let mainWindow = null;
let runtimeProcess = null;
let rpcCounter = 1;
const pending = new Map();

if (process.platform === 'win32') {
  app.setAppUserModelId('com.aer.vulkanml');
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

  return path.join(__dirname, '..', '..', '..', 'runtime', 'gpu-runtime', 'target', 'debug', binaryName);
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
