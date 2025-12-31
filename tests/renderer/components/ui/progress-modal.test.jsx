import React from 'react';
import { describe, it, expect, vi } from 'vitest';
import { render, fireEvent } from '@testing-library/react';
import { ProgressModal, formatBytes, formatTime } from '@/components/ui/progress-modal';

describe('ProgressModal', () => {
  it('returns null when closed', () => {
    const { container } = render(<ProgressModal isOpen={false} />);
    expect(container.firstChild).toBeNull();
  });

  it('formats time and bytes for edge cases', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(1024)).toBe('1 KB');
    expect(formatTime(0)).toBe('--:--');
    expect(formatTime(NaN)).toBe('--:--');
    expect(formatTime(Infinity)).toBe('--:--');
    expect(formatTime(65)).toBe('1:05');
    expect(formatTime(3600)).toBe('1h 0m');
  });

  it('renders byte-based progress and handles cancel', () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(0));
    const onCancel = vi.fn();
    const { rerender, getByText } = render(
      <ProgressModal
        isOpen
        title="Download"
        progress={0}
        currentBytes={0}
        totalBytes={100}
        onCancel={onCancel}
        canCancel
      />
    );
    vi.setSystemTime(new Date(1000));
    rerender(
      <ProgressModal
        isOpen
        title="Download"
        progress={50}
        currentBytes={50}
        totalBytes={100}
        onCancel={onCancel}
        canCancel
      />
    );
    fireEvent.click(getByText('Cancel'));
    expect(onCancel).toHaveBeenCalled();
    vi.useRealTimers();
  });

  it('renders percentage-based progress with status', () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(0));
    const { rerender, getByText } = render(
      <ProgressModal
        isOpen
        title="Transcribing"
        progress={0}
        statusMessage="Starting"
      />
    );
    vi.setSystemTime(new Date(1000));
    rerender(
      <ProgressModal
        isOpen
        title="Transcribing"
        progress={25}
        statusMessage="Running"
        currentItem={1}
        totalItems={4}
      />
    );
    expect(getByText('Running')).toBeTruthy();
    vi.useRealTimers();
  });
});
