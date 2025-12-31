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
