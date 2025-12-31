import { describe, it, expect, beforeEach } from 'vitest';
import { pingRuntime, listDevices, runSmokeTest, transcribe } from '@/api/runtime';

describe('runtime api', () => {
  beforeEach(() => {
    delete window.aerRuntime;
  });

  it('throws when runtime bridge is missing', async () => {
    await expect(pingRuntime()).rejects.toThrow('Runtime bridge not available');
  });

  it('parses runtime responses', async () => {
    window.aerRuntime = {
      ping: async () => ({ message: 'ok', backend: 'Metal' }),
      listDevices: async () => ({ devices: [{ name: 'GPU', device_type: 'DiscreteGpu', backend: 'Vulkan' }] }),
      smokeTest: async () => ({ message: 'smoke ok' }),
      transcribe: async () => ({ jobs: 1, outputs: ['/tmp/out.srt'] })
    };

    await expect(pingRuntime()).resolves.toEqual({ message: 'ok', backend: 'Metal' });
    await expect(listDevices()).resolves.toEqual({
      devices: [{ name: 'GPU', device_type: 'DiscreteGpu', backend: 'Vulkan' }]
    });
    await expect(runSmokeTest()).resolves.toEqual({ message: 'smoke ok' });
    await expect(transcribe({})).resolves.toEqual({ jobs: 1, outputs: ['/tmp/out.srt'] });
  });
});
