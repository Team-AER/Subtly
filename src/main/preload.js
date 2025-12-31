const { contextBridge, ipcRenderer } = require('electron');

function registerPreloadApis(bridge = contextBridge, ipc = ipcRenderer) {
  bridge.exposeInMainWorld('aerRuntime', {
    ping: () => ipc.invoke('runtime:ping'),
    listDevices: () => ipc.invoke('runtime:list-devices'),
    smokeTest: () => ipc.invoke('runtime:smoke-test'),
    transcribe: (payload) => ipc.invoke('runtime:transcribe', payload),
    onEvent: (handler) => {
      ipc.removeAllListeners('runtime:event');
      ipc.on('runtime:event', (_event, data) => handler(data));
    }
  });

  bridge.exposeInMainWorld('aerDialog', {
    openFile: () => ipc.invoke('dialog:open-file'),
    openDirectory: () => ipc.invoke('dialog:open-directory')
  });

  bridge.exposeInMainWorld('aerModels', {
    listAvailable: () => ipc.invoke('models:list-available'),
    listInstalled: () => ipc.invoke('models:list-installed'),
    getModelPath: (modelId) => ipc.invoke('models:get-path', modelId),
    download: (modelId) => ipc.invoke('models:download', modelId),
    cancelDownload: (modelId) => ipc.invoke('models:cancel-download', modelId),
    deleteModel: (modelId) => ipc.invoke('models:delete', modelId),
    getModelsDirectory: () => ipc.invoke('models:get-directory'),
    onDownloadProgress: (handler) => {
      ipc.removeAllListeners('models:download-progress');
      ipc.on('models:download-progress', (_event, data) => handler(data));
    }
  });
}

if (process.env.AER_DISABLE_AUTO_START !== '1') {
  registerPreloadApis();
}

module.exports = {
  registerPreloadApis
};
