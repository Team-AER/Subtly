import { z } from 'zod';

const deviceSchema = z.object({
  name: z.string(),
  vendor: z.number().optional(),
  device: z.number().optional(),
  device_type: z.string(),
  backend: z.string(),
  driver: z.string().optional(),
  driver_info: z.string().optional()
});

const pingSchema = z.object({
  message: z.string(),
  backend: z.string()
});

const listDevicesSchema = z.object({
  devices: z.array(deviceSchema)
});

const smokeSchema = z.object({
  message: z.string()
});

const transcribeSchema = z.object({
  jobs: z.number(),
  outputs: z.array(z.string())
});

function ensureRuntime() {
  if (!window.aerRuntime) {
    throw new Error('Runtime bridge not available');
  }
  return window.aerRuntime;
}

export async function pingRuntime() {
  const runtime = ensureRuntime();
  const result = await runtime.ping();
  return pingSchema.parse(result);
}

export async function listDevices() {
  const runtime = ensureRuntime();
  const result = await runtime.listDevices();
  return listDevicesSchema.parse(result);
}

export async function runSmokeTest() {
  const runtime = ensureRuntime();
  const result = await runtime.smokeTest();
  return smokeSchema.parse(result);
}

export async function transcribe(payload) {
  const runtime = ensureRuntime();
  const result = await runtime.transcribe(payload);
  return transcribeSchema.parse(result);
}
