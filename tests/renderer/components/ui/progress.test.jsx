import React from 'react';
import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import { Progress } from '@/components/ui/progress';

describe('Progress', () => {
  it('clamps percentage and renders stripes', () => {
    const { container } = render(<Progress value={120} max={100} showStripes size="lg" />);
    const outer = container.firstChild;
    const bar = outer.children[1];
    expect(bar.style.width).toBe('100%');
    const stripe = outer.children[0];
    expect(stripe).toBeTruthy();
  });

  it('handles mid and zero progress states', () => {
    const { container, rerender } = render(<Progress value={50} max={100} showStripes size="unknown" />);
    const outer = container.firstChild;
    const pulse = outer.children[2];
    expect(pulse).toBeTruthy();

    rerender(<Progress value={0} max={100} />);
    const bar = container.firstChild.children[0];
    expect(bar.style.width).toBe('0%');
  });
});
