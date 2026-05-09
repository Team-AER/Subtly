//! User-facing settings shared across UI and core. Persists to
//! `${config_dir}/subtly/settings.json` so they survive restart.

use crate::output::replace::ReplaceRule;
use crate::transcribe::TranscribeParams;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub input_path: String,
    pub output_dir: String,
    pub selected_model: Option<String>,
    pub threads: usize,
    pub beam_size: u32,
    pub best_of: u32,
    pub max_len_chars: u32,
    pub split_on_word: bool,
    pub vad_threshold: f32,
    pub vad_min_speech_ms: u32,
    pub vad_min_sil_ms: u32,
    pub vad_pad_ms: u32,
    pub no_speech_thold: f32,
    pub max_context: u32,
    pub dedup_merge_gap_sec: f32,
    pub translate: bool,
    pub language: String,
    pub flash_attn: bool,
    pub export_formats: Vec<String>,
    pub dry_run: bool,

    /// Initial prompt biasing the decoder. Use this to teach Whisper
    /// proper nouns / brand names / domain jargon up-front so the
    /// transcript spells them right.
    pub initial_prompt: String,
    /// Find/replace rules applied to every segment before output writers
    /// run. Empty by default; users add entries as they spot recurring
    /// mistranscriptions.
    pub replacements: Vec<ReplaceRule>,
    /// When true, run the cue resegmenter after inference. On by default
    /// because Whisper's native cue boundaries are too irregular for
    /// subtitle players.
    pub resegment_enabled: bool,
    pub max_cue_chars: u32,
    pub max_cue_ms: u32,
    pub min_cue_ms: u32,
    /// Whether to ask Whisper for per-token timestamps. Required by the
    /// resegmenter; turning it off saves a small amount of work but
    /// disables word-aligned cue boundaries.
    pub token_timestamps: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            input_path: String::new(),
            output_dir: String::new(),
            selected_model: Some(crate::models::DEFAULT_WHISPER_MODEL_ID.to_string()),
            threads: num_cpus::get(),
            beam_size: 8,
            best_of: 8,
            max_len_chars: 60,
            split_on_word: true,
            vad_threshold: 0.35,
            vad_min_speech_ms: 200,
            vad_min_sil_ms: 250,
            vad_pad_ms: 80,
            no_speech_thold: 0.75,
            max_context: 0,
            dedup_merge_gap_sec: 0.6,
            translate: false,
            language: "auto".to_string(),
            flash_attn: crate::whisper::flash_attention_is_safe(),
            export_formats: vec!["srt".to_string()],
            dry_run: false,
            initial_prompt: String::new(),
            replacements: Vec::new(),
            resegment_enabled: true,
            max_cue_chars: 84,
            max_cue_ms: 6000,
            min_cue_ms: 800,
            token_timestamps: true,
        }
    }
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("input path is required")]
    InputPathRequired,
    #[error("at least one export format is required")]
    NoExportFormats,
    #[error("threads must be positive")]
    BadThreads,
    #[error("beam_size must be positive")]
    BadBeam,
    #[error("best_of must be positive")]
    BadBestOf,
    #[error("max_len_chars must be positive")]
    BadMaxLen,
    #[error("language is required")]
    LanguageRequired,
}

impl Settings {
    /// Convert to a transcribe payload with validation. Returns the first error
    /// encountered.
    pub fn build_transcribe_params(
        &self,
        whisper_model_path: &str,
        vad_model_path: &str,
    ) -> Result<TranscribeParams, ValidationError> {
        if self.input_path.trim().is_empty() {
            return Err(ValidationError::InputPathRequired);
        }
        if self.export_formats.is_empty() {
            return Err(ValidationError::NoExportFormats);
        }
        if self.threads == 0 {
            return Err(ValidationError::BadThreads);
        }
        if self.beam_size == 0 {
            return Err(ValidationError::BadBeam);
        }
        if self.best_of == 0 {
            return Err(ValidationError::BadBestOf);
        }
        if self.max_len_chars == 0 {
            return Err(ValidationError::BadMaxLen);
        }
        if self.language.trim().is_empty() {
            return Err(ValidationError::LanguageRequired);
        }
        Ok(TranscribeParams {
            input_path: self.input_path.clone(),
            output_dir: if self.output_dir.is_empty() {
                None
            } else {
                Some(self.output_dir.clone())
            },
            model_path: Some(whisper_model_path.to_string()),
            vad_model_path: Some(vad_model_path.to_string()),
            threads: Some(self.threads),
            beam_size: Some(self.beam_size),
            best_of: Some(self.best_of),
            max_len_chars: Some(self.max_len_chars),
            split_on_word: Some(self.split_on_word),
            vad_threshold: Some(self.vad_threshold),
            vad_min_speech_ms: Some(self.vad_min_speech_ms),
            vad_min_sil_ms: Some(self.vad_min_sil_ms),
            vad_pad_ms: Some(self.vad_pad_ms),
            no_speech_thold: Some(self.no_speech_thold),
            max_context: Some(self.max_context),
            dedup_merge_gap_sec: Some(self.dedup_merge_gap_sec),
            translate: Some(self.translate),
            language: Some(self.language.clone()),
            flash_attn: Some(self.flash_attn),
            output_formats: Some(self.export_formats.clone()),
            dry_run: Some(self.dry_run),
            initial_prompt: Some(self.initial_prompt.clone()),
            replacements: Some(self.replacements.clone()),
            resegment_enabled: Some(self.resegment_enabled),
            max_cue_chars: Some(self.max_cue_chars),
            max_cue_ms: Some(self.max_cue_ms),
            min_cue_ms: Some(self.min_cue_ms),
            token_timestamps: Some(self.token_timestamps),
        })
    }
}

const SETTINGS_FILENAME: &str = "settings.json";

pub fn settings_path() -> PathBuf {
    crate::paths::config_directory().join(SETTINGS_FILENAME)
}

pub fn load_settings() -> Settings {
    let path = settings_path();
    let Ok(bytes) = std::fs::read(&path) else {
        return Settings::default();
    };
    let mut settings: Settings = serde_json::from_slice(&bytes).unwrap_or_default();
    if settings.selected_model.is_none() {
        settings.selected_model = Some(crate::models::DEFAULT_WHISPER_MODEL_ID.to_string());
    }
    if !crate::whisper::flash_attention_is_safe() {
        settings.flash_attn = false;
    }
    settings
}

pub fn save_settings(settings: &Settings) -> std::io::Result<()> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(settings)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(path, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_aiko_style_baseline() {
        let s = Settings::default();
        assert_eq!(
            s.selected_model.as_deref(),
            Some(crate::models::DEFAULT_WHISPER_MODEL_ID)
        );
        assert_eq!(s.beam_size, 8);
        assert_eq!(s.language, "auto");
        assert!(!s.translate);
        assert_eq!(s.flash_attn, crate::whisper::flash_attention_is_safe());
        assert_eq!(s.export_formats, vec!["srt".to_string()]);
    }

    #[test]
    fn empty_input_rejected() {
        let s = Settings::default();
        assert!(matches!(
            s.build_transcribe_params("model.bin", "vad.bin"),
            Err(ValidationError::InputPathRequired)
        ));
    }

    #[test]
    fn happy_path_builds_params() {
        let mut s = Settings::default();
        s.input_path = "/tmp/audio.wav".into();
        let p = s.build_transcribe_params("m.bin", "v.bin").unwrap();
        assert_eq!(p.input_path, "/tmp/audio.wav");
        assert_eq!(p.model_path.as_deref(), Some("m.bin"));
    }
}
