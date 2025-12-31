import { describe, it, expect } from 'vitest';
import { cn } from '@/lib/utils';

describe('cn', () => {
  it('merges class names with tailwind-merge', () => {
    expect(cn('px-2', 'px-4', { hidden: false, block: true })).toBe('px-4 block');
  });
});
