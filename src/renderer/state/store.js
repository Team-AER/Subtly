import { create } from 'zustand';

export const useRuntimeStore = create((set) => ({
  logs: [],
  selectedDevice: null,
  selectedModel: null,
  inputPath: '',
  outputDir: '',
  settings: {
    modelPath: '',
    vadModelPath: '',
    whisperPath: '',
    ffmpegPath: '',
    vkIcdFilenames: '',
    threads: navigator.hardwareConcurrency || 8,
    beamSize: 8,
    bestOf: 8,
    maxLenChars: 60,
    splitOnWord: true,
    vadThreshold: 0.35,
    vadMinSpeechMs: 200,
    vadMinSilMs: 250,
    vadPadMs: 80,
    noSpeechThold: 0.75,
    maxContext: 0,
    dedupMergeGapSec: 0.6,
    translate: true,
    language: 'auto',
    dryRun: false
  },
  setSelectedDevice: (device) => set({ selectedDevice: device }),
  setSelectedModel: (modelId) => set({ selectedModel: modelId }),
  setInputPath: (inputPath) => set({ inputPath }),
  setOutputDir: (outputDir) => set({ outputDir }),
  updateSettings: (patch) =>
    set((state) => ({ settings: { ...state.settings, ...patch } })),
  addLog: (entry) =>
    set((state) => ({
      logs: [...state.logs, { id: crypto.randomUUID(), message: entry }]
    }))
}));
