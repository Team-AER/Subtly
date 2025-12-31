use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempPath;
use walkdir::WalkDir;

#[derive(Debug, Deserialize)]
struct RpcRequest {
    id: u64,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct RpcResponse {
    id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
}

#[derive(Debug, Serialize)]
struct RpcError {
    message: String,
}

fn main() -> Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout_lock = stdout.lock();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let request: RpcRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(err) => {
                let response = RpcResponse {
                    id: 0,
                    result: None,
                    error: Some(RpcError {
                        message: format!("Invalid request: {err}"),
                    }),
                };
                write_response(&mut stdout_lock, response)?;
                continue;
            }
        };

        let response = match handle_request(&request, &mut stdout_lock) {
            Ok(result) => RpcResponse {
                id: request.id,
                result: Some(result),
                error: None,
            },
            Err(err) => RpcResponse {
                id: request.id,
                result: None,
                error: Some(RpcError {
                    message: err.to_string(),
                }),
            },
        };

        write_response(&mut stdout_lock, response)?;
    }

    Ok(())
}

fn write_response(stdout: &mut impl Write, response: RpcResponse) -> Result<()> {
    let payload = serde_json::to_string(&response)?;
    writeln!(stdout, "{payload}")?;
    stdout.flush()?;
    Ok(())
}

fn write_event(stdout: &mut impl Write, event: &str, payload: serde_json::Value) -> Result<()> {
    let message = json!({ "event": event, "payload": payload });
    let payload = serde_json::to_string(&message)?;
    writeln!(stdout, "{payload}")?;
    stdout.flush()?;
    Ok(())
}

fn handle_request(request: &RpcRequest, stdout: &mut impl Write) -> Result<serde_json::Value> {
    match request.method.as_str() {
        "ping" => ping_with_gpu_info(),
        "list_devices" => list_devices(),
        "smoke_test" => smoke_test(),
        "transcribe" => transcribe(&request.params, stdout),
        _ => Err(anyhow!("Unknown method: {}", request.method)),
    }
}

fn ping_with_gpu_info() -> Result<serde_json::Value> {
    let instance = wgpu::Instance::default();
    let adapters = instance.enumerate_adapters(wgpu::Backends::all());
    
    if let Some(adapter) = adapters.into_iter().next() {
        let info = adapter.get_info();
        let backend_name = format!("{:?}", info.backend);
        Ok(json!({
            "message": "Runtime ready",
            "gpu_enabled": true,
            "gpu_name": info.name,
            "gpu_backend": backend_name,
            "gpu_type": format!("{:?}", info.device_type)
        }))
    } else {
        Ok(json!({
            "message": "Runtime ready (CPU fallback)",
            "gpu_enabled": false,
            "gpu_name": null,
            "gpu_backend": "CPU",
            "gpu_type": "Cpu"
        }))
    }
}

fn list_devices() -> Result<serde_json::Value> {
    let instance = wgpu::Instance::default();
    let mut devices = Vec::new();

    for adapter in instance.enumerate_adapters(wgpu::Backends::all()) {
        let info = adapter.get_info();
        devices.push(json!({
            "name": info.name,
            "vendor": info.vendor,
            "device": info.device,
            "device_type": format!("{:?}", info.device_type),
            "backend": format!("{:?}", info.backend),
            "driver": info.driver,
            "driver_info": info.driver_info
        }));
    }

    Ok(json!({ "devices": devices }))
}

fn smoke_test() -> Result<serde_json::Value> {
    let instance = wgpu::Instance::default();
    let adapter = instance
        .enumerate_adapters(wgpu::Backends::all())
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("No compatible GPU adapters found"))?;

    let info = adapter.get_info();

    let (device, _queue): (wgpu::Device, wgpu::Queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("aer-device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
        },
        None,
    ))?;

    let _ = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("smoke-buffer"),
        size: 1024,
        usage: wgpu::BufferUsages::STORAGE,
        mapped_at_creation: false,
    });

    Ok(json!({
        "message": format!(
            "Smoke test ok on {} ({:?})",
            info.name, info.backend
        )
    }))
}

#[derive(Debug, Deserialize)]
struct TranscribeParams {
    input_path: String,
    output_dir: Option<String>,
    model_path: Option<String>,
    vad_model_path: Option<String>,
    whisper_path: Option<String>,
    ffmpeg_path: Option<String>,
    vk_icd_filenames: Option<String>,
    threads: Option<usize>,
    beam_size: Option<u32>,
    best_of: Option<u32>,
    max_len_chars: Option<u32>,
    split_on_word: Option<bool>,
    vad_threshold: Option<f32>,
    vad_min_speech_ms: Option<u32>,
    vad_min_sil_ms: Option<u32>,
    vad_pad_ms: Option<u32>,
    no_speech_thold: Option<f32>,
    max_context: Option<u32>,
    dedup_merge_gap_sec: Option<f32>,
    translate: Option<bool>,
    language: Option<String>,
    dry_run: Option<bool>,
}

#[derive(Debug)]
struct TranscribeConfig {
    input_path: PathBuf,
    output_dir: Option<PathBuf>,
    model_path: String,
    vad_model_path: String,
    whisper_path: String,
    ffmpeg_path: String,
    vk_icd_filenames: Option<String>,
    threads: usize,
    beam_size: u32,
    best_of: u32,
    max_len_chars: u32,
    split_on_word: bool,
    vad_threshold: f32,
    vad_min_speech_ms: u32,
    vad_min_sil_ms: u32,
    vad_pad_ms: u32,
    no_speech_thold: f32,
    max_context: u32,
    dedup_merge_gap_sec: f32,
    translate: bool,
    language: String,
    dry_run: bool,
}

fn transcribe(params: &serde_json::Value, stdout: &mut impl Write) -> Result<serde_json::Value> {
    let input: TranscribeParams = serde_json::from_value(params.clone())
        .map_err(|err| anyhow!("Invalid transcribe params: {err}"))?;

    if input.input_path.trim().is_empty() {
        return Err(anyhow!("input_path is required"));
    }

    let asset_dir = resolve_asset_dir();
    let config = TranscribeConfig {
        input_path: PathBuf::from(input.input_path),
        output_dir: input.output_dir.map(PathBuf::from),
        model_path: resolve_optional_path(
            input.model_path.as_deref(),
            asset_dir
                .as_ref()
                .map(|dir| dir.join("models/ggml-large-v3.bin")),
            "models/ggml-large-v3.bin",
        ),
        vad_model_path: resolve_optional_path(
            input.vad_model_path.as_deref(),
            asset_dir
                .as_ref()
                .map(|dir| dir.join("models/ggml-silero-v6.2.0.bin")),
            "models/ggml-silero-v6.2.0.bin",
        ),
        whisper_path: resolve_optional_path(
            input.whisper_path.as_deref(),
            asset_dir
                .as_ref()
                .map(|dir| dir.join("bin").join(default_binary_name("whisper-cli"))),
            "./build/bin/whisper-cli",
        ),
        ffmpeg_path: resolve_optional_path(
            input.ffmpeg_path.as_deref(),
            asset_dir
                .as_ref()
                .map(|dir| dir.join("bin").join(default_binary_name("ffmpeg"))),
            "ffmpeg",
        ),
        vk_icd_filenames: input
            .vk_icd_filenames
            .filter(|value| !value.trim().is_empty()),
        threads: input.threads.unwrap_or_else(num_cpus::get),
        beam_size: input.beam_size.unwrap_or(8),
        best_of: input.best_of.unwrap_or(8),
        max_len_chars: input.max_len_chars.unwrap_or(60),
        split_on_word: input.split_on_word.unwrap_or(true),
        vad_threshold: input.vad_threshold.unwrap_or(0.35),
        vad_min_speech_ms: input.vad_min_speech_ms.unwrap_or(200),
        vad_min_sil_ms: input.vad_min_sil_ms.unwrap_or(250),
        vad_pad_ms: input.vad_pad_ms.unwrap_or(80),
        no_speech_thold: input.no_speech_thold.unwrap_or(0.75),
        max_context: input.max_context.unwrap_or(0),
        dedup_merge_gap_sec: input.dedup_merge_gap_sec.unwrap_or(0.6),
        translate: input.translate.unwrap_or(true),
        language: input.language.unwrap_or_else(|| "auto".to_string()),
        dry_run: input.dry_run.unwrap_or(false),
    };

    let inputs = collect_inputs(&config.input_path)?;
    if inputs.is_empty() {
        return Err(anyhow!("No media files found at {}", config.input_path.display()));
    }

    if !config.dry_run {
        ensure_path_exists("whisper-cli", &config.whisper_path)?;
        ensure_path_exists("Whisper model", &config.model_path)?;
        ensure_path_exists("VAD model", &config.vad_model_path)?;
        ensure_executable_available("ffmpeg", &config.ffmpeg_path)?;
    }

    let mut outputs = Vec::new();
    for input_path in inputs {
        let output_base = resolve_output_base(&config, &input_path)?;
        let output_srt = output_base.with_extension("srt");

        if is_up_to_date(&input_path, &output_srt) {
            write_event(
                stdout,
                "log",
                json!(format!("SKIP (up-to-date): {}", input_path.display())),
            )?;
            outputs.push(output_srt.display().to_string());
            continue;
        }

        write_event(
            stdout,
            "log",
            json!(format!("Processing {}", input_path.display())),
        )?;

        let mut tmp_file: Option<TempPath> = None;
        let tmp_wav = if config.dry_run {
            output_base.with_extension("__tmp__.wav")
        } else {
            let temp_path = tempfile::Builder::new().suffix(".wav").tempfile()?.into_temp_path();
            let path = temp_path.to_path_buf();
            tmp_file = Some(temp_path);
            path
        };

        let input_path_arg = input_path.to_string_lossy();
        let tmp_wav_arg = tmp_wav.to_string_lossy();
        let ffmpeg_args = [
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-i",
            input_path_arg.as_ref(),
            "-vn",
            "-af",
            "pan=mono|c0=0.35*FL+0.35*FR+0.80*FC+0.15*SL+0.15*SR,loudnorm=I=-16:LRA=11:TP=-1.5",
            "-ar",
            "16000",
            "-c:a",
            "pcm_s16le",
            tmp_wav_arg.as_ref(),
        ];

        run_command(
            stdout,
            &config.ffmpeg_path,
            &ffmpeg_args,
            config.dry_run,
            config.vk_icd_filenames.as_deref(),
        )?;

        let mut whisper_args = vec![
            "-m".to_string(),
            config.model_path.clone(),
            "-f".to_string(),
            tmp_wav.to_string_lossy().to_string(),
            "-l".to_string(),
            config.language.clone(),
        ];

        if config.translate {
            whisper_args.push("-tr".to_string());
        }

        whisper_args.extend([
            "-t".to_string(),
            config.threads.to_string(),
            "-bs".to_string(),
            config.beam_size.to_string(),
            "-bo".to_string(),
            config.best_of.to_string(),
            "-nth".to_string(),
            config.no_speech_thold.to_string(),
            "-mc".to_string(),
            config.max_context.to_string(),
            "--suppress-nst".to_string(),
            "--vad".to_string(),
            "-vm".to_string(),
            config.vad_model_path.clone(),
            "-vt".to_string(),
            config.vad_threshold.to_string(),
            "-vspd".to_string(),
            config.vad_min_speech_ms.to_string(),
            "-vsd".to_string(),
            config.vad_min_sil_ms.to_string(),
            "-vp".to_string(),
            config.vad_pad_ms.to_string(),
            "-ml".to_string(),
            config.max_len_chars.to_string(),
            "-osrt".to_string(),
            "-of".to_string(),
            output_base.to_string_lossy().to_string(),
            "-pp".to_string(),
        ]);

        if config.split_on_word {
            whisper_args.push("-sow".to_string());
        }

        run_command(
            stdout,
            &config.whisper_path,
            &whisper_args,
            config.dry_run,
            config.vk_icd_filenames.as_deref(),
        )?;

        if !config.dry_run {
            dedup_srt(&output_srt, config.dedup_merge_gap_sec)?;
        } else {
            write_event(
                stdout,
                "log",
                json!(format!(
                    "DRY-RUN post-process SRT: {}",
                    output_srt.display()
                )),
            )?;
        }

        drop(tmp_file);

        outputs.push(output_srt.display().to_string());
        write_event(
            stdout,
            "log",
            json!(format!("Wrote: {}", output_srt.display())),
        )?;
    }

    Ok(json!({
        "jobs": outputs.len(),
        "outputs": outputs
    }))
}

fn resolve_asset_dir() -> Option<PathBuf> {
    if let Ok(value) = std::env::var("AER_ASSET_DIR") {
        let path = PathBuf::from(value);
        if path.exists() {
            return Some(path);
        }
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let candidate = parent.join("assets");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    let cwd = std::env::current_dir().ok();
    let candidates = [
        cwd.as_ref().map(|dir| dir.join("runtime/assets")),
        cwd.as_ref().map(|dir| dir.join("assets")),
    ];
    for candidate in candidates.into_iter().flatten() {
        if candidate.exists() {
            return Some(candidate);
        }
    }

    None
}

fn default_binary_name(base: &str) -> String {
    if cfg!(windows) {
        format!("{base}.exe")
    } else {
        base.to_string()
    }
}

fn resolve_optional_path(
    value: Option<&str>,
    asset_default: Option<PathBuf>,
    fallback: &str,
) -> String {
    if let Some(raw) = value {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    if let Some(path) = asset_default {
        return path.to_string_lossy().to_string();
    }
    fallback.to_string()
}

fn ensure_path_exists(label: &str, path: &str) -> Result<()> {
    let resolved = Path::new(path);
    if resolved.exists() {
        return Ok(());
    }
    Err(anyhow!("{label} not found at {path}"))
}

fn ensure_executable_available(label: &str, path: &str) -> Result<()> {
    let resolved = Path::new(path);
    if resolved.is_absolute() || path.contains(std::path::MAIN_SEPARATOR) {
        if !resolved.exists() {
            return Err(anyhow!("{label} not found at {path}"));
        }
    }
    Ok(())
}

fn collect_inputs(input_path: &Path) -> Result<Vec<PathBuf>> {
    if !input_path.exists() {
        return Err(anyhow!("Input path does not exist: {}", input_path.display()));
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
    let file_stem = input_path
        .file_stem()
        .and_then(OsStr::to_str)
        .ok_or_else(|| anyhow!("Invalid input filename: {}", input_path.display()))?;

    if let Some(output_dir) = &config.output_dir {
        fs::create_dir_all(output_dir)?;
        return Ok(output_dir.join(file_stem));
    }

    let parent = input_path
        .parent()
        .ok_or_else(|| anyhow!("Missing parent directory"))?;
    Ok(parent.join(file_stem))
}

fn is_up_to_date(input: &Path, output: &Path) -> bool {
    let output_meta = match fs::metadata(output) {
        Ok(meta) => meta,
        Err(_) => return false,
    };
    let input_meta = match fs::metadata(input) {
        Ok(meta) => meta,
        Err(_) => return false,
    };

    match (input_meta.modified(), output_meta.modified()) {
        (Ok(input_time), Ok(output_time)) => output_time >= input_time,
        _ => false,
    }
}

fn run_command(
    stdout: &mut impl Write,
    program: &str,
    args: &[impl AsRef<OsStr>],
    dry_run: bool,
    vk_icd_filenames: Option<&str>,
) -> Result<()> {
    let rendered = format!(
        "{} {}",
        program,
        args.iter()
            .map(|arg| arg.as_ref().to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ")
    );
    if dry_run {
        write_event(stdout, "log", json!(format!("DRY-RUN {}", rendered)))?;
        return Ok(());
    }

    let mut command = Command::new(program);
    command.args(args);
    command.stdout(std::process::Stdio::null());
    command.stderr(std::process::Stdio::null());
    if let Some(value) = vk_icd_filenames {
        command.env("VK_ICD_FILENAMES", value);
    }
    let status = command.status()?;
    if !status.success() {
        return Err(anyhow!("Command failed: {}", rendered));
    }
    Ok(())
}

#[derive(Debug)]
struct SubtitleItem {
    start_ms: i64,
    end_ms: i64,
    text: String,
    norm: String,
}

fn dedup_srt(path: &Path, merge_gap_sec: f32) -> Result<()> {
    let content = match fs::read_to_string(path) {
        Ok(data) => data,
        Err(_) => return Ok(()),
    };

    let normalized = content.replace("\r\n", "\n");
    if normalized.trim().is_empty() {
        return Ok(());
    }

    let mut items = Vec::new();
    for block in normalized.split("\n\n") {
        let mut lines = block.lines();
        let _index = lines.next();
        let times = match lines.next() {
            Some(line) if line.contains("-->") => line,
            _ => continue,
        };
        let mut time_parts = times.split("-->").map(|s| s.trim());
        let start = time_parts.next().unwrap_or_default();
        let end = time_parts.next().unwrap_or_default();
        let start_ms = timestamp_to_ms(start)?;
        let end_ms = timestamp_to_ms(end)?;
        let text = lines.collect::<Vec<_>>().join("\n").trim().to_string();
        if text.is_empty() {
            continue;
        }
        let norm = text
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        items.push(SubtitleItem {
            start_ms,
            end_ms,
            text,
            norm,
        });
    }

    let mut merged: Vec<SubtitleItem> = Vec::new();
    let merge_gap_ms = (merge_gap_sec * 1000.0) as i64;
    for item in items {
        if let Some(prev) = merged.last_mut() {
            if prev.norm == item.norm && item.start_ms <= prev.end_ms + merge_gap_ms {
                prev.end_ms = prev.end_ms.max(item.end_ms);
                continue;
            }
        }
        merged.push(item);
    }

    let mut out = String::new();
    for (idx, item) in merged.iter().enumerate() {
        out.push_str(&(idx + 1).to_string());
        out.push('\n');
        out.push_str(&format!(
            "{} --> {}\n",
            ms_to_timestamp(item.start_ms),
            ms_to_timestamp(item.end_ms)
        ));
        out.push_str(&item.text);
        out.push_str("\n\n");
    }

    fs::write(path, out.trim_end().to_string() + "\n")?;
    Ok(())
}

fn timestamp_to_ms(ts: &str) -> Result<i64> {
    let mut parts = ts.split(':');
    let hours = parts
        .next()
        .ok_or_else(|| anyhow!("Invalid timestamp"))?
        .parse::<i64>()?;
    let minutes = parts
        .next()
        .ok_or_else(|| anyhow!("Invalid timestamp"))?
        .parse::<i64>()?;
    let seconds_ms = parts
        .next()
        .ok_or_else(|| anyhow!("Invalid timestamp"))?;
    let mut seconds_parts = seconds_ms.split(',');
    let seconds = seconds_parts
        .next()
        .ok_or_else(|| anyhow!("Invalid timestamp"))?
        .parse::<i64>()?;
    let millis = seconds_parts
        .next()
        .ok_or_else(|| anyhow!("Invalid timestamp"))?
        .parse::<i64>()?;
    Ok((hours * 3600 + minutes * 60 + seconds) * 1000 + millis)
}

fn ms_to_timestamp(ms: i64) -> String {
    let mut remaining = ms;
    let hours = remaining / 3_600_000;
    remaining %= 3_600_000;
    let minutes = remaining / 60_000;
    remaining %= 60_000;
    let seconds = remaining / 1000;
    let millis = remaining % 1000;
    format!("{hours:02}:{minutes:02}:{seconds:02},{millis:03}")
}
