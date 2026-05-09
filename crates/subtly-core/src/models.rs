//! Whisper model catalog. Ported from `src/shared/models.js`.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDescriptor {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub size: &'static str,
    pub size_bytes: u64,
    pub url: &'static str,
    pub filename: &'static str,
    pub recommended: bool,
}

pub const DEFAULT_WHISPER_MODEL_ID: &str = "large-v2";
pub const DEFAULT_WHISPER_MODEL_FILENAME: &str = "ggml-large-v2.bin";

pub const WHISPER_MODELS: &[ModelDescriptor] = &[
    ModelDescriptor {
        id: DEFAULT_WHISPER_MODEL_ID,
        name: "Large V2",
        description: "Accuracy-first default, matching Aiko's documented macOS model choice.",
        size: "3.09 GB",
        size_bytes: 3_094_623_232,
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v2.bin",
        filename: DEFAULT_WHISPER_MODEL_FILENAME,
        recommended: true,
    },
    ModelDescriptor {
        id: "large-v3-turbo-q5_0",
        name: "Large V3 Turbo (Q5)",
        description:
            "Smaller, faster turbo model. Use when speed matters more than maximum accuracy.",
        size: "574 MB",
        size_bytes: 601_976_064,
        url:
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin",
        filename: "ggml-large-v3-turbo-q5_0.bin",
        recommended: false,
    },
    ModelDescriptor {
        id: "large-v3-turbo",
        name: "Large V3 Turbo",
        description: "Full precision turbo model. Faster, but not the accuracy-first default.",
        size: "1.5 GB",
        size_bytes: 1_624_555_275,
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin",
        filename: "ggml-large-v3-turbo.bin",
        recommended: false,
    },
    ModelDescriptor {
        id: "large-v3",
        name: "Large V3",
        description: "Latest large model. Try it if it works better for your audio.",
        size: "3.1 GB",
        size_bytes: 3_094_623_232,
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin",
        filename: "ggml-large-v3.bin",
        recommended: false,
    },
    ModelDescriptor {
        id: "medium",
        name: "Medium",
        description: "Good accuracy with faster processing. Works well for clear audio.",
        size: "1.5 GB",
        size_bytes: 1_527_742_464,
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin",
        filename: "ggml-medium.bin",
        recommended: false,
    },
    ModelDescriptor {
        id: "small",
        name: "Small",
        description: "Fast processing with decent accuracy. Good for quick drafts.",
        size: "488 MB",
        size_bytes: 487_601_152,
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
        filename: "ggml-small.bin",
        recommended: false,
    },
    ModelDescriptor {
        id: "base",
        name: "Base",
        description: "Lightweight model for basic transcription needs.",
        size: "148 MB",
        size_bytes: 147_951_488,
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
        filename: "ggml-base.bin",
        recommended: false,
    },
    ModelDescriptor {
        id: "tiny",
        name: "Tiny",
        description: "Fastest model. Use for testing or very simple audio.",
        size: "78 MB",
        size_bytes: 77_691_904,
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin",
        filename: "ggml-tiny.bin",
        recommended: false,
    },
];

pub const VAD_MODEL: ModelDescriptor = ModelDescriptor {
    id: "silero-vad",
    name: "Silero VAD",
    description: "Voice Activity Detection model (required)",
    size: "864 KB",
    size_bytes: 885_098,
    url: "https://huggingface.co/ggml-org/whisper-vad/resolve/main/ggml-silero-v5.1.2.bin",
    filename: "silero_vad.bin",
    recommended: false,
};

pub fn find_descriptor(model_id: &str) -> Option<&'static ModelDescriptor> {
    if model_id == VAD_MODEL.id {
        return Some(&VAD_MODEL);
    }
    WHISPER_MODELS.iter().find(|m| m.id == model_id)
}

#[derive(Debug, Clone, Serialize)]
pub struct InstalledModel {
    pub descriptor: ModelDescriptor,
    pub path: PathBuf,
    pub installed_size: u64,
    pub complete: bool,
}

/// Enumerate models present in `models_dir`. Mirrors `getInstalledModels` in main.js.
pub fn installed_models(models_dir: &std::path::Path) -> Vec<InstalledModel> {
    let mut out = Vec::new();
    let mut consider = |descriptor: &ModelDescriptor| {
        let path = models_dir.join(descriptor.filename);
        if let Ok(meta) = fs::metadata(&path) {
            let installed_size = meta.len();
            let complete = is_size_acceptable(installed_size, descriptor.size_bytes);
            out.push(InstalledModel {
                descriptor: descriptor.clone(),
                path,
                installed_size,
                complete,
            });
        }
    };

    for m in WHISPER_MODELS {
        consider(m);
    }
    consider(&VAD_MODEL);
    out
}

fn is_size_acceptable(actual: u64, expected: u64) -> bool {
    if actual == expected {
        return true;
    }
    let lo = (expected as f64 * 0.95) as u64;
    let hi = (expected as f64 * 1.05) as u64;
    actual >= lo && actual <= hi
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vad_descriptor_present() {
        assert_eq!(VAD_MODEL.id, "silero-vad");
    }

    #[test]
    fn find_descriptor_handles_known_ids() {
        assert!(find_descriptor("silero-vad").is_some());
        assert!(find_descriptor(DEFAULT_WHISPER_MODEL_ID).is_some());
        assert!(find_descriptor("large-v3").is_some());
        assert!(find_descriptor("nonexistent").is_none());
    }

    #[test]
    fn size_tolerance_works() {
        assert!(is_size_acceptable(1000, 1000));
        assert!(is_size_acceptable(960, 1000));
        assert!(!is_size_acceptable(900, 1000));
        assert!(is_size_acceptable(1040, 1000));
        assert!(!is_size_acceptable(1100, 1000));
    }
}
