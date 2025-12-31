import React from 'react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useRuntimeStore } from '@/state/store';

const pingRuntime = vi.fn();
const listDevices = vi.fn();
const runSmokeTest = vi.fn();
const transcribe = vi.fn();

vi.mock('@/api/runtime', () => ({
  pingRuntime: () => pingRuntime(),
  listDevices: () => listDevices(),
  runSmokeTest: () => runSmokeTest(),
  transcribe: (payload) => transcribe(payload)
}));

vi.mock('@/components/ModelManager', () => ({
  default: () => <div>ModelManager</div>
}));

function renderApp(ui) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } }
  });
  return render(<QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>);
}

function openAdvancedSettings(getByText) {
  fireEvent.click(getByText('Advanced settings'));
}

describe('App', () => {
  const defaultState = useRuntimeStore.getState();
  beforeEach(() => {
    useRuntimeStore.setState(defaultState, true);
    pingRuntime.mockReset();
    listDevices.mockReset();
    runSmokeTest.mockReset();
    transcribe.mockReset();
    window.aerRuntime = { onEvent: vi.fn() };
    window.aerModels = {
      getModelPath: vi.fn(async (modelId) => (modelId === 'silero-vad' ? '/models/vad.bin' : '/models/base.bin'))
    };
    window.aerDialog = {
      openFile: vi.fn().mockResolvedValue('/tmp/input.mp4'),
      openDirectory: vi.fn().mockResolvedValue('/tmp/dir')
    };
  });

  it('selects the best device based on backend and type', async () => {
    const { selectBestDevice } = await import('@/App');
    expect(selectBestDevice([])).toBeNull();
    expect(
      selectBestDevice([
        { name: 'CPU', device_type: 'Cpu', backend: 'CPU' },
        { name: 'GPU', device_type: 'DiscreteGpu', backend: 'Vulkan' }
      ]).name
    ).toBe('GPU');
    expect(
      selectBestDevice([
        { name: 'GPU2', device_type: 'IntegratedGpu', backend: 'Metal' },
        { name: 'GPU1', device_type: 'IntegratedGpu', backend: 'wgpu' }
      ]).name
    ).toBe('GPU2');
    expect(
      selectBestDevice([{ name: 'Any', device_type: 'Other', backend: 'Vulkan' }]).name
    ).toBe('Any');
    expect(
      selectBestDevice([{ name: 'CPU', device_type: 'Cpu', backend: 'CPU' }]).name
    ).toBe('CPU');
  });

  it('logs runtime and device query errors', async () => {
    pingRuntime.mockRejectedValue(new Error('ping down'));
    listDevices.mockRejectedValue(new Error('device down'));
    const { default: App } = await import('@/App');
    const { unmount } = renderApp(<App />);

    await waitFor(() => {
      const logs = useRuntimeStore.getState().logs.map((entry) => entry.message);
      expect(logs.some((message) => message.includes('Runtime error'))).toBe(true);
      expect(logs.some((message) => message.includes('Device query failed'))).toBe(true);
    });
  });

  it('auto-selects device and handles runtime events', async () => {
    pingRuntime.mockResolvedValue({ message: 'ok', backend: 'Metal' });
    listDevices.mockResolvedValue({
      devices: [
        { name: 'CPU', device_type: 'Cpu', backend: 'CPU' },
        { name: 'GPU', device_type: 'DiscreteGpu', backend: 'Vulkan' }
      ]
    });
    const { default: App } = await import('@/App');
    const { unmount } = renderApp(<App />);

    await waitFor(() => {
      expect(useRuntimeStore.getState().selectedDevice?.name).toBe('GPU');
    });

    const handler = window.aerRuntime.onEvent.mock.calls[0][0];
    handler({ event: 'log', payload: 'Processing /tmp/file.mp4' });
    handler({ event: 'log', payload: 'ffmpeg started' });
    handler({ event: 'log', payload: 'whisper started' });
    handler({ event: 'log', payload: 'SKIP (up-to-date): /tmp/file.mp4' });
    handler({ event: 'progress', payload: { progress: 50, current: 1, total: 2, phase: 'Running' } });

    const { progressModal } = useRuntimeStore.getState();
    expect(progressModal.progress).toBe(50);
    expect(progressModal.statusMessage).toBe('Running');
    unmount();
  });

  it('runs smoke tests and handles transcription flow', async () => {
    pingRuntime.mockResolvedValue({ message: 'ok', backend: 'Metal' });
    listDevices.mockResolvedValue({ devices: [] });
    runSmokeTest.mockResolvedValue({ message: 'smoke ok' });
    transcribe.mockResolvedValue({ jobs: 1, outputs: ['/tmp/out.srt'] });

    const { default: App } = await import('@/App');
    const { getByText, getByPlaceholderText } = renderApp(<App />);

    useRuntimeStore.getState().setSelectedModel('base');
    await waitFor(() => {
      expect(window.aerModels.getModelPath).toHaveBeenCalled();
    });
    openAdvancedSettings(getByText);
    fireEvent.click(getByText('Run Vulkan smoke test'));
    await waitFor(() => {
      expect(useRuntimeStore.getState().logs.some((entry) => entry.message.includes('smoke ok'))).toBe(true);
    });

    useRuntimeStore.getState().setInputPath('/tmp/input.mp4');
    fireEvent.change(getByPlaceholderText('Defaults to input file/folder'), {
      target: { value: '/tmp/output' }
    });

    fireEvent.click(getByText('Pick file'));
    await waitFor(() => {
      expect(useRuntimeStore.getState().inputPath).toBe('/tmp/input.mp4');
    });
    fireEvent.click(getByText('Pick folder'));
    await waitFor(() => {
      expect(useRuntimeStore.getState().inputPath).toBe('/tmp/dir');
    });

    await waitFor(() => getByText('Generate subtitles'));
    fireEvent.click(getByText('Generate subtitles'));
    await waitFor(() => {
      expect(transcribe).toHaveBeenCalled();
      expect(useRuntimeStore.getState().logs.some((entry) => entry.message.includes('Completed'))).toBe(true);
    });
  });

  it('handles missing model paths and file pickers', async () => {
    pingRuntime.mockResolvedValue({ message: 'ok', backend: 'Metal' });
    listDevices.mockResolvedValue({ devices: [] });
    window.aerModels.getModelPath = vi.fn(async () => null);

    const { default: App } = await import('@/App');
    const { getByText } = renderApp(<App />);

    useRuntimeStore.getState().setInputPath('/tmp/input.mp4');
    await waitFor(() => getByText('Generate subtitles'));
    fireEvent.click(getByText('Generate subtitles'));

    await waitFor(() => {
      expect(useRuntimeStore.getState().logs.some((entry) => entry.message.includes('No Whisper model'))).toBe(true);
    });

    delete window.aerDialog;
    fireEvent.click(getByText('Pick file'));
    fireEvent.click(getByText('Pick folder'));
    await waitFor(() => {
      expect(useRuntimeStore.getState().logs.some((entry) => entry.message.includes('picker unavailable'))).toBe(true);
    });
  });

  it('logs smoke test failures', async () => {
    pingRuntime.mockResolvedValue({ message: 'ok', backend: 'Metal' });
    listDevices.mockResolvedValue({ devices: [] });
    runSmokeTest.mockRejectedValue(new Error('nope'));

    const { default: App } = await import('@/App');
    const { getByText } = renderApp(<App />);

    openAdvancedSettings(getByText);
    fireEvent.click(getByText('Run Vulkan smoke test'));
    await waitFor(() => {
      expect(useRuntimeStore.getState().logs.some((entry) => entry.message.includes('Smoke test failed'))).toBe(true);
    });
  });

  it('logs VAD and transcription failures and supports cancel', async () => {
    pingRuntime.mockResolvedValue({ message: 'ok', backend: 'Metal' });
    listDevices.mockResolvedValue({ devices: [] });
    window.aerModels.getModelPath = vi.fn(async (modelId) => (modelId === 'silero-vad' ? null : '/models/base.bin'));
    transcribe.mockRejectedValue(new Error('boom'));

    useRuntimeStore.getState().setSelectedModel('base');
    useRuntimeStore.getState().setInputPath('/tmp/input.mp4');

    const { default: App } = await import('@/App');
    const { getByText } = renderApp(<App />);

    await waitFor(() => {
      expect(window.aerModels.getModelPath).toHaveBeenCalled();
    });
    await waitFor(() => {
      expect(window.aerModels.getModelPath).toHaveBeenCalledTimes(2);
    });
    await new Promise((resolve) => setTimeout(resolve, 0));
    await waitFor(() => getByText('Generate subtitles'));
    fireEvent.click(getByText('Generate subtitles'));
    await waitFor(() => {
      expect(useRuntimeStore.getState().logs.some((entry) => entry.message.includes('VAD model'))).toBe(true);
    });

    window.aerModels.getModelPath = vi.fn(async () => '/models/base.bin');
    fireEvent.click(getByText('Generate subtitles'));
    await waitFor(() => {
      expect(useRuntimeStore.getState().logs.some((entry) => entry.message.includes('Transcription failed'))).toBe(true);
    });

    let rejectTranscribe;
    transcribe.mockReturnValue(new Promise((_resolve, reject) => { rejectTranscribe = reject; }));
    fireEvent.click(getByText('Generate subtitles'));
    await waitFor(() => getByText('Cancel'));
    fireEvent.click(getByText('Cancel'));
    expect(useRuntimeStore.getState().logs.some((entry) => entry.message.includes('cancelled'))).toBe(true);

    const logCount = useRuntimeStore.getState().logs.length;
    rejectTranscribe(new Error('late failure'));
    await waitFor(() => {
      expect(useRuntimeStore.getState().logs.length).toBe(logCount);
    });
  });
});
