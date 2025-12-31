const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('aerRuntime', {
  ping: () => ipcRenderer.invoke('runtime:ping'),
  listDevices: () => ipcRenderer.invoke('runtime:list-devices'),
  smokeTest: () => ipcRenderer.invoke('runtime:smoke-test'),
  transcribe: (payload) => ipcRenderer.invoke('runtime:transcribe', payload),
  onEvent: (handler) => {
    ipcRenderer.removeAllListeners('runtime:event');
    ipcRenderer.on('runtime:event', (_event, data) => handler(data));
  }
});

contextBridge.exposeInMainWorld('aerDialog', {
  openFile: () => ipcRenderer.invoke('dialog:open-file'),
  openDirectory: () => ipcRenderer.invoke('dialog:open-directory')
});

contextBridge.exposeInMainWorld('aerModels', {
  listAvailable: () => ipcRenderer.invoke('models:list-available'),
  listInstalled: () => ipcRenderer.invoke('models:list-installed'),
  getModelPath: (modelId) => ipcRenderer.invoke('models:get-path', modelId),
  download: (modelId) => ipcRenderer.invoke('models:download', modelId),
  cancelDownload: (modelId) => ipcRenderer.invoke('models:cancel-download', modelId),
  deleteModel: (modelId) => ipcRenderer.invoke('models:delete', modelId),
  getModelsDirectory: () => ipcRenderer.invoke('models:get-directory'),
  onDownloadProgress: (handler) => {
    ipcRenderer.removeAllListeners('models:download-progress');
    ipcRenderer.on('models:download-progress', (_event, data) => handler(data));
  }
});
