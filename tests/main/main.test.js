// @vitest-environment node
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import fs from 'fs';
import path from 'path';
import os from 'os';
import { EventEmitter } from 'events';
import { PassThrough } from 'stream';

const browserWindows = [];
const ipcHandlers = new Map();
const appEvents = new Map();

const spawnMock = vi.fn();
const httpsGet = vi.fn();
const httpGet = vi.fn();

const electronMock = {
  app: {
    isPackaged: false,
    whenReady: vi.fn(() => Promise.resolve()),
    on: vi.fn((event, handler) => {
      appEvents.set(event, handler);
    }),
    getAppPath: vi.fn(() => '/app'),
    getPath: vi.fn(() => '/user'),
    getVersion: vi.fn(() => '0.0.0'),
    quit: vi.fn(),
    setAppUserModelId: vi.fn()
  },
  BrowserWindow: vi.fn(),
  ipcMain: {
    handle: vi.fn((channel, handler) => {
      ipcHandlers.set(channel, handler);
    })
  },
  dialog: {
    showOpenDialog: vi.fn(),
    showErrorBox: vi.fn()
  }
};

electronMock.BrowserWindow.getAllWindows = vi.fn(() => browserWindows);

vi.mock('@sentry/electron/main', () => ({
  init: vi.fn()
}));


function createMockProcess() {
  const proc = new EventEmitter();
  proc.stdout = new EventEmitter();
  proc.stderr = new EventEmitter();
  proc.stdout.setEncoding = vi.fn();
  proc.stderr.setEncoding = vi.fn();
  proc.stdin = { write: vi.fn() };
  proc.kill = vi.fn();
  return proc;
}

function createMockWindow(opts) {
  const win = {
    options: opts,
    loadURL: vi.fn(),
    loadFile: vi.fn(),
    isDestroyed: vi.fn(() => false),
    webContents: { send: vi.fn() }
  };
  browserWindows.push(win);
  return win;
}

let cachedMain = null;

async function loadMain() {
  if (!cachedMain) {
    process.env.AER_DISABLE_AUTO_START = '1';
    const mod = await import('../../src/main/main');
    cachedMain = mod.default || mod;
  }
  return cachedMain;
}

describe('main process', () => {
  beforeEach(async () => {
    ipcHandlers.clear();
    appEvents.clear();
    browserWindows.length = 0;
    process.env.NODE_ENV = 'test';
    httpsGet.mockReset();
    httpGet.mockReset();
    spawnMock.mockReset();

    const main = await loadMain();
    main.__test.pending.clear();
    main.__test.activeDownloads.clear();
    main.setMainWindow(null);
    main.setRuntimeProcess(null);
    main.setElectronModule(electronMock);
    main.setSpawnFunction((...args) => spawnMock(...args));
    main.setHttpClients({
      https: { get: (...args) => httpsGet(...args) },
      http: { get: (...args) => httpGet(...args) }
    });
    electronMock.app.isPackaged = false;
    electronMock.app.whenReady.mockReset().mockResolvedValue();
    electronMock.app.on.mockReset().mockImplementation((event, handler) => {
      appEvents.set(event, handler);
    });
    electronMock.app.getAppPath.mockReset().mockReturnValue('/app');
    electronMock.app.getPath.mockReset().mockReturnValue('/user');
    electronMock.app.getVersion.mockReset().mockReturnValue('0.0.0');
    electronMock.app.quit.mockReset();
    electronMock.app.setAppUserModelId.mockReset();
    electronMock.BrowserWindow.mockClear();
    electronMock.BrowserWindow.mockImplementation(createMockWindow);
    electronMock.BrowserWindow.getAllWindows.mockReset().mockImplementation(() => browserWindows);
    electronMock.ipcMain.handle.mockReset().mockImplementation((channel, handler) => {
      ipcHandlers.set(channel, handler);
    });
    electronMock.dialog.showOpenDialog.mockReset();
    electronMock.dialog.showErrorBox.mockReset();
  });

  afterEach(() => {
    delete process.env.AER_RUNTIME_PATH;
    delete process.env.SENTRY_DSN;
    delete process.env.VITE_DEV_SERVER_URL;
    delete process.env.AER_ASSET_DIR;
    delete process.env.NODE_ENV;
  });

  it('handles platform-specific helpers', async () => {
    const main = await loadMain();
    expect(main.getRuntimeBinaryName('win32')).toBe('gpu-runtime.exe');
    expect(main.getRuntimeBinaryName('darwin')).toBe('gpu-runtime');

    const { app } = electronMock;
    main.maybeSetAppUserModelId('win32', app);
    expect(app.setAppUserModelId).toHaveBeenCalledWith('com.aer.subtitleforge');
    main.maybeSetAppUserModelId('darwin', app);
    expect(app.setAppUserModelId).toHaveBeenCalledTimes(1);

    const sentry = await import('@sentry/electron/main');
    main.maybeInitSentry({ SENTRY_DSN: 'dsn' }, sentry);
    expect(sentry.init).toHaveBeenCalled();
  });

  it('resolves runtime paths for dev and packaged modes', async () => {
    const main = await loadMain();
    const { app } = electronMock;
    app.getAppPath.mockReturnValue('/workspace');
    expect(main.resolveRuntimePath()).toBe(
      path.join('/workspace', 'runtime', 'gpu-runtime', 'target', 'debug', 'gpu-runtime')
    );

    process.env.AER_RUNTIME_PATH = '/custom/runtime';
    expect(main.resolveRuntimePath()).toBe('/custom/runtime');
    delete process.env.AER_RUNTIME_PATH;

    Object.defineProperty(process, 'resourcesPath', { value: '/resources', configurable: true });
    app.isPackaged = true;
    expect(main.resolveRuntimePath()).toBe(path.join('/resources', 'runtime', 'gpu-runtime'));
    app.getPath.mockReturnValue('/userdata');
    expect(main.getModelsDirectory()).toBe(path.join('/userdata', 'models'));
  });

  it('manages model directories and installed models', async () => {
    const main = await loadMain();
    const { app } = electronMock;
    const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'aer-main-'));
    app.isPackaged = false;
    app.getAppPath.mockReturnValue(tempRoot);

    const modelsDir = main.ensureModelsDirectory();
    expect(fs.existsSync(modelsDir)).toBe(true);

    const models = await import('../../src/shared/models');
    const { WHISPER_MODELS, VAD_MODEL } = models.default || models;
    const whisperModel = WHISPER_MODELS[0];
    const whisperPath = path.join(modelsDir, whisperModel.filename);
    fs.writeFileSync(whisperPath, '');
    fs.truncateSync(whisperPath, whisperModel.sizeBytes);
    const vadPath = path.join(modelsDir, VAD_MODEL.filename);
    fs.writeFileSync(vadPath, '');
    fs.truncateSync(vadPath, VAD_MODEL.sizeBytes);

    const installed = main.getInstalledModels();
    expect(installed.find((model) => model.id === whisperModel.id).complete).toBe(true);
    expect(installed.find((model) => model.id === VAD_MODEL.id).complete).toBe(true);
  });

  it('returns empty installed models when directory is missing', async () => {
    const main = await loadMain();
    const { app } = electronMock;
    const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'aer-empty-'));
    app.isPackaged = false;
    app.getAppPath.mockReturnValue(tempRoot);
    const modelsDir = path.join(tempRoot, 'runtime', 'assets', 'models');
    if (fs.existsSync(modelsDir)) {
      fs.rmSync(modelsDir, { recursive: true, force: true });
    }
    expect(main.getInstalledModels()).toEqual([]);
  });

  it('downloads models and supports cancel', async () => {
    const main = await loadMain();
    const { app } = electronMock;
    const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'aer-download-'));
    app.isPackaged = false;
    app.getAppPath.mockReturnValue(tempRoot);

    httpsGet.mockImplementation((url, options, cb) => {
      if (typeof options === 'function') {
        cb = options;
      }
      const res = new PassThrough();
      res.statusCode = 200;
      res.headers = { 'content-length': '4' };
      res.destroy = vi.fn();
      cb(res);
      res.write('data');
      res.end();
      return { on: vi.fn() };
    });

    const progressUpdates = [];
    const result = await main.downloadModel('base', (progress) => {
      progressUpdates.push(progress);
    });
    expect(result.modelId).toBe('base');
    expect(progressUpdates.length).toBeGreaterThan(0);

    main.__test.activeDownloads.set('base', { abort: vi.fn() });
    expect(main.cancelDownload('base')).toBe(true);
    expect(main.cancelDownload('missing')).toBe(false);
  });

  it('follows redirects when downloading models', async () => {
    const main = await loadMain();
    const { app } = electronMock;
    const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'aer-redirect-'));
    app.isPackaged = false;
    app.getAppPath.mockReturnValue(tempRoot);

    let callCount = 0;
    httpsGet.mockImplementation((url, options, cb) => {
      if (typeof options === 'function') {
        cb = options;
      }
      callCount += 1;
      const res = new PassThrough();
      if (callCount === 1) {
        res.statusCode = 302;
        res.headers = { location: 'https://redirected' };
        cb(res);
        res.end();
        return { on: vi.fn() };
      }
      res.statusCode = 200;
      res.headers = { 'content-length': '4' };
      res.destroy = vi.fn();
      cb(res);
      res.write('data');
      res.end();
      return { on: vi.fn() };
    });

    const result = await main.downloadModel('base', () => {});
    expect(result.modelId).toBe('base');
    expect(callCount).toBe(2);
  });

  it('handles download failures and deletes models', async () => {
    const main = await loadMain();
    const { app } = electronMock;
    const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'aer-download-fail-'));
    app.isPackaged = false;
    app.getAppPath.mockReturnValue(tempRoot);

    httpsGet.mockImplementation((url, cb) => {
      const res = new PassThrough();
      res.statusCode = 404;
      res.headers = {};
      cb(res);
      res.end();
      return { on: vi.fn() };
    });

    await expect(main.downloadModel('base', () => {})).rejects.toThrow('Download failed');

    const modelsDir = main.ensureModelsDirectory();
    const models = await import('../../src/shared/models');
    const { WHISPER_MODELS } = models.default || models;
    const model = WHISPER_MODELS[0];
    const filePath = path.join(modelsDir, model.filename);
    fs.writeFileSync(filePath, 'x');
    expect(main.deleteModel(model.id)).toBe(true);
    expect(main.deleteModel(model.id)).toBe(false);
    expect(() => main.deleteModel('unknown-model')).toThrow('Unknown model');
  });

  it('starts runtime, sends RPC, and handles events', async () => {
    const main = await loadMain();
    const proc = createMockProcess();
    spawnMock.mockReturnValue(proc);

    const { BrowserWindow, dialog } = electronMock;
    const win = new BrowserWindow({});
    main.setMainWindow(win);
    main.startRuntime();

    const pendingPromise = main.sendRpc('ping');
    expect(proc.stdin.write).toHaveBeenCalled();

    const payload = JSON.stringify({ id: 1, result: { ok: true } });
    proc.stdout.emit('data', `${payload}\n`);
    await expect(pendingPromise).resolves.toEqual({ ok: true });

    const errorPromise = main.sendRpc('ping');
    const [errorId] = Array.from(main.__test.pending.keys());
    proc.stdout.emit('data', `${JSON.stringify({ id: errorId, error: { message: 'nope' } })}\n`);
    await expect(errorPromise).rejects.toThrow('nope');

    const eventMessage = { event: 'log', payload: 'hello' };
    proc.stdout.emit('data', `${JSON.stringify(eventMessage)}\n`);
    expect(win.webContents.send).toHaveBeenCalledWith('runtime:event', eventMessage);

    proc.stdout.emit('data', '{invalid json');
    proc.stderr.emit('data', 'oops');
    const error = new Error('boom');
    proc.emit('error', error);
    expect(dialog.showErrorBox).toHaveBeenCalledWith('Runtime error', error.message);

    proc.emit('exit', 1);
    expect(() => main.sendRpc('ping')).toThrow('Runtime process is not running');
  });

  it('creates windows and registers IPC handlers', async () => {
    const main = await loadMain();
    const { BrowserWindow, ipcMain, dialog } = electronMock;
    const proc = createMockProcess();
    spawnMock.mockReturnValue(proc);

    process.env.VITE_DEV_SERVER_URL = 'http://localhost:3000';
    main.createWindow();
    expect(BrowserWindow).toHaveBeenCalled();
    expect(browserWindows[0].loadURL).toHaveBeenCalledWith('http://localhost:3000');

    delete process.env.VITE_DEV_SERVER_URL;
    main.createWindow();
    expect(browserWindows[1].loadFile).toHaveBeenCalled();

    const { app } = electronMock;
    app.isPackaged = true;
    dialog.showOpenDialog.mockResolvedValue({ canceled: false, filePaths: ['/tmp/a'] });

    await main.initApp();
    expect(ipcMain.handle).toHaveBeenCalled();

    const openFile = ipcHandlers.get('dialog:open-file');
    const filePath = await openFile();
    expect(filePath).toBe('/tmp/a');
    dialog.showOpenDialog.mockResolvedValue({ canceled: true, filePaths: [] });
    const openDir = ipcHandlers.get('dialog:open-directory');
    expect(await openDir()).toBeNull();

    app.isPackaged = false;
    const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'aer-ipc-'));
    app.getAppPath.mockReturnValue(tempRoot);

    const listAvailable = ipcHandlers.get('models:list-available');
    const available = await listAvailable();
    expect(available.whisperModels.length).toBeGreaterThan(0);

    const listInstalled = ipcHandlers.get('models:list-installed');
    expect(Array.isArray(await listInstalled())).toBe(true);

    const modelsDir = main.ensureModelsDirectory();
    const models = await import('../../src/shared/models');
    const { WHISPER_MODELS, VAD_MODEL } = models.default || models;
    const model = WHISPER_MODELS[0];
    const modelPath = path.join(modelsDir, model.filename);
    fs.writeFileSync(modelPath, 'x');

    const getPath = ipcHandlers.get('models:get-path');
    expect(await getPath(null, model.id)).toBe(modelPath);
    expect(await getPath(null, 'missing')).toBeNull();

    httpsGet.mockImplementation((url, options, cb) => {
      if (typeof options === 'function') {
        cb = options;
      }
      const res = new PassThrough();
      res.statusCode = 200;
      res.headers = { 'content-length': '4' };
      res.destroy = vi.fn();
      cb(res);
      res.write('data');
      res.end();
      return { on: vi.fn() };
    });

    const win = browserWindows[1];
    main.setMainWindow(win);
    const downloadHandler = ipcHandlers.get('models:download');
    const downloadResult = await downloadHandler({}, model.id);
    expect(downloadResult.success).toBe(true);
    expect(win.webContents.send).toHaveBeenCalled();

    const cancelHandler = ipcHandlers.get('models:cancel-download');
    main.__test.activeDownloads.set(model.id, { abort: vi.fn() });
    expect(cancelHandler({}, model.id)).toBe(true);

    const deleteHandler = ipcHandlers.get('models:delete');
    const deleteResult = await deleteHandler({}, model.id);
    expect(deleteResult.success).toBe(true);
    const deleteFail = await deleteHandler({}, 'unknown-model');
    expect(deleteFail.success).toBe(false);

    const getDir = ipcHandlers.get('models:get-directory');
    expect(await getDir()).toBe(modelsDir);

    const runtimePing = ipcHandlers.get('runtime:ping');
    const pingPromise = runtimePing();
    const [pingId] = Array.from(main.__test.pending.keys());
    proc.stdout.emit('data', `${JSON.stringify({ id: pingId, result: { ok: true } })}\n`);
    await expect(pingPromise).resolves.toEqual({ ok: true });

    const runtimeList = ipcHandlers.get('runtime:list-devices');
    const listPromise = runtimeList();
    const [listId] = Array.from(main.__test.pending.keys());
    proc.stdout.emit('data', `${JSON.stringify({ id: listId, result: { devices: [] } })}\n`);
    await expect(listPromise).resolves.toEqual({ devices: [] });

    const runtimeSmoke = ipcHandlers.get('runtime:smoke-test');
    const smokePromise = runtimeSmoke();
    const [smokeId] = Array.from(main.__test.pending.keys());
    proc.stdout.emit('data', `${JSON.stringify({ id: smokeId, result: { message: 'ok' } })}\n`);
    await expect(smokePromise).resolves.toEqual({ message: 'ok' });

    const runtimeTranscribe = ipcHandlers.get('runtime:transcribe');
    const transcribePromise = runtimeTranscribe({}, { input_path: 'x' });
    const [transcribeId] = Array.from(main.__test.pending.keys());
    proc.stdout.emit('data', `${JSON.stringify({ id: transcribeId, result: { jobs: 0, outputs: [] } })}\n`);
    await expect(transcribePromise).resolves.toEqual({ jobs: 0, outputs: [] });
  });
});
