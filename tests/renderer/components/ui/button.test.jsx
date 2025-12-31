import React from 'react';
import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import { Button } from '@/components/ui/button';

describe('Button', () => {
  it('renders a button with default styles', () => {
    const { getByRole } = render(<Button>Click</Button>);
    const button = getByRole('button');
    expect(button.className).toContain('rounded-full');
    expect(button.className).toContain('from-accent-500');
  });

  it('renders as a child element', () => {
    const { getByText } = render(
      <Button asChild>
        <a href="/test">Link</a>
      </Button>
    );
    const link = getByText('Link');
    expect(link.tagName).toBe('A');
    expect(link.className).toContain('rounded-full');
  });
});
