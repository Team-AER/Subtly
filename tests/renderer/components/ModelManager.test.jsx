import React from 'react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import ModelManager from '@/components/ModelManager';
import { useRuntimeStore } from '@/state/store';

function renderWithClient(ui) {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false }
    }
  });
  return render(<QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>);
}

const whisperModel = {
  id: 'base',
  name: 'Base',
  description: 'Base model',
  size: '10 MB',
  sizeBytes: 10,
  url: 'https://example.com/base',
  filename: 'base.bin',
  recommended: true
};

const vadModel = {
  id: 'silero-vad',
  name: 'VAD',
  description: 'vad model',
  size: '2 MB',
  sizeBytes: 2,
  url: 'https://example.com/vad',
  filename: 'vad.bin',
  required: true
};

describe('ModelManager', () => {
  const defaultState = useRuntimeStore.getState();
  beforeEach(() => {
    useRuntimeStore.setState(defaultState, true);
    window.aerModels = {
      listAvailable: vi.fn().mockResolvedValue({ whisperModels: [whisperModel], vadModel }),
      listInstalled: vi.fn().mockResolvedValue([]),
      download: vi.fn().mockResolvedValue({ success: true }),
      deleteModel: vi.fn().mockResolvedValue({ success: true }),
      cancelDownload: vi.fn(),
      onDownloadProgress: vi.fn()
    };
  });

  it('auto-selects installed models and renders status', async () => {
    window.aerModels.listInstalled.mockResolvedValue([
      { id: whisperModel.id, complete: true },
      { id: vadModel.id, complete: true }
    ]);

    const { getByText } = renderWithClient(<ModelManager />);
    await waitFor(() => getByText('Whisper Models'));
    await waitFor(() => {
      expect(useRuntimeStore.getState().selectedModel).toBe(whisperModel.id);
    });
    expect(getByText('Installed')).toBeTruthy();
    expect(getByText('Selected')).toBeTruthy();

    const card = getByText('Base').closest('[role="button"]');
    fireEvent.click(card);
    fireEvent.keyDown(card, { key: 'Enter' });
    fireEvent.keyDown(card, { key: ' ' });
  });

  it('renders safely without model bridge', async () => {
    delete window.aerModels;
    const { getByText } = renderWithClient(<ModelManager />);
    await waitFor(() => getByText('Whisper Models'));
    expect(getByText('Whisper Models')).toBeTruthy();
  });

  it('handles download and cancel actions', async () => {
    let resolveDownload;
    window.aerModels.download = vi.fn().mockReturnValue(new Promise((resolve) => { resolveDownload = resolve; }));

    const { getByText, queryAllByText } = renderWithClient(<ModelManager />);
    await waitFor(() => getByText('Base'));
    const baseCard = getByText('Base').closest('div[class*="rounded-xl"]');
    const downloadButton = baseCard.querySelector('button');
    fireEvent.click(downloadButton);
    expect(window.aerModels.download).toHaveBeenCalledWith(whisperModel.id);

    await waitFor(() => {
      expect(window.aerModels.onDownloadProgress).toHaveBeenCalled();
    });
    window.aerModels.onDownloadProgress.mock.calls[0][0]({
      modelId: whisperModel.id,
      progress: 30,
      downloadedBytes: 3,
      totalBytes: 10
    });

    await waitFor(() => getByText('Downloading Model'));

    const cancelButtons = queryAllByText('Cancel');
    if (cancelButtons.length > 0) {
      fireEvent.click(cancelButtons[0]);
    }
    expect(window.aerModels.cancelDownload).toHaveBeenCalled();

    resolveDownload({ success: true });
  });

  it('deletes models and logs failures', async () => {
    window.aerModels.listInstalled.mockResolvedValue([{ id: whisperModel.id, complete: true }]);
    window.aerModels.deleteModel.mockResolvedValue({ success: false, error: 'nope' });

    const { getByText } = renderWithClient(<ModelManager />);
    await waitFor(() => getByText('Delete'));
    fireEvent.click(getByText('Delete'));
    await waitFor(() => {
      expect(useRuntimeStore.getState().logs.slice(-1)[0].message).toContain('Delete failed');
    });
  });
});
