//! Transcription orchestration. Lifted from `runtime/gpu-runtime/src/main.rs`
//! and converted to async + mpsc-based event emission.

use crate::events::{Event, ProgressUpdate};
use crate::paths::{
    default_binary_name, ensure_executable_available, ensure_path_exists,
    resolve_asset_dir, resolve_optional_path,
};
use crate::srt::dedup_srt;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use tempfile::TempPath;
use tokio::process::Command;
use tokio::sync::mpsc;
use walkdir::WalkDir;

/// Wire-compatible with the previous JSON-RPC `transcribe` params.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TranscribeParams {
    pub input_path: String,
    pub output_dir: Option<String>,
    pub model_path: Option<String>,
    pub vad_model_path: Option<String>,
    pub whisper_path: Option<String>,
    pub ffmpeg_path: Option<String>,
    pub vk_icd_filenames: Option<String>,
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
}

#[derive(Debug)]
pub struct TranscribeConfig {
    pub input_path: PathBuf,
    pub output_dir: Option<PathBuf>,
    pub model_path: String,
    pub vad_model_path: String,
    pub whisper_path: String,
    pub ffmpeg_path: String,
    pub vk_icd_filenames: Option<String>,
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
        Ok(TranscribeConfig {
            input_path: PathBuf::from(self.input_path),
            output_dir: self.output_dir.map(PathBuf::from),
            model_path: resolve_optional_path(
                self.model_path.as_deref(),
                asset.map(|d| d.join("models/ggml-large-v3.bin")),
                "models/ggml-large-v3.bin",
            ),
            vad_model_path: resolve_optional_path(
                self.vad_model_path.as_deref(),
                asset.map(|d| d.join("models/silero_vad.bin")),
                "models/silero_vad.bin",
            ),
            whisper_path: resolve_optional_path(
                self.whisper_path.as_deref(),
                asset.map(|d| d.join("bin").join(default_binary_name("whisper-cli"))),
                "./build/bin/whisper-cli",
            ),
            ffmpeg_path: resolve_optional_path(
                self.ffmpeg_path.as_deref(),
                asset.map(|d| d.join("bin").join(default_binary_name("ffmpeg"))),
                "ffmpeg",
            ),
            vk_icd_filenames: self
                .vk_icd_filenames
                .filter(|v| !v.trim().is_empty()),
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
            translate: self.translate.unwrap_or(true),
            language: self.language.unwrap_or_else(|| "auto".to_string()),
            flash_attn: self.flash_attn.unwrap_or(false),
            output_formats: self
                .output_formats
                .unwrap_or_else(|| vec!["srt".to_string()]),
            dry_run: self.dry_run.unwrap_or(false),
        })
    }
}

/// Run the transcription pipeline. Emits log/progress events to `events`.
pub async fn transcribe(
    params: TranscribeParams,
    events: mpsc::Sender<Event>,
    cancel: tokio::sync::watch::Receiver<bool>,
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
        ensure_path_exists("whisper-cli", &config.whisper_path)?;
        ensure_path_exists("Whisper model", &config.model_path)?;
        ensure_path_exists("VAD model", &config.vad_model_path)?;
        ensure_executable_available("ffmpeg", &config.ffmpeg_path)?;
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
        let mut outputs_for_file = Vec::new();
        for format in &config.output_formats {
            let ext = format_to_ext(format);
            outputs_for_file.push(output_base.with_extension(ext));
        }

        let mut needs_run = false;
        for output_file in &outputs_for_file {
            if !is_up_to_date(&input_path, output_file) {
                needs_run = true;
                break;
            }
        }
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

        let mut tmp_file: Option<TempPath> = None;
        let tmp_wav = if config.dry_run {
            output_base.with_extension("__tmp__.wav")
        } else {
            let temp = tempfile::Builder::new()
                .suffix(".wav")
                .tempfile()?
                .into_temp_path();
            let path = temp.to_path_buf();
            tmp_file = Some(temp);
            path
        };

        let input_arg = input_path.to_string_lossy().into_owned();
        let tmp_arg = tmp_wav.to_string_lossy().into_owned();
        let ffmpeg_args = [
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-i",
            input_arg.as_str(),
            "-vn",
            "-af",
            "pan=mono|c0=0.35*FL+0.35*FR+0.80*FC+0.15*SL+0.15*SR,loudnorm=I=-16:LRA=11:TP=-1.5",
            "-ar",
            "16000",
            "-c:a",
            "pcm_s16le",
            tmp_arg.as_str(),
        ];

        run_command(
            &events,
            &config.ffmpeg_path,
            &ffmpeg_args,
            config.dry_run,
            config.vk_icd_filenames.as_deref(),
        )
        .await?;

        let mut whisper_args: Vec<String> = vec![
            "-m".into(),
            config.model_path.clone(),
            "-f".into(),
            tmp_wav.to_string_lossy().into_owned(),
            "-l".into(),
            config.language.clone(),
        ];
        if config.translate {
            whisper_args.push("-tr".into());
        }
        whisper_args.extend([
            "-t".into(),
            config.threads.to_string(),
            "-bs".into(),
            config.beam_size.to_string(),
            "-bo".into(),
            config.best_of.to_string(),
            "-nth".into(),
            config.no_speech_thold.to_string(),
            "-mc".into(),
            config.max_context.to_string(),
            "--suppress-nst".into(),
            "--vad".into(),
            "-vm".into(),
            config.vad_model_path.clone(),
            "-vt".into(),
            config.vad_threshold.to_string(),
            "-vspd".into(),
            config.vad_min_speech_ms.to_string(),
            "-vsd".into(),
            config.vad_min_sil_ms.to_string(),
            "-vp".into(),
            config.vad_pad_ms.to_string(),
            "-ml".into(),
            config.max_len_chars.to_string(),
        ]);
        if config.flash_attn {
            whisper_args.push("-fa".into());
        } else {
            whisper_args.push("-nfa".into());
        }
        if config.split_on_word {
            whisper_args.push("-sow".into());
        }
        whisper_args.push("-of".into());
        whisper_args.push(output_base.to_string_lossy().into_owned());
        for format in &config.output_formats {
            whisper_args.push(format_to_flag(format).into());
        }

        let whisper_args_ref: Vec<&str> = whisper_args.iter().map(|s| s.as_str()).collect();
        run_command(
            &events,
            &config.whisper_path,
            &whisper_args_ref,
            config.dry_run,
            config.vk_icd_filenames.as_deref(),
        )
        .await?;

        if config.output_formats.iter().any(|f| f == "srt") && !config.dry_run {
            let output_srt = output_base.with_extension("srt");
            dedup_srt(&output_srt, config.dedup_merge_gap_sec)?;
        } else if config.dry_run {
            let _ = events
                .send(Event::Log(format!(
                    "DRY-RUN post-process SRT: {}",
                    output_base.with_extension("srt").display()
                )))
                .await;
        }

        drop(tmp_file);

        for out in outputs_for_file {
            let s = out.display().to_string();
            outputs.push(s.clone());
            let _ = events.send(Event::OutputWritten(s.clone())).await;
            let _ = events.send(Event::Log(format!("Wrote: {s}"))).await;
        }
    }

    let _ = events.send(Event::Done).await;
    Ok(TranscribeOutcome {
        jobs: outputs.len(),
        outputs,
    })
}

fn format_to_ext(format: &str) -> &'static str {
    match format {
        "vtt" => "vtt",
        "json" => "json",
        "csv" => "csv",
        "txt" => "txt",
        _ => "srt",
    }
}

fn format_to_flag(format: &str) -> &'static str {
    match format {
        "vtt" => "-ovtt",
        "json" => "-oj",
        "csv" => "-ocsv",
        "txt" => "-otxt",
        _ => "-osrt",
    }
}

fn collect_inputs(input_path: &Path) -> Result<Vec<PathBuf>> {
    if !input_path.exists() {
        return Err(anyhow!(
            "Input path does not exist: {}",
            input_path.display()
        ));
    }
    let extensions = ["mp4", "mkv", "mov", "wav", "mp3", "m4a"];
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

async fn run_command(
    events: &mpsc::Sender<Event>,
    program: &str,
    args: &[&str],
    dry_run: bool,
    vk_icd_filenames: Option<&str>,
) -> Result<()> {
    let rendered = format!("{program} {}", args.join(" "));
    if dry_run {
        let _ = events.send(Event::Log(format!("DRY-RUN {rendered}"))).await;
        return Ok(());
    }

    let mut command = Command::new(program);
    command.args(args);
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    if let Some(value) = vk_icd_filenames {
        command.env("VK_ICD_FILENAMES", value);
    }

    let program_path = Path::new(program);
    if let Some(parent) = program_path.parent() {
        if !parent.as_os_str().is_empty() {
            command.current_dir(parent);
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(parent) = program_path.parent() {
            if !parent.as_os_str().is_empty() {
                let mut paths = vec![parent.to_path_buf()];
                if let Some(existing) = std::env::var_os("LD_LIBRARY_PATH") {
                    paths.extend(std::env::split_paths(&existing));
                }
                if let Ok(joined) = std::env::join_paths(paths) {
                    command.env("LD_LIBRARY_PATH", joined);
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        let is_whisper = program_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n == "whisper-cli")
            .unwrap_or(false);
        if is_whisper {
            if let Some(parent) = program_path.parent() {
                let metal_source = parent.join("ggml-metal.metal");
                if metal_source.exists() {
                    command.env("GGML_METAL_PATH_RESOURCES", parent);
                }
            }
        }
    }

    let output = command.output().await?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{}\n{}", stderr.trim(), stdout.trim())
            .trim()
            .to_string();
        let combined = truncate_log(&combined, 8000);
        let exit_code = output
            .status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "terminated".to_string());
        if combined.is_empty() {
            return Err(anyhow!("Command failed (exit {exit_code}): {rendered}"));
        }
        return Err(anyhow!(
            "Command failed (exit {exit_code}): {rendered} ({combined})"
        ));
    }
    Ok(())
}

fn truncate_log(value: &str, max_chars: usize) -> String {
    let count = value.chars().count();
    if count <= max_chars {
        return value.to_string();
    }
    let start = count.saturating_sub(max_chars);
    value.chars().skip(start).collect()
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
        assert!(c.translate);
    }

    #[test]
    fn truncate_log_preserves_short() {
        assert_eq!(truncate_log("hello", 100), "hello");
    }

    #[test]
    fn truncate_log_clamps_long() {
        let s = "a".repeat(20);
        assert_eq!(truncate_log(&s, 5).len(), 5);
    }

    #[test]
    fn format_mapping() {
        assert_eq!(format_to_ext("vtt"), "vtt");
        assert_eq!(format_to_ext("unknown"), "srt");
        assert_eq!(format_to_flag("json"), "-oj");
    }
}
