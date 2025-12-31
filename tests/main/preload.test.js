import { describe, it, expect, vi } from 'vitest';

vi.mock('electron', () => ({
  contextBridge: { exposeInMainWorld: vi.fn() },
  ipcRenderer: { invoke: vi.fn(), on: vi.fn(), removeAllListeners: vi.fn() }
}));

process.env.AER_DISABLE_AUTO_START = '1';
const preloadModule = await import('../../src/main/preload');
const { registerPreloadApis } = preloadModule.default || preloadModule;

describe('preload', () => {
  it('exposes runtime, dialog, and model APIs', () => {
    const exposed = {};
    const bridge = {
      exposeInMainWorld: (key, value) => {
        exposed[key] = value;
      }
    };
    const ipc = {
      invoke: vi.fn(),
      on: vi.fn(),
      removeAllListeners: vi.fn()
    };

    registerPreloadApis(bridge, ipc);

    expect(Object.keys(exposed)).toEqual(['aerRuntime', 'aerDialog', 'aerModels']);
    exposed.aerRuntime.ping();
    exposed.aerRuntime.listDevices();
    exposed.aerRuntime.smokeTest();
    exposed.aerRuntime.transcribe({ input_path: 'x' });
    exposed.aerRuntime.onEvent(() => {});
    exposed.aerDialog.openFile();
    exposed.aerDialog.openDirectory();
    exposed.aerModels.listAvailable();
    exposed.aerModels.listInstalled();
    exposed.aerModels.getModelPath('base');
    exposed.aerModels.download('base');
    exposed.aerModels.cancelDownload('base');
    exposed.aerModels.deleteModel('base');
    exposed.aerModels.getModelsDirectory();
    exposed.aerModels.onDownloadProgress(() => {});

    expect(ipc.invoke).toHaveBeenCalled();
    expect(ipc.on).toHaveBeenCalled();
    expect(ipc.removeAllListeners).toHaveBeenCalled();
  });
});
