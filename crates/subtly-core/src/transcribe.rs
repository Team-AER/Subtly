//! Transcription orchestrator. Walks input file(s), pipes each through the
//! in-process pipeline (decode → downmix → resample → loudnorm → whisper)
//! and writes the requested output formats. Emits `Event`s as it goes.

use crate::audio::decode_to_mono_16k;
use crate::events::{Event, ProgressUpdate};
use crate::models::DEFAULT_WHISPER_MODEL_FILENAME;
use crate::output::replace::ReplaceRule;
use crate::output::resegment::{self, ResegmentConfig};
use crate::output::{self, Segment};
use crate::paths::{ensure_path_exists, resolve_asset_dir, resolve_optional_path};
use crate::whisper::{transcribe_samples, WhisperConfig};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use tokio::sync::{mpsc, watch};
use walkdir::WalkDir;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TranscribeParams {
    pub input_path: String,
    pub output_dir: Option<String>,
    pub model_path: Option<String>,
    pub vad_model_path: Option<String>,
    pub threads: Option<usize>,
    pub beam_size: Option<u32>,
    pub best_of: Option<u32>,
    pub max_len_chars: Option<u32>,
    pub split_on_word: Option<bool>,
    pub vad_threshold: Option<f32>,
    pub vad_min_speech_ms: Option<u32>,
    pub vad_min_sil_ms: Option<u32>,
    pub vad_pad_ms: Option<u32>,
    pub no_speech_thold: Option<f32>,
    pub max_context: Option<u32>,
    pub dedup_merge_gap_sec: Option<f32>,
    pub translate: Option<bool>,
    pub language: Option<String>,
    pub flash_attn: Option<bool>,
    pub output_formats: Option<Vec<String>>,
    pub dry_run: Option<bool>,
    pub initial_prompt: Option<String>,
    pub replacements: Option<Vec<ReplaceRule>>,
    pub resegment_enabled: Option<bool>,
    pub max_cue_chars: Option<u32>,
    pub max_cue_ms: Option<u32>,
    pub min_cue_ms: Option<u32>,
    pub token_timestamps: Option<bool>,
}

#[derive(Debug)]
pub struct TranscribeConfig {
    pub input_path: PathBuf,
    pub output_dir: Option<PathBuf>,
    pub model_path: String,
    pub vad_model_path: String,
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
    pub output_formats: Vec<String>,
    pub dry_run: bool,
    pub initial_prompt: String,
    pub replacements: Vec<ReplaceRule>,
    pub resegment_enabled: bool,
    pub max_cue_chars: u32,
    pub max_cue_ms: u32,
    pub min_cue_ms: u32,
    pub token_timestamps: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct TranscribeOutcome {
    pub jobs: usize,
    pub outputs: Vec<String>,
}

impl TranscribeParams {
    pub fn into_config(self) -> Result<TranscribeConfig> {
        if self.input_path.trim().is_empty() {
            return Err(anyhow!("input_path is required"));
        }
        let asset_dir = resolve_asset_dir();
        let asset = asset_dir.as_ref();
        let default_model_path = format!("models/{DEFAULT_WHISPER_MODEL_FILENAME}");
        Ok(TranscribeConfig {
            input_path: PathBuf::from(self.input_path),
            output_dir: self.output_dir.map(PathBuf::from),
            model_path: resolve_optional_path(
                self.model_path.as_deref(),
                asset.map(|d| d.join(&default_model_path)),
                &default_model_path,
            ),
            vad_model_path: resolve_optional_path(
                self.vad_model_path.as_deref(),
                asset.map(|d| d.join("models/silero_vad.bin")),
                "models/silero_vad.bin",
            ),
            threads: self.threads.unwrap_or_else(num_cpus::get),
            beam_size: self.beam_size.unwrap_or(8),
            best_of: self.best_of.unwrap_or(8),
            max_len_chars: self.max_len_chars.unwrap_or(60),
            split_on_word: self.split_on_word.unwrap_or(true),
            vad_threshold: self.vad_threshold.unwrap_or(0.35),
            vad_min_speech_ms: self.vad_min_speech_ms.unwrap_or(200),
            vad_min_sil_ms: self.vad_min_sil_ms.unwrap_or(250),
            vad_pad_ms: self.vad_pad_ms.unwrap_or(80),
            no_speech_thold: self.no_speech_thold.unwrap_or(0.75),
            max_context: self.max_context.unwrap_or(0),
            dedup_merge_gap_sec: self.dedup_merge_gap_sec.unwrap_or(0.6),
            translate: self.translate.unwrap_or(false),
            language: self.language.unwrap_or_else(|| "auto".to_string()),
            flash_attn: self
                .flash_attn
                .unwrap_or_else(crate::whisper::flash_attention_is_safe)
                && crate::whisper::flash_attention_is_safe(),
            output_formats: self
                .output_formats
                .unwrap_or_else(|| vec!["srt".to_string()]),
            dry_run: self.dry_run.unwrap_or(false),
            initial_prompt: self.initial_prompt.unwrap_or_default(),
            replacements: self.replacements.unwrap_or_default(),
            resegment_enabled: self.resegment_enabled.unwrap_or(true),
            max_cue_chars: self.max_cue_chars.unwrap_or(84),
            max_cue_ms: self.max_cue_ms.unwrap_or(6000),
            min_cue_ms: self.min_cue_ms.unwrap_or(800),
            token_timestamps: self.token_timestamps.unwrap_or(true),
        })
    }
}

/// Run the transcription pipeline. Emits log/progress/segment events.
pub async fn transcribe(
    params: TranscribeParams,
    events: mpsc::Sender<Event>,
    cancel: watch::Receiver<bool>,
) -> Result<TranscribeOutcome> {
    let config = params.into_config()?;
    let inputs = collect_inputs(&config.input_path)?;
    if inputs.is_empty() {
        return Err(anyhow!(
            "No media files found at {}",
            config.input_path.display()
        ));
    }

    if !config.dry_run {
        ensure_path_exists("Whisper model", &config.model_path)?;
        ensure_path_exists("VAD model", &config.vad_model_path)?;
    }

    let mut outputs = Vec::new();
    let total_files = inputs.len();
    for (idx, input_path) in inputs.into_iter().enumerate() {
        if *cancel.borrow() {
            return Err(anyhow!("Transcription cancelled"));
        }
        let _ = events
            .send(Event::Progress(ProgressUpdate {
                progress: Some(((idx as f32 / total_files as f32) * 100.0) as u8),
                current: Some((idx + 1) as u64),
                total: Some(total_files as u64),
                phase: Some(format!(
                    "Processing {}",
                    input_path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("file")
                )),
            }))
            .await;

        let output_base = resolve_output_base(&config, &input_path)?;
        let outputs_for_file: Vec<PathBuf> = config
            .output_formats
            .iter()
            .map(|fmt| output_base.with_extension(output::format_to_ext(fmt)))
            .collect();

        let needs_run = outputs_for_file
            .iter()
            .any(|o| !is_up_to_date(&input_path, o));
        if !needs_run {
            let _ = events
                .send(Event::Log(format!(
                    "SKIP (up-to-date): {}",
                    input_path.display()
                )))
                .await;
            for out in &outputs_for_file {
                outputs.push(out.display().to_string());
            }
            continue;
        }

        let _ = events
            .send(Event::Log(format!("Processing {}", input_path.display())))
            .await;

        if config.dry_run {
            let _ = events
                .send(Event::Log(format!(
                    "DRY-RUN would decode + transcribe {}",
                    input_path.display()
                )))
                .await;
            for out in &outputs_for_file {
                outputs.push(out.display().to_string());
            }
            continue;
        }

        let _ = events
            .send(Event::Log(format!(
                "Decoding audio from {}",
                input_path.display()
            )))
            .await;
        let path_for_decode = input_path.clone();
        let samples = tokio::task::spawn_blocking(move || decode_to_mono_16k(&path_for_decode))
            .await
            .map_err(|e| anyhow!("decode task panicked: {e}"))??;

        let segments: Vec<Segment> = transcribe_samples(
            samples,
            build_whisper_config(&config),
            events.clone(),
            cancel.clone(),
        )
        .await?;

        let segments = post_process(segments, &config);

        write_outputs(&output_base, &segments, &config, &events, &mut outputs).await?;
    }

    let _ = events.send(Event::Done).await;
    Ok(TranscribeOutcome {
        jobs: outputs.len(),
        outputs,
    })
}

/// Apply the resegmenter (cue boundary cleanup) and the user's find/replace
/// dictionary to the engine's segment list. Order matters: resegment first
/// so word-boundary breaks operate on the verbatim transcript, then apply
/// replacements so corrected text shows up in every output format.
fn post_process(mut segments: Vec<Segment>, config: &TranscribeConfig) -> Vec<Segment> {
    if config.resegment_enabled {
        segments = resegment::run(
            &segments,
            ResegmentConfig {
                max_chars: config.max_cue_chars,
                max_ms: config.max_cue_ms,
                min_ms: config.min_cue_ms,
            },
        );
    }
    if !config.replacements.is_empty() {
        crate::output::replace::apply(&mut segments, &config.replacements);
    }
    segments
}

async fn write_outputs(
    output_base: &Path,
    segments: &[Segment],
    config: &TranscribeConfig,
    events: &mpsc::Sender<Event>,
    outputs: &mut Vec<String>,
) -> Result<()> {
    let written = output::write_all(
        segments,
        output_base,
        &config.output_formats,
        config.dedup_merge_gap_sec,
    )?;
    for path in written {
        let s = path.display().to_string();
        outputs.push(s.clone());
        let _ = events.send(Event::OutputWritten(s.clone())).await;
        let _ = events.send(Event::Log(format!("Wrote: {s}"))).await;
    }
    Ok(())
}

fn build_whisper_config(c: &TranscribeConfig) -> WhisperConfig {
    WhisperConfig {
        model_path: c.model_path.clone(),
        vad_model_path: c.vad_model_path.clone(),
        language: c.language.clone(),
        translate: c.translate,
        flash_attn: c.flash_attn,
        threads: c.threads,
        beam_size: c.beam_size,
        best_of: c.best_of,
        max_len_chars: c.max_len_chars,
        split_on_word: c.split_on_word,
        no_speech_thold: c.no_speech_thold,
        max_context: c.max_context,
        vad_threshold: c.vad_threshold,
        vad_min_speech_ms: c.vad_min_speech_ms,
        vad_min_sil_ms: c.vad_min_sil_ms,
        vad_pad_ms: c.vad_pad_ms,
        initial_prompt: c.initial_prompt.clone(),
        // Token timestamps default on whenever the resegmenter is on, since
        // the resegmenter needs them. We still respect an explicit override.
        token_timestamps: c.token_timestamps || c.resegment_enabled,
    }
}

fn collect_inputs(input_path: &Path) -> Result<Vec<PathBuf>> {
    if !input_path.exists() {
        return Err(anyhow!(
            "Input path does not exist: {}",
            input_path.display()
        ));
    }
    let extensions = ["mp4", "mkv", "mov", "wav", "mp3", "m4a", "flac", "ogg"];
    if input_path.is_file() {
        return Ok(vec![input_path.to_path_buf()]);
    }
    let mut files = Vec::new();
    for entry in WalkDir::new(input_path).follow_links(true) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let ext = entry
            .path()
            .extension()
            .and_then(OsStr::to_str)
            .unwrap_or("")
            .to_lowercase();
        if extensions.contains(&ext.as_str()) {
            files.push(entry.path().to_path_buf());
        }
    }
    Ok(files)
}

fn resolve_output_base(config: &TranscribeConfig, input_path: &Path) -> Result<PathBuf> {
    let stem = input_path
        .file_stem()
        .and_then(OsStr::to_str)
        .ok_or_else(|| anyhow!("Invalid input filename: {}", input_path.display()))?;
    if let Some(dir) = &config.output_dir {
        std::fs::create_dir_all(dir)?;
        return Ok(dir.join(stem));
    }
    let parent = input_path
        .parent()
        .ok_or_else(|| anyhow!("Cannot resolve parent of {}", input_path.display()))?;
    Ok(parent.join(stem))
}

fn is_up_to_date(input: &Path, output: &Path) -> bool {
    let (Ok(o), Ok(i)) = (std::fs::metadata(output), std::fs::metadata(input)) else {
        return false;
    };
    matches!(
        (i.modified(), o.modified()),
        (Ok(it), Ok(ot)) if ot >= it
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_input_path() {
        let p = TranscribeParams::default();
        assert!(p.into_config().is_err());
    }

    #[test]
    fn applies_defaults() {
        let p = TranscribeParams {
            input_path: "/tmp/x".into(),
            ..Default::default()
        };
        let c = p.into_config().unwrap();
        assert_eq!(c.beam_size, 8);
        assert_eq!(c.language, "auto");
        assert!(!c.translate);
        assert_eq!(c.flash_attn, crate::whisper::flash_attention_is_safe());
        assert!(c.resegment_enabled);
        assert!(c.token_timestamps);
        assert_eq!(c.max_cue_chars, 84);
        assert!(c.initial_prompt.is_empty());
        assert!(c.replacements.is_empty());
    }

    #[test]
    fn post_process_runs_resegment_then_replace() {
        let cfg = TranscribeConfig {
            input_path: PathBuf::from("/tmp/x"),
            output_dir: None,
            model_path: String::new(),
            vad_model_path: String::new(),
            threads: 1,
            beam_size: 1,
            best_of: 1,
            max_len_chars: 60,
            split_on_word: true,
            vad_threshold: 0.0,
            vad_min_speech_ms: 0,
            vad_min_sil_ms: 0,
            vad_pad_ms: 0,
            no_speech_thold: 0.0,
            max_context: 0,
            dedup_merge_gap_sec: 0.0,
            translate: false,
            language: "auto".into(),
            flash_attn: false,
            output_formats: vec!["srt".into()],
            dry_run: false,
            initial_prompt: String::new(),
            replacements: vec![ReplaceRule {
                from: "eryka".into(),
                to: "Areca".into(),
                case_sensitive: false,
                whole_word: true,
            }],
            resegment_enabled: false,
            max_cue_chars: 84,
            max_cue_ms: 6000,
            min_cue_ms: 0,
            token_timestamps: false,
        };
        let segs = vec![Segment::new(0, 1000, "An eryka palm")];
        let out = post_process(segs, &cfg);
        assert_eq!(out[0].text, "An Areca palm");
    }
}
