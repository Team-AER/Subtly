import React from 'react';
import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';

describe('Card components', () => {
  it('renders card wrappers', () => {
    const { getByText } = render(
      <Card>
        <CardHeader>
          <CardTitle>Title</CardTitle>
        </CardHeader>
        <CardContent>Body</CardContent>
      </Card>
    );
    expect(getByText('Title').className).toContain('font-display');
    expect(getByText('Body').className).toContain('mt-4');
  });
});
