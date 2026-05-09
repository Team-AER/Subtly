//! Inference parameter bundle. Mirrors the previous CLI flags 1:1 so the
//! existing settings UI keeps working.

#[derive(Debug, Clone)]
pub struct WhisperConfig {
    pub model_path: String,
    pub vad_model_path: String,
    pub language: String,
    pub translate: bool,
    pub flash_attn: bool,
    pub threads: usize,
    pub beam_size: u32,
    pub best_of: u32,
    pub max_len_chars: u32,
    pub split_on_word: bool,
    pub no_speech_thold: f32,
    pub max_context: u32,
    pub vad_threshold: f32,
    pub vad_min_speech_ms: u32,
    pub vad_min_sil_ms: u32,
    pub vad_pad_ms: u32,
}
