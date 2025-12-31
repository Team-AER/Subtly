import { describe, it, expect } from 'vitest';
import models from '../../src/shared/models';

const { WHISPER_MODELS, VAD_MODEL } = models;

describe('shared models', () => {
  it('contains whisper models with required fields', () => {
    expect(Array.isArray(WHISPER_MODELS)).toBe(true);
    expect(WHISPER_MODELS.length).toBeGreaterThan(0);
    for (const model of WHISPER_MODELS) {
      expect(model).toEqual(
        expect.objectContaining({
          id: expect.any(String),
          name: expect.any(String),
          description: expect.any(String),
          size: expect.any(String),
          sizeBytes: expect.any(Number),
          url: expect.any(String),
          filename: expect.any(String),
          recommended: expect.any(Boolean)
        })
      );
    }
  });

  it('marks the VAD model as required', () => {
    expect(VAD_MODEL).toEqual(
      expect.objectContaining({
        id: 'silero-vad',
        required: true
      })
    );
  });
});
