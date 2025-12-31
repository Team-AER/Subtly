import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useRuntimeStore } from '@/state/store';

const defaultState = useRuntimeStore.getState();

describe('runtime store', () => {
  beforeEach(() => {
    useRuntimeStore.setState(defaultState, true);
  });

  it('initializes with defaults', () => {
    const state = useRuntimeStore.getState();
    expect(state.logs).toEqual([]);
    expect(state.selectedDevice).toBe(null);
    expect(state.selectedModel).toBe(null);
    expect(state.inputPath).toBe('');
    expect(state.outputDir).toBe('');
    expect(state.settings.language).toBe('auto');
    expect(state.progressModal.isOpen).toBe(false);
  });

  it('updates selection and paths', () => {
    const device = { name: 'GPU-1' };
    useRuntimeStore.getState().setSelectedDevice(device);
    useRuntimeStore.getState().setSelectedModel('base');
    useRuntimeStore.getState().setInputPath('/tmp/input.mp4');
    useRuntimeStore.getState().setOutputDir('/tmp/out');
    const state = useRuntimeStore.getState();
    expect(state.selectedDevice).toEqual(device);
    expect(state.selectedModel).toBe('base');
    expect(state.inputPath).toBe('/tmp/input.mp4');
    expect(state.outputDir).toBe('/tmp/out');
  });

  it('updates settings patch', () => {
    useRuntimeStore.getState().updateSettings({ beamSize: 12, translate: false });
    const state = useRuntimeStore.getState();
    expect(state.settings.beamSize).toBe(12);
    expect(state.settings.translate).toBe(false);
  });

  it('adds logs with ids', () => {
    useRuntimeStore.getState().addLog('hello');
    const { logs } = useRuntimeStore.getState();
    expect(logs).toHaveLength(1);
    expect(logs[0].message).toBe('hello');
    expect(logs[0].id).toMatch(/^test-id-/);
  });

  it('controls the progress modal', () => {
    useRuntimeStore.getState().showProgressModal({
      type: 'transcription',
      title: 'Working',
      progress: 25,
      statusMessage: 'Running'
    });
    let state = useRuntimeStore.getState();
    expect(state.progressModal.isOpen).toBe(true);
    expect(state.progressModal.title).toBe('Working');
    expect(state.progressModal.progress).toBe(25);

    useRuntimeStore.getState().updateProgressModal({
      progress: 70,
      statusMessage: 'Almost done'
    });
    state = useRuntimeStore.getState();
    expect(state.progressModal.progress).toBe(70);
    expect(state.progressModal.statusMessage).toBe('Almost done');

    useRuntimeStore.getState().hideProgressModal();
    state = useRuntimeStore.getState();
    expect(state.progressModal.isOpen).toBe(false);
    expect(state.progressModal.progress).toBe(0);
    expect(state.progressModal.statusMessage).toBe('');
  });

  it('falls back to default threads when hardwareConcurrency is missing', async () => {
    const original = globalThis.navigator?.hardwareConcurrency;
    globalThis.navigator = globalThis.navigator || {};
    Object.defineProperty(globalThis.navigator, 'hardwareConcurrency', {
      configurable: true,
      value: 0
    });
    vi.resetModules();
    const storeModule = await import('@/state/store');
    const { useRuntimeStore: freshStore } = storeModule;
    expect(freshStore.getState().settings.threads).toBe(8);
    Object.defineProperty(globalThis.navigator, 'hardwareConcurrency', {
      configurable: true,
      value: original
    });
  });
});
