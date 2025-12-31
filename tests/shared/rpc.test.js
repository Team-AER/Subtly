import { describe, it, expect } from 'vitest';
import rpc from '../../src/shared/rpc';

const { RPC_METHODS } = rpc;

describe('shared rpc', () => {
  it('exposes expected RPC method names', () => {
    expect(RPC_METHODS).toEqual({
      PING: 'ping',
      LIST_DEVICES: 'list_devices',
      SMOKE_TEST: 'smoke_test',
      TRANSCRIBE: 'transcribe'
    });
  });
});
