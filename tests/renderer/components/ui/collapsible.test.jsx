import React from 'react';
import { describe, it, expect } from 'vitest';
import { render, fireEvent } from '@testing-library/react';
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from '@/components/ui/collapsible';

describe('Collapsible', () => {
  it('toggles open state with trigger', () => {
    const { queryByText, getByText } = render(
      <Collapsible>
        <CollapsibleTrigger>Toggle</CollapsibleTrigger>
        <CollapsibleContent>Hidden</CollapsibleContent>
        Plain text
      </Collapsible>
    );
    expect(queryByText('Hidden')).toBeNull();
    fireEvent.click(getByText('Toggle'));
    expect(getByText('Hidden')).toBeTruthy();
  });

  it('supports asChild trigger', () => {
    const { getByText } = render(
      <Collapsible defaultOpen>
        <CollapsibleTrigger asChild>
          <div>Trigger</div>
        </CollapsibleTrigger>
        <CollapsibleContent>Visible</CollapsibleContent>
      </Collapsible>
    );
    const trigger = getByText('Trigger');
    expect(trigger.getAttribute('data-state')).toBe('open');
  });
});
