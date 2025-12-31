/**
 * Available Whisper models for download.
 * All models from Hugging Face ggerganov/whisper.cpp repository.
 */
const WHISPER_MODELS = [
  {
    id: 'large-v3-turbo-q5_0',
    name: 'Large V3 Turbo (Q5)',
    description: 'Best balance of speed and accuracy. Recommended for most users.',
    size: '574 MB',
    sizeBytes: 601976064,
    url: 'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin',
    filename: 'ggml-large-v3-turbo-q5_0.bin',
    recommended: true
  },
  {
    id: 'large-v3-turbo',
    name: 'Large V3 Turbo',
    description: 'Full precision turbo model. Faster than Large V3 with similar quality.',
    size: '1.6 GB',
    sizeBytes: 1624182016,
    url: 'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin',
    filename: 'ggml-large-v3-turbo.bin',
    recommended: false
  },
  {
    id: 'large-v3',
    name: 'Large V3',
    description: 'Highest accuracy. Best for difficult audio or critical transcriptions.',
    size: '3.1 GB',
    sizeBytes: 3094623232,
    url: 'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin',
    filename: 'ggml-large-v3.bin',
    recommended: false
  },
  {
    id: 'medium',
    name: 'Medium',
    description: 'Good accuracy with faster processing. Works well for clear audio.',
    size: '1.5 GB',
    sizeBytes: 1527742464,
    url: 'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin',
    filename: 'ggml-medium.bin',
    recommended: false
  },
  {
    id: 'small',
    name: 'Small',
    description: 'Fast processing with decent accuracy. Good for quick drafts.',
    size: '488 MB',
    sizeBytes: 487601152,
    url: 'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin',
    filename: 'ggml-small.bin',
    recommended: false
  },
  {
    id: 'base',
    name: 'Base',
    description: 'Lightweight model for basic transcription needs.',
    size: '148 MB',
    sizeBytes: 147951488,
    url: 'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin',
    filename: 'ggml-base.bin',
    recommended: false
  },
  {
    id: 'tiny',
    name: 'Tiny',
    description: 'Fastest model. Use for testing or very simple audio.',
    size: '78 MB',
    sizeBytes: 77691904,
    url: 'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin',
    filename: 'ggml-tiny.bin',
    recommended: false
  }
];

/**
 * Silero VAD model - required for all transcriptions
 * Note: whisper.cpp requires the GGML-converted version, not the raw ONNX file
 */
const VAD_MODEL = {
  id: 'silero-vad',
  name: 'Silero VAD',
  description: 'Voice Activity Detection model (required)',
  size: '2.2 MB',
  sizeBytes: 2284548,
  url: 'https://huggingface.co/ggml-org/whisper-vad/resolve/main/ggml-silero-v5.1.2.bin',
  filename: 'silero_vad.bin',
  required: true
};

module.exports = {
  WHISPER_MODELS,
  VAD_MODEL
};
