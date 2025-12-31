import { afterEach, beforeEach, vi } from 'vitest';
import { cleanup } from '@testing-library/react';

let id = 0;

beforeEach(() => {
  id = 0;
  if (!globalThis.crypto) {
    globalThis.crypto = {};
  }
  if (globalThis.crypto.randomUUID) {
    vi.spyOn(globalThis.crypto, 'randomUUID').mockImplementation(() => `test-id-${++id}`);
  } else {
    globalThis.crypto.randomUUID = () => `test-id-${++id}`;
  }
  if (!globalThis.navigator) {
    globalThis.navigator = {};
  }
  if (!globalThis.navigator.hardwareConcurrency) {
    globalThis.navigator.hardwareConcurrency = 8;
  }
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllEnvs();
});
