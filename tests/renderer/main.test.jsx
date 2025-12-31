import { describe, it, expect, vi, beforeEach } from 'vitest';

const renderMock = vi.fn();
const createRootMock = vi.fn(() => ({ render: renderMock }));

vi.mock('react-dom/client', () => ({
  createRoot: createRootMock
}));

const sentryInit = vi.fn();
vi.mock('@sentry/electron/renderer', () => ({
  init: sentryInit
}));

describe('renderer entry', () => {
  beforeEach(() => {
    document.body.innerHTML = '<div id="root"></div>';
    renderMock.mockClear();
    createRootMock.mockClear();
    sentryInit.mockClear();
  });

  it('renders the root and initializes Sentry when configured', async () => {
    vi.resetModules();
    vi.stubEnv('VITE_SENTRY_DSN', 'dsn');
    await import('../../src/renderer/main.jsx');
    expect(createRootMock).toHaveBeenCalled();
    expect(renderMock).toHaveBeenCalled();
    expect(sentryInit).toHaveBeenCalled();
  });

  it('renders without Sentry when not configured', async () => {
    vi.resetModules();
    await import('../../src/renderer/main.jsx');
    expect(createRootMock).toHaveBeenCalled();
    expect(sentryInit).not.toHaveBeenCalled();
  });
});
