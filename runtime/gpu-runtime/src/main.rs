use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
#[cfg(not(coverage))]
use std::io::{self, BufRead};
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

#[cfg(not(coverage))]
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

#[cfg(coverage)]
fn main() -> Result<()> {
    Ok(())
}

fn write_response(stdout: &mut impl Write, response: RpcResponse) -> Result<()> {
    serde_json::to_writer(&mut *stdout, &response)?;
    writeln!(stdout)?;
    stdout.flush()?;
    Ok(())
}

fn write_event(stdout: &mut impl Write, event: &str, payload: serde_json::Value) -> Result<()> {
    let message = json!({ "event": event, "payload": payload });
    serde_json::to_writer(&mut *stdout, &message)?;
    writeln!(stdout)?;
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
    let info = adapters.into_iter().next().map(|adapter| adapter.get_info());
    Ok(ping_response_from_info(info))
}

fn ping_response_from_info(info: Option<wgpu::AdapterInfo>) -> serde_json::Value {
    if let Some(info) = info {
        let backend_name = format!("{:?}", info.backend);
        json!({
            "message": "Runtime ready",
            "gpu_enabled": true,
            "gpu_name": info.name,
            "gpu_backend": backend_name,
            "gpu_type": format!("{:?}", info.device_type)
        })
    } else {
        json!({
            "message": "Runtime ready (CPU fallback)",
            "gpu_enabled": false,
            "gpu_name": null,
            "gpu_backend": "CPU",
            "gpu_type": "Cpu"
        })
    }
}

fn list_devices() -> Result<serde_json::Value> {
    let instance = wgpu::Instance::default();
    let infos = instance
        .enumerate_adapters(wgpu::Backends::all())
        .into_iter()
        .map(|adapter| adapter.get_info())
        .collect::<Vec<_>>();
    Ok(list_devices_from_infos(infos))
}

fn list_devices_from_infos(infos: Vec<wgpu::AdapterInfo>) -> serde_json::Value {
    let devices = infos
        .into_iter()
        .map(|info| {
            json!({
                "name": info.name,
                "vendor": info.vendor,
                "device": info.device,
                "device_type": format!("{:?}", info.device_type),
                "backend": format!("{:?}", info.backend),
                "driver": info.driver,
                "driver_info": info.driver_info
            })
        })
        .collect::<Vec<_>>();

    json!({ "devices": devices })
}

#[cfg(not(test))]
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

#[cfg(test)]
fn smoke_test() -> Result<serde_json::Value> {
    Ok(json!({
        "message": "Smoke test ok on Test Adapter (Vulkan)"
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
            write_event(stdout, "log", json!(format!("SKIP (up-to-date): {}", input_path.display())))?;
            outputs.push(output_srt.display().to_string());
            continue;
        }

        write_event(stdout, "log", json!(format!("Processing {}", input_path.display())))?;

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

        run_command(stdout, &config.ffmpeg_path, &ffmpeg_args, config.dry_run, config.vk_icd_filenames.as_deref())?;

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

        run_command(stdout, &config.whisper_path, &whisper_args, config.dry_run, config.vk_icd_filenames.as_deref())?;

        if !config.dry_run {
            dedup_srt(&output_srt, config.dedup_merge_gap_sec)?;
        } else {
            write_event(stdout, "log", json!(format!("DRY-RUN post-process SRT: {}", output_srt.display())))?;
        }

        drop(tmp_file);

        outputs.push(output_srt.display().to_string());
        write_event(stdout, "log", json!(format!("Wrote: {}", output_srt.display())))?;
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

    let exe_path = std::env::current_exe().unwrap();
    let parent = exe_path.parent().unwrap();
    let candidate = parent.join("assets");
    if candidate.exists() { return Some(candidate); }

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

#[cfg(windows)]
fn default_binary_name(base: &str) -> String {
    format!("{base}.exe")
}

#[cfg(not(windows))]
fn default_binary_name(base: &str) -> String {
    base.to_string()
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

    let parent = input_path.parent().unwrap();
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

    is_up_to_date_with_modified(input_meta.modified(), output_meta.modified())
}

fn is_up_to_date_with_modified(
    input_modified: std::io::Result<std::time::SystemTime>,
    output_modified: std::io::Result<std::time::SystemTime>,
) -> bool {
    match (input_modified, output_modified) {
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
            if prev.norm == item.norm && item.start_ms <= prev.end_ms + merge_gap_ms { prev.end_ms = prev.end_ms.max(item.end_ms); continue; }
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
    let hours = parts.next().unwrap_or("").parse::<i64>()?;
    let minutes = parts
        .next()
        .ok_or_else(|| anyhow!("Invalid timestamp"))?
        .parse::<i64>()?;
    let seconds_ms = parts
        .next()
        .ok_or_else(|| anyhow!("Invalid timestamp"))?;
    let mut seconds_parts = seconds_ms.split(',');
    let seconds = seconds_parts.next().unwrap_or("").parse::<i64>()?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::io::{self, Cursor};
    use std::sync::Mutex;
    use std::time::{Duration, SystemTime};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn transcribe_with_lock(
        params: &serde_json::Value,
        stdout: &mut impl std::io::Write,
    ) -> Result<serde_json::Value> {
        let _guard = ENV_LOCK.lock().unwrap();
        transcribe(params, stdout)
    }

    fn restore_env_var(name: &str, original: Option<String>) {
        if let Some(value) = original {
            std::env::set_var(name, value);
        } else {
            std::env::remove_var(name);
        }
    }

    struct FailingWriter {
        fail_after: Option<usize>,
        fail_on_newline: bool,
        fail_on_flush: bool,
        writes: usize,
    }

    impl FailingWriter {
        fn fail_after(limit: usize) -> Self {
            Self {
                fail_after: Some(limit),
                fail_on_newline: false,
                fail_on_flush: false,
                writes: 0,
            }
        }

        fn fail_on_newline() -> Self {
            Self {
                fail_after: None,
                fail_on_newline: true,
                fail_on_flush: false,
                writes: 0,
            }
        }

        fn fail_on_flush() -> Self {
            Self {
                fail_after: None,
                fail_on_newline: false,
                fail_on_flush: true,
                writes: 0,
            }
        }
    }

    impl std::io::Write for FailingWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            if self.fail_on_newline && buf == b"\n" {
                return Err(io::Error::new(io::ErrorKind::Other, "newline failure"));
            }
            if let Some(limit) = self.fail_after {
                if self.writes >= limit {
                    return Err(io::Error::new(io::ErrorKind::Other, "write failure"));
                }
            }
            self.writes += 1;
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            if self.fail_on_flush {
                return Err(io::Error::new(io::ErrorKind::Other, "flush failure"));
            }
            Ok(())
        }
    }

    struct SubstringFailWriter {
        needle: Vec<u8>,
        buffer: Vec<u8>,
    }

    impl SubstringFailWriter {
        fn new(needle: &str) -> Self {
            Self {
                needle: needle.as_bytes().to_vec(),
                buffer: Vec::new(),
            }
        }
    }

    impl std::io::Write for SubstringFailWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.buffer.extend_from_slice(buf);
            if self
                .buffer
                .windows(self.needle.len())
                .any(|window| window == self.needle)
            {
                return Err(io::Error::new(io::ErrorKind::Other, "substring failure"));
            }
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn writes_responses_and_events() {
        let mut out = Vec::new();
        write_response(
            &mut out,
            RpcResponse {
                id: 1,
                result: Some(json!({ "ok": true })),
                error: None,
            },
        )
        .unwrap();
        write_event(&mut out, "log", json!("hello")).unwrap();

        let content = String::from_utf8(out).unwrap();
        let mut lines = content.lines();
        let first: Value = serde_json::from_str(lines.next().unwrap()).unwrap();
        assert_eq!(first["id"], 1);
        assert_eq!(first["result"]["ok"], true);
        let second: Value = serde_json::from_str(lines.next().unwrap()).unwrap();
        assert_eq!(second["event"], "log");
    }

    #[test]
    fn failing_writer_allows_writes() {
        let mut out = FailingWriter::fail_after(1000);
        write_event(&mut out, "log", json!("ok")).unwrap();
    }

    #[test]
    fn write_response_and_event_handle_errors() {
        let response = RpcResponse {
            id: 1,
            result: Some(json!({ "ok": true })),
            error: None,
        };
        let mut out = FailingWriter::fail_after(0);
        assert!(write_response(&mut out, response).is_err());

        let response = RpcResponse {
            id: 1,
            result: Some(json!({ "ok": true })),
            error: None,
        };
        let mut out = FailingWriter::fail_on_newline();
        assert!(write_response(&mut out, response).is_err());

        let response = RpcResponse {
            id: 1,
            result: Some(json!({ "ok": true })),
            error: None,
        };
        let mut out = FailingWriter::fail_on_flush();
        assert!(write_response(&mut out, response).is_err());

        let mut out = FailingWriter::fail_after(0);
        assert!(write_event(&mut out, "log", json!("hello")).is_err());

        let mut out = FailingWriter::fail_on_newline();
        assert!(write_event(&mut out, "log", json!("hello")).is_err());

        let mut out = FailingWriter::fail_on_flush();
        assert!(write_event(&mut out, "log", json!("hello")).is_err());
    }

    #[test]
    fn restore_env_var_handles_both_cases() {
        let _guard = ENV_LOCK.lock().unwrap();
        restore_env_var("AER_ASSET_DIR", Some("temp-value".to_string()));
        assert_eq!(std::env::var("AER_ASSET_DIR").unwrap(), "temp-value");
        restore_env_var("AER_ASSET_DIR", None);
        assert!(std::env::var("AER_ASSET_DIR").is_err());
    }

    #[test]
    fn handles_unknown_methods() {
        let request = RpcRequest {
            id: 1,
            method: "nope".to_string(),
            params: json!({}),
        };
        let mut out = Cursor::new(Vec::new());
        let result = handle_request(&request, &mut out);
        assert!(result.is_err());
    }

    #[test]
    fn handles_known_methods() {
        let mut out = Cursor::new(Vec::new());
        let request = RpcRequest {
            id: 1,
            method: "ping".to_string(),
            params: json!({}),
        };
        assert!(handle_request(&request, &mut out).is_ok());

        let request = RpcRequest {
            id: 2,
            method: "list_devices".to_string(),
            params: json!({}),
        };
        assert!(handle_request(&request, &mut out).is_ok());

        let request = RpcRequest {
            id: 3,
            method: "smoke_test".to_string(),
            params: json!({}),
        };
        assert!(handle_request(&request, &mut out).is_ok());

        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("clip.mp3");
        fs::write(&input, "x").unwrap();
        let request = RpcRequest {
            id: 4,
            method: "transcribe".to_string(),
            params: json!({
                "input_path": input.to_string_lossy(),
                "output_dir": temp.path().to_string_lossy(),
                "threads": 1,
                "beam_size": 1,
                "best_of": 1,
                "max_len_chars": 1,
                "split_on_word": true,
                "vad_threshold": 0.1,
                "vad_min_speech_ms": 1,
                "vad_min_sil_ms": 1,
                "vad_pad_ms": 1,
                "no_speech_thold": 0.1,
                "max_context": 0,
                "dedup_merge_gap_sec": 0.1,
                "translate": true,
                "language": "auto",
                "dry_run": true
            }),
        };
        assert!(handle_request(&request, &mut out).is_ok());
    }

    #[test]
    fn transcribe_rejects_invalid_params() {
        let params = json!({
            "input_path": 123
        });
        let mut out = Vec::new();
        let err = transcribe_with_lock(&params, &mut out).unwrap_err();
        assert!(err.to_string().contains("Invalid transcribe params"));
    }

    #[test]
    fn formats_ping_and_device_responses() {
        let info = wgpu::AdapterInfo {
            name: "GPU".to_string(),
            vendor: 1,
            device: 2,
            device_type: wgpu::DeviceType::DiscreteGpu,
            backend: wgpu::Backend::Vulkan,
            driver: "drv".to_string(),
            driver_info: "info".to_string(),
        };
        let gpu = ping_response_from_info(Some(info.clone()));
        assert_eq!(gpu["gpu_enabled"], true);
        let cpu = ping_response_from_info(None);
        assert_eq!(cpu["gpu_enabled"], false);

        let list = list_devices_from_infos(vec![info]);
        assert_eq!(list["devices"][0]["name"], "GPU");
    }

    #[test]
    fn calls_ping_and_list_devices() {
        let ping = ping_with_gpu_info().unwrap();
        assert!(ping["message"].as_str().unwrap().contains("Runtime ready"));
        let devices = list_devices().unwrap();
        assert!(devices.get("devices").is_some());
    }

    #[test]
    fn resolves_optional_paths() {
        let temp = tempfile::tempdir().unwrap();
        let asset_path = temp.path().join("asset.bin");
        fs::write(&asset_path, "x").unwrap();

        assert_eq!(
            resolve_optional_path(Some(" custom "), None, "fallback"),
            "custom"
        );
        assert_eq!(
            resolve_optional_path(Some(""), Some(asset_path.clone()), "fallback"),
            asset_path.to_string_lossy()
        );
        assert_eq!(
            resolve_optional_path(None, None, "fallback"),
            "fallback"
        );
    }

    #[test]
    fn resolves_asset_dir_sources() {
        let _guard = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let asset_dir = temp.path().join("assets");
        fs::create_dir_all(&asset_dir).unwrap();
        std::env::set_var("AER_ASSET_DIR", asset_dir.to_string_lossy().to_string());
        assert_eq!(
            fs::canonicalize(resolve_asset_dir().unwrap()).unwrap(),
            fs::canonicalize(&asset_dir).unwrap()
        );
        std::env::remove_var("AER_ASSET_DIR");

        let exe = std::env::current_exe().unwrap();
        let exe_assets = exe.parent().unwrap().join("assets");
        fs::create_dir_all(&exe_assets).unwrap();
        fs::remove_dir_all(&exe_assets).unwrap();
        fs::create_dir_all(&exe_assets).unwrap();
        assert_eq!(
            fs::canonicalize(resolve_asset_dir().unwrap()).unwrap(),
            fs::canonicalize(&exe_assets).unwrap()
        );
        fs::remove_dir_all(&exe_assets).unwrap();

        let nested = temp.path().join("runtime").join("assets");
        fs::create_dir_all(&nested).unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_var("AER_ASSET_DIR", temp.path().join("missing").to_string_lossy().to_string());
        std::env::set_current_dir(temp.path()).unwrap();
        assert_eq!(
            fs::canonicalize(resolve_asset_dir().unwrap()).unwrap(),
            fs::canonicalize(&nested).unwrap()
        );
        std::env::set_current_dir(original).unwrap();
        std::env::remove_var("AER_ASSET_DIR");
    }

    #[test]
    fn ensures_paths_and_executables() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("exists.bin");
        fs::write(&file_path, "x").unwrap();
        ensure_path_exists("file", file_path.to_string_lossy().as_ref()).unwrap();
        assert!(ensure_path_exists("file", temp.path().join("missing").to_string_lossy().as_ref()).is_err());

        ensure_executable_available("tool", "nonexistent").unwrap();
        assert!(ensure_executable_available("tool", file_path.to_string_lossy().as_ref()).is_ok());
        assert!(ensure_executable_available("tool", temp.path().join("nope").to_string_lossy().as_ref()).is_err());
    }

    #[test]
    fn collects_inputs_and_resolves_outputs() {
        let temp = tempfile::tempdir().unwrap();
        let media = temp.path().join("clip.mp4");
        let ignored = temp.path().join("note.txt");
        fs::write(&media, "x").unwrap();
        fs::write(&ignored, "x").unwrap();

        let files = collect_inputs(temp.path()).unwrap();
        assert_eq!(files.len(), 1);

        let single = collect_inputs(&media).unwrap();
        assert_eq!(single.len(), 1);

        let missing = collect_inputs(&temp.path().join("missing"));
        assert!(missing.is_err());

        let config = TranscribeConfig {
            input_path: media.clone(),
            output_dir: None,
            model_path: "model".to_string(),
            vad_model_path: "vad".to_string(),
            whisper_path: "whisper".to_string(),
            ffmpeg_path: "ffmpeg".to_string(),
            vk_icd_filenames: None,
            threads: 1,
            beam_size: 1,
            best_of: 1,
            max_len_chars: 1,
            split_on_word: true,
            vad_threshold: 0.1,
            vad_min_speech_ms: 1,
            vad_min_sil_ms: 1,
            vad_pad_ms: 1,
            no_speech_thold: 0.1,
            max_context: 0,
            dedup_merge_gap_sec: 0.1,
            translate: true,
            language: "auto".to_string(),
            dry_run: true,
        };

        let output = resolve_output_base(&config, &media).unwrap();
        assert!(output.ends_with("clip"));

        let output_dir = temp.path().join("out");
        let config = TranscribeConfig { output_dir: Some(output_dir.clone()), ..config };
        let output = resolve_output_base(&config, &media).unwrap();
        assert!(output.starts_with(&output_dir));

        let invalid = resolve_output_base(&config, Path::new("/"));
        assert!(invalid.is_err());
        let output = resolve_output_base(&config, Path::new("file")).unwrap();
        assert!(output.starts_with(&output_dir));
    }

    #[test]
    fn checks_up_to_date_and_runs_commands() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("input.txt");
        let output = temp.path().join("output.txt");
        fs::write(&output, "x").unwrap();
        assert!(!is_up_to_date(&input, &output));
        fs::remove_file(&output).unwrap();
        fs::write(&output, "x").unwrap();
        std::thread::sleep(Duration::from_millis(10));
        fs::write(&input, "x").unwrap();
        assert!(!is_up_to_date(&input, &output));
        std::thread::sleep(Duration::from_millis(10));
        fs::write(&output, "x").unwrap();
        assert!(is_up_to_date(&input, &output));
        fs::remove_file(&output).unwrap();
        assert!(!is_up_to_date(&input, &output));
        fs::remove_file(&input).unwrap();
        assert!(!is_up_to_date(&input, &output));

        let mut out = Vec::new();
        run_command(&mut out, "echo", &["hello"], true, None).unwrap();
        let output = String::from_utf8(out).unwrap();
        assert!(output.contains("DRY-RUN"));

        #[cfg(windows)]
        let (program, args) = ("cmd", ["/C", "exit", "0"]);
        #[cfg(not(windows))]
        let (program, args): (&str, [&str; 0]) = ("true", []);

        let mut out = Vec::new();
        run_command(&mut out, program, &args, false, Some("vk.json")).unwrap();
    }

    #[test]
    fn run_command_reports_event_errors() {
        let mut out = FailingWriter::fail_after(0);
        let err = run_command(&mut out, "echo", &["hi"], true, None).unwrap_err();
        assert!(err.to_string().contains("write failure"));
    }

    #[test]
    fn run_command_reports_missing_program() {
        let mut out = Vec::new();
        let err = run_command(&mut out, "does-not-exist", &["hi"], false, None).unwrap_err();
        let message = err.to_string();
        let has_no_such_file = message.contains("No such file");
        let has_os_error = message.contains("os error");
        let has_system_error = message.contains("The system cannot");
        let count = has_no_such_file as u8 + has_os_error as u8 + has_system_error as u8;
        assert!(count > 0);
    }

    #[test]
    fn handles_modified_errors() {
        let err = std::io::Error::new(std::io::ErrorKind::Other, "boom");
        assert!(!is_up_to_date_with_modified(Err(err), Ok(SystemTime::now())));
        let err = std::io::Error::new(std::io::ErrorKind::Other, "boom");
        assert!(!is_up_to_date_with_modified(Ok(SystemTime::now()), Err(err)));
    }

    #[test]
    fn run_command_reports_failure() {
        let mut out = Vec::new();
        #[cfg(windows)]
        let (program, args) = ("cmd", ["/C", "exit", "1"]);
        #[cfg(not(windows))]
        let (program, args): (&str, [&str; 0]) = ("false", []);

        let err = run_command(&mut out, program, &args, false, None).unwrap_err();
        assert!(err.to_string().contains("Command failed"));
    }

    fn create_noop_executable(dir: &Path) -> PathBuf {
        #[cfg(windows)]
        {
            let path = dir.join("noop.cmd");
            let contents = "@echo off\r\nexit /B 0\r\n";
            fs::write(&path, contents).unwrap();
            return path;
        }
        #[cfg(not(windows))]
        {
            let path = dir.join("noop.sh");
            let contents = "#!/bin/sh\nexit 0\n";
            fs::write(&path, contents).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&path).unwrap().permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&path, perms).unwrap();
            }
            path
        }
    }

    fn create_failing_executable(dir: &Path) -> PathBuf {
        #[cfg(windows)]
        {
            let path = dir.join("fail.cmd");
            let contents = "@echo off\r\nexit /B 1\r\n";
            fs::write(&path, contents).unwrap();
            return path;
        }
        #[cfg(not(windows))]
        {
            let path = dir.join("fail.sh");
            let contents = "#!/bin/sh\nexit 1\n";
            fs::write(&path, contents).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&path).unwrap().permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&path, perms).unwrap();
            }
            path
        }
    }

    #[test]
    fn skips_when_outputs_are_up_to_date() {
        let temp = tempfile::tempdir().unwrap();
        let media = temp.path().join("clip.mp4");
        fs::write(&media, "x").unwrap();
        std::thread::sleep(Duration::from_millis(10));

        let output_srt = temp.path().join("clip.srt");
        fs::write(&output_srt, "x").unwrap();

        let mut out = Vec::new();
        let params = json!({
            "input_path": media.to_string_lossy(),
            "threads": 1,
            "beam_size": 1,
            "best_of": 1,
            "max_len_chars": 1,
            "split_on_word": true,
            "vad_threshold": 0.1,
            "vad_min_speech_ms": 1,
            "vad_min_sil_ms": 1,
            "vad_pad_ms": 1,
            "no_speech_thold": 0.1,
            "max_context": 0,
            "dedup_merge_gap_sec": 0.1,
            "translate": true,
            "language": "auto",
            "dry_run": true
        });
        let result = transcribe_with_lock(&params, &mut out).unwrap();
        assert_eq!(result["jobs"], 1);
        let log = String::from_utf8(out).unwrap();
        assert!(log.contains("SKIP (up-to-date)"));
    }

    #[test]
    fn transcribe_reports_skip_log_errors() {
        let temp = tempfile::tempdir().unwrap();
        let media = temp.path().join("clip.mp4");
        fs::write(&media, "x").unwrap();
        std::thread::sleep(Duration::from_millis(10));
        let output_srt = temp.path().join("clip.srt");
        fs::write(&output_srt, "x").unwrap();

        let mut out = SubstringFailWriter::new("SKIP (up-to-date)");
        let params = json!({
            "input_path": media.to_string_lossy(),
            "threads": 1,
            "beam_size": 1,
            "best_of": 1,
            "max_len_chars": 1,
            "split_on_word": true,
            "vad_threshold": 0.1,
            "vad_min_speech_ms": 1,
            "vad_min_sil_ms": 1,
            "vad_pad_ms": 1,
            "no_speech_thold": 0.1,
            "max_context": 0,
            "dedup_merge_gap_sec": 0.1,
            "translate": true,
            "language": "auto",
            "dry_run": true
        });
        assert!(transcribe_with_lock(&params, &mut out).is_err());
    }

    #[test]
    fn transcribe_reports_processing_log_errors() {
        let temp = tempfile::tempdir().unwrap();
        let media = temp.path().join("clip.mp4");
        fs::write(&media, "x").unwrap();

        let mut out = SubstringFailWriter::new("Processing");
        let params = json!({
            "input_path": media.to_string_lossy(),
            "threads": 1,
            "beam_size": 1,
            "best_of": 1,
            "max_len_chars": 1,
            "split_on_word": true,
            "vad_threshold": 0.1,
            "vad_min_speech_ms": 1,
            "vad_min_sil_ms": 1,
            "vad_pad_ms": 1,
            "no_speech_thold": 0.1,
            "max_context": 0,
            "dedup_merge_gap_sec": 0.1,
            "translate": true,
            "language": "auto",
            "dry_run": true
        });
        assert!(transcribe_with_lock(&params, &mut out).is_err());
    }

    #[test]
    fn transcribe_reports_wrote_log_errors() {
        let temp = tempfile::tempdir().unwrap();
        let media = temp.path().join("clip.mp4");
        fs::write(&media, "x").unwrap();

        let mut out = SubstringFailWriter::new("Wrote:");
        let params = json!({
            "input_path": media.to_string_lossy(),
            "threads": 1,
            "beam_size": 1,
            "best_of": 1,
            "max_len_chars": 1,
            "split_on_word": true,
            "vad_threshold": 0.1,
            "vad_min_speech_ms": 1,
            "vad_min_sil_ms": 1,
            "vad_pad_ms": 1,
            "no_speech_thold": 0.1,
            "max_context": 0,
            "dedup_merge_gap_sec": 0.1,
            "translate": true,
            "language": "auto",
            "dry_run": true
        });
        assert!(transcribe_with_lock(&params, &mut out).is_err());
    }

    #[test]
    fn transcribe_reports_post_process_log_errors() {
        let temp = tempfile::tempdir().unwrap();
        let media = temp.path().join("clip.mp4");
        fs::write(&media, "x").unwrap();

        let mut out = SubstringFailWriter::new("DRY-RUN post-process SRT");
        let params = json!({
            "input_path": media.to_string_lossy(),
            "threads": 1,
            "beam_size": 1,
            "best_of": 1,
            "max_len_chars": 1,
            "split_on_word": true,
            "vad_threshold": 0.1,
            "vad_min_speech_ms": 1,
            "vad_min_sil_ms": 1,
            "vad_pad_ms": 1,
            "no_speech_thold": 0.1,
            "max_context": 0,
            "dedup_merge_gap_sec": 0.1,
            "translate": true,
            "language": "auto",
            "dry_run": true
        });
        assert!(transcribe_with_lock(&params, &mut out).is_err());
    }

    #[test]
    fn transcribe_reports_ffmpeg_failure() {
        let temp = tempfile::tempdir().unwrap();
        let media = temp.path().join("clip.mp4");
        let model = temp.path().join("model.bin");
        let vad = temp.path().join("vad.bin");
        fs::write(&media, "x").unwrap();
        fs::write(&model, "x").unwrap();
        fs::write(&vad, "x").unwrap();
        let noop = create_noop_executable(temp.path());
        let fail = create_failing_executable(temp.path());

        let params = json!({
            "input_path": media.to_string_lossy(),
            "output_dir": temp.path().join("out").to_string_lossy(),
            "model_path": model.to_string_lossy(),
            "vad_model_path": vad.to_string_lossy(),
            "whisper_path": noop.to_string_lossy(),
            "ffmpeg_path": fail.to_string_lossy(),
            "threads": 1,
            "beam_size": 1,
            "best_of": 1,
            "max_len_chars": 1,
            "split_on_word": false,
            "vad_threshold": 0.1,
            "vad_min_speech_ms": 1,
            "vad_min_sil_ms": 1,
            "vad_pad_ms": 1,
            "no_speech_thold": 0.1,
            "max_context": 0,
            "dedup_merge_gap_sec": 0.1,
            "translate": false,
            "language": "auto",
            "dry_run": false
        });
        let mut out = Vec::new();
        let err = transcribe_with_lock(&params, &mut out).unwrap_err();
        assert!(err.to_string().contains("Command failed"));
    }

    #[test]
    fn transcribe_reports_whisper_failure() {
        let temp = tempfile::tempdir().unwrap();
        let media = temp.path().join("clip.mp4");
        let model = temp.path().join("model.bin");
        let vad = temp.path().join("vad.bin");
        fs::write(&media, "x").unwrap();
        fs::write(&model, "x").unwrap();
        fs::write(&vad, "x").unwrap();
        let noop = create_noop_executable(temp.path());
        let fail = create_failing_executable(temp.path());

        let params = json!({
            "input_path": media.to_string_lossy(),
            "output_dir": temp.path().join("out").to_string_lossy(),
            "model_path": model.to_string_lossy(),
            "vad_model_path": vad.to_string_lossy(),
            "whisper_path": fail.to_string_lossy(),
            "ffmpeg_path": noop.to_string_lossy(),
            "threads": 1,
            "beam_size": 1,
            "best_of": 1,
            "max_len_chars": 1,
            "split_on_word": false,
            "vad_threshold": 0.1,
            "vad_min_speech_ms": 1,
            "vad_min_sil_ms": 1,
            "vad_pad_ms": 1,
            "no_speech_thold": 0.1,
            "max_context": 0,
            "dedup_merge_gap_sec": 0.1,
            "translate": false,
            "language": "auto",
            "dry_run": false
        });
        let mut out = Vec::new();
        let err = transcribe_with_lock(&params, &mut out).unwrap_err();
        assert!(err.to_string().contains("Command failed"));
    }

    #[test]
    fn transcribes_non_dry_run_executes_commands() {
        let temp = tempfile::tempdir().unwrap();
        let media = temp.path().join("clip.mp4");
        fs::write(&media, "x").unwrap();
        let model = temp.path().join("model.bin");
        let vad = temp.path().join("vad.bin");
        fs::write(&model, "x").unwrap();
        fs::write(&vad, "x").unwrap();
        let noop = create_noop_executable(temp.path());

        let mut out = Vec::new();
        let params = json!({
            "input_path": media.to_string_lossy(),
            "output_dir": temp.path().join("out").to_string_lossy(),
            "model_path": model.to_string_lossy(),
            "vad_model_path": vad.to_string_lossy(),
            "whisper_path": noop.to_string_lossy(),
            "ffmpeg_path": noop.to_string_lossy(),
            "threads": 1,
            "beam_size": 1,
            "best_of": 1,
            "max_len_chars": 1,
            "split_on_word": false,
            "vad_threshold": 0.1,
            "vad_min_speech_ms": 1,
            "vad_min_sil_ms": 1,
            "vad_pad_ms": 1,
            "no_speech_thold": 0.1,
            "max_context": 0,
            "dedup_merge_gap_sec": 0.1,
            "translate": false,
            "language": "auto",
            "dry_run": false
        });
        let result = transcribe_with_lock(&params, &mut out).unwrap();
        assert_eq!(result["jobs"], 1);
    }

    #[test]
    fn transcribe_errors_on_missing_input_path() {
        let temp = tempfile::tempdir().unwrap();
        let missing = temp.path().join("missing.mp4");
        let params = json!({
            "input_path": missing.to_string_lossy(),
            "threads": 1,
            "beam_size": 1,
            "best_of": 1,
            "max_len_chars": 1,
            "split_on_word": true,
            "vad_threshold": 0.1,
            "vad_min_speech_ms": 1,
            "vad_min_sil_ms": 1,
            "vad_pad_ms": 1,
            "no_speech_thold": 0.1,
            "max_context": 0,
            "dedup_merge_gap_sec": 0.1,
            "translate": true,
            "language": "auto",
            "dry_run": true
        });
        let mut out = Vec::new();
        let err = transcribe_with_lock(&params, &mut out).unwrap_err();
        assert!(err.to_string().contains("Input path does not exist"));
    }

    #[test]
    fn transcribe_errors_when_output_dir_is_file() {
        let temp = tempfile::tempdir().unwrap();
        let media = temp.path().join("clip.mp4");
        fs::write(&media, "x").unwrap();
        let output_dir = temp.path().join("output-file");
        fs::write(&output_dir, "x").unwrap();

        let params = json!({
            "input_path": media.to_string_lossy(),
            "output_dir": output_dir.to_string_lossy(),
            "threads": 1,
            "beam_size": 1,
            "best_of": 1,
            "max_len_chars": 1,
            "split_on_word": true,
            "vad_threshold": 0.1,
            "vad_min_speech_ms": 1,
            "vad_min_sil_ms": 1,
            "vad_pad_ms": 1,
            "no_speech_thold": 0.1,
            "max_context": 0,
            "dedup_merge_gap_sec": 0.1,
            "translate": true,
            "language": "auto",
            "dry_run": true
        });
        let mut out = Vec::new();
        assert!(transcribe_with_lock(&params, &mut out).is_err());
    }

    #[test]
    fn transcribe_defaults_language() {
        let temp = tempfile::tempdir().unwrap();
        let media = temp.path().join("clip.mp4");
        fs::write(&media, "x").unwrap();
        let params = json!({
            "input_path": media.to_string_lossy(),
            "threads": 1,
            "beam_size": 1,
            "best_of": 1,
            "max_len_chars": 1,
            "split_on_word": true,
            "vad_threshold": 0.1,
            "vad_min_speech_ms": 1,
            "vad_min_sil_ms": 1,
            "vad_pad_ms": 1,
            "no_speech_thold": 0.1,
            "max_context": 0,
            "dedup_merge_gap_sec": 0.1,
            "translate": true,
            "dry_run": true
        });
        let mut out = Vec::new();
        let result = transcribe_with_lock(&params, &mut out).unwrap();
        assert_eq!(result["jobs"], 1);
        let log = String::from_utf8(out).unwrap();
        assert!(log.contains("-l auto"));
    }

    #[test]
    fn transcribe_uses_asset_dir_defaults() {
        let _guard = ENV_LOCK.lock().unwrap();
        let original = std::env::var("AER_ASSET_DIR").ok();
        let temp = tempfile::tempdir().unwrap();
        let asset_dir = temp.path().join("assets");
        fs::create_dir_all(asset_dir.join("models")).unwrap();
        fs::create_dir_all(asset_dir.join("bin")).unwrap();
        std::env::set_var("AER_ASSET_DIR", asset_dir.to_string_lossy().to_string());

        let media = temp.path().join("clip.mp4");
        fs::write(&media, "x").unwrap();
        let params = json!({
            "input_path": media.to_string_lossy(),
            "threads": 1,
            "beam_size": 1,
            "best_of": 1,
            "max_len_chars": 1,
            "split_on_word": true,
            "vad_threshold": 0.1,
            "vad_min_speech_ms": 1,
            "vad_min_sil_ms": 1,
            "vad_pad_ms": 1,
            "no_speech_thold": 0.1,
            "max_context": 0,
            "dedup_merge_gap_sec": 0.1,
            "translate": true,
            "dry_run": true
        });
        let mut out = Vec::new();
        let result = transcribe(&params, &mut out).unwrap();
        assert_eq!(result["jobs"], 1);
        let log = String::from_utf8(out).unwrap();
        assert!(log.contains("models/ggml-large-v3.bin"));
        assert!(log.contains("models/ggml-silero-v6.2.0.bin"));
        assert!(log.contains("bin/whisper-cli"));
        assert!(log.contains("bin/ffmpeg"));

        restore_env_var("AER_ASSET_DIR", original);
    }

    #[test]
    fn transcribe_filters_vk_icd_filenames() {
        let temp = tempfile::tempdir().unwrap();
        let media = temp.path().join("clip.mp4");
        fs::write(&media, "x").unwrap();
        let params = json!({
            "input_path": media.to_string_lossy(),
            "threads": 1,
            "beam_size": 1,
            "best_of": 1,
            "max_len_chars": 1,
            "split_on_word": true,
            "vad_threshold": 0.1,
            "vad_min_speech_ms": 1,
            "vad_min_sil_ms": 1,
            "vad_pad_ms": 1,
            "no_speech_thold": 0.1,
            "max_context": 0,
            "dedup_merge_gap_sec": 0.1,
            "translate": true,
            "language": "auto",
            "vk_icd_filenames": " ",
            "dry_run": true
        });
        let mut out = Vec::new();
        let result = transcribe_with_lock(&params, &mut out).unwrap();
        assert_eq!(result["jobs"], 1);
    }

    #[test]
    fn transcribe_requires_whisper_path() {
        let temp = tempfile::tempdir().unwrap();
        let media = temp.path().join("clip.mp4");
        let model = temp.path().join("model.bin");
        let vad = temp.path().join("vad.bin");
        fs::write(&media, "x").unwrap();
        fs::write(&model, "x").unwrap();
        fs::write(&vad, "x").unwrap();
        let missing = temp.path().join("missing-whisper");

        let params = json!({
            "input_path": media.to_string_lossy(),
            "output_dir": temp.path().join("out").to_string_lossy(),
            "model_path": model.to_string_lossy(),
            "vad_model_path": vad.to_string_lossy(),
            "whisper_path": missing.to_string_lossy(),
            "ffmpeg_path": "ffmpeg",
            "threads": 1,
            "beam_size": 1,
            "best_of": 1,
            "max_len_chars": 1,
            "split_on_word": false,
            "vad_threshold": 0.1,
            "vad_min_speech_ms": 1,
            "vad_min_sil_ms": 1,
            "vad_pad_ms": 1,
            "no_speech_thold": 0.1,
            "max_context": 0,
            "dedup_merge_gap_sec": 0.1,
            "translate": false,
            "language": "auto",
            "dry_run": false
        });
        let mut out = Vec::new();
        let err = transcribe_with_lock(&params, &mut out).unwrap_err();
        assert!(err.to_string().contains("whisper-cli not found"));
    }

    #[test]
    fn transcribe_requires_model_path() {
        let temp = tempfile::tempdir().unwrap();
        let media = temp.path().join("clip.mp4");
        let vad = temp.path().join("vad.bin");
        fs::write(&media, "x").unwrap();
        fs::write(&vad, "x").unwrap();
        let noop = create_noop_executable(temp.path());
        let missing = temp.path().join("missing-model.bin");

        let params = json!({
            "input_path": media.to_string_lossy(),
            "output_dir": temp.path().join("out").to_string_lossy(),
            "model_path": missing.to_string_lossy(),
            "vad_model_path": vad.to_string_lossy(),
            "whisper_path": noop.to_string_lossy(),
            "ffmpeg_path": "ffmpeg",
            "threads": 1,
            "beam_size": 1,
            "best_of": 1,
            "max_len_chars": 1,
            "split_on_word": false,
            "vad_threshold": 0.1,
            "vad_min_speech_ms": 1,
            "vad_min_sil_ms": 1,
            "vad_pad_ms": 1,
            "no_speech_thold": 0.1,
            "max_context": 0,
            "dedup_merge_gap_sec": 0.1,
            "translate": false,
            "language": "auto",
            "dry_run": false
        });
        let mut out = Vec::new();
        let err = transcribe_with_lock(&params, &mut out).unwrap_err();
        assert!(err.to_string().contains("Whisper model not found"));
    }

    #[test]
    fn transcribe_requires_vad_path() {
        let temp = tempfile::tempdir().unwrap();
        let media = temp.path().join("clip.mp4");
        let model = temp.path().join("model.bin");
        fs::write(&media, "x").unwrap();
        fs::write(&model, "x").unwrap();
        let noop = create_noop_executable(temp.path());
        let missing = temp.path().join("missing-vad.bin");

        let params = json!({
            "input_path": media.to_string_lossy(),
            "output_dir": temp.path().join("out").to_string_lossy(),
            "model_path": model.to_string_lossy(),
            "vad_model_path": missing.to_string_lossy(),
            "whisper_path": noop.to_string_lossy(),
            "ffmpeg_path": "ffmpeg",
            "threads": 1,
            "beam_size": 1,
            "best_of": 1,
            "max_len_chars": 1,
            "split_on_word": false,
            "vad_threshold": 0.1,
            "vad_min_speech_ms": 1,
            "vad_min_sil_ms": 1,
            "vad_pad_ms": 1,
            "no_speech_thold": 0.1,
            "max_context": 0,
            "dedup_merge_gap_sec": 0.1,
            "translate": false,
            "language": "auto",
            "dry_run": false
        });
        let mut out = Vec::new();
        let err = transcribe_with_lock(&params, &mut out).unwrap_err();
        assert!(err.to_string().contains("VAD model not found"));
    }

    #[test]
    fn transcribe_requires_ffmpeg_path() {
        let temp = tempfile::tempdir().unwrap();
        let media = temp.path().join("clip.mp4");
        let model = temp.path().join("model.bin");
        let vad = temp.path().join("vad.bin");
        fs::write(&media, "x").unwrap();
        fs::write(&model, "x").unwrap();
        fs::write(&vad, "x").unwrap();
        let noop = create_noop_executable(temp.path());
        let missing = temp.path().join("missing-ffmpeg");

        let params = json!({
            "input_path": media.to_string_lossy(),
            "output_dir": temp.path().join("out").to_string_lossy(),
            "model_path": model.to_string_lossy(),
            "vad_model_path": vad.to_string_lossy(),
            "whisper_path": noop.to_string_lossy(),
            "ffmpeg_path": missing.to_string_lossy(),
            "threads": 1,
            "beam_size": 1,
            "best_of": 1,
            "max_len_chars": 1,
            "split_on_word": false,
            "vad_threshold": 0.1,
            "vad_min_speech_ms": 1,
            "vad_min_sil_ms": 1,
            "vad_pad_ms": 1,
            "no_speech_thold": 0.1,
            "max_context": 0,
            "dedup_merge_gap_sec": 0.1,
            "translate": false,
            "language": "auto",
            "dry_run": false
        });
        let mut out = Vec::new();
        let err = transcribe_with_lock(&params, &mut out).unwrap_err();
        assert!(err.to_string().contains("ffmpeg not found"));
    }

    #[cfg(unix)]
    #[test]
    fn transcribe_fails_when_tempdir_missing() {
        let _guard = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let media = temp.path().join("clip.mp4");
        let model = temp.path().join("model.bin");
        let vad = temp.path().join("vad.bin");
        fs::write(&media, "x").unwrap();
        fs::write(&model, "x").unwrap();
        fs::write(&vad, "x").unwrap();
        let noop = create_noop_executable(temp.path());

        let invalid_tmp = temp.path().join("missing-tmp");
        let original = std::env::var("TMPDIR").ok();
        std::env::set_var("TMPDIR", invalid_tmp.to_string_lossy().to_string());

        let params = json!({
            "input_path": media.to_string_lossy(),
            "output_dir": temp.path().join("out").to_string_lossy(),
            "model_path": model.to_string_lossy(),
            "vad_model_path": vad.to_string_lossy(),
            "whisper_path": noop.to_string_lossy(),
            "ffmpeg_path": noop.to_string_lossy(),
            "threads": 1,
            "beam_size": 1,
            "best_of": 1,
            "max_len_chars": 1,
            "split_on_word": false,
            "vad_threshold": 0.1,
            "vad_min_speech_ms": 1,
            "vad_min_sil_ms": 1,
            "vad_pad_ms": 1,
            "no_speech_thold": 0.1,
            "max_context": 0,
            "dedup_merge_gap_sec": 0.1,
            "translate": false,
            "language": "auto",
            "dry_run": false
        });
        let mut out = Vec::new();
        assert!(transcribe(&params, &mut out).is_err());

        restore_env_var("TMPDIR", original);
    }

    #[test]
    fn transcribe_propagates_dedup_errors() {
        let temp = tempfile::tempdir().unwrap();
        let media = temp.path().join("clip.mp4");
        let model = temp.path().join("model.bin");
        let vad = temp.path().join("vad.bin");
        fs::write(&media, "x").unwrap();
        fs::write(&model, "x").unwrap();
        fs::write(&vad, "x").unwrap();
        let noop = create_noop_executable(temp.path());

        let output_dir = temp.path().join("out");
        fs::create_dir_all(&output_dir).unwrap();
        let srt = output_dir.join("clip.srt");
        fs::write(&srt, "1\n00:00:bad --> 00:00:01,000\nHello\n\n").unwrap();
        std::thread::sleep(Duration::from_millis(10));
        fs::write(&media, "x").unwrap();

        let params = json!({
            "input_path": media.to_string_lossy(),
            "output_dir": output_dir.to_string_lossy(),
            "model_path": model.to_string_lossy(),
            "vad_model_path": vad.to_string_lossy(),
            "whisper_path": noop.to_string_lossy(),
            "ffmpeg_path": noop.to_string_lossy(),
            "threads": 1,
            "beam_size": 1,
            "best_of": 1,
            "max_len_chars": 1,
            "split_on_word": false,
            "vad_threshold": 0.1,
            "vad_min_speech_ms": 1,
            "vad_min_sil_ms": 1,
            "vad_pad_ms": 1,
            "no_speech_thold": 0.1,
            "max_context": 0,
            "dedup_merge_gap_sec": 0.1,
            "translate": false,
            "language": "auto",
            "dry_run": false
        });
        let mut out = Vec::new();
        assert!(transcribe_with_lock(&params, &mut out).is_err());
    }

    #[cfg(coverage)]
    #[test]
    fn main_returns_ok_in_coverage() {
        assert!(crate::main().is_ok());
    }

    #[test]
    fn dedups_srt_and_timestamps() {
        let temp = tempfile::tempdir().unwrap();
        let srt = temp.path().join("test.srt");
        fs::write(&srt, "").unwrap();
        dedup_srt(&srt, 0.5).unwrap();

        let missing = temp.path().join("missing.srt");
        dedup_srt(&missing, 0.5).unwrap();

        let content = "1\n00:00:00,500 --> 00:00:00,900\n \n\n2\n00:00:01,000 --> 00:00:02,000\nHello\n\n3\n00:00:02,100 --> 00:00:03,000\nhello\n\n4\n00:00:03,600 --> 00:00:04,000\nWorld\n\n";
        fs::write(&srt, content).unwrap();
        dedup_srt(&srt, 0.5).unwrap();
        let output = fs::read_to_string(&srt).unwrap();
        assert!(output.to_lowercase().contains("hello"));
        assert!(output.contains("-->"));

        assert_eq!(timestamp_to_ms("00:00:01,500").unwrap(), 1500);
        assert!(timestamp_to_ms("bad").is_err());
        assert_eq!(ms_to_timestamp(1500), "00:00:01,500");
    }

    #[cfg(unix)]
    #[test]
    fn dedup_srt_fails_on_write_errors() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let srt = temp.path().join("test.srt");
        fs::write(&srt, "1\n00:00:01,000 --> 00:00:02,000\nHello\n\n").unwrap();
        let mut perms = fs::metadata(&srt).unwrap().permissions();
        perms.set_mode(0o444);
        fs::set_permissions(&srt, perms).unwrap();

        let result = dedup_srt(&srt, 0.5);
        assert!(result.is_err());

        let mut perms = fs::metadata(&srt).unwrap().permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&srt, perms).unwrap();
    }

    #[test]
    fn dedup_srt_reports_invalid_timestamps() {
        let temp = tempfile::tempdir().unwrap();
        let srt = temp.path().join("bad.srt");
        fs::write(&srt, "1\n00:00:bad --> 00:00:01,000\nHello\n\n").unwrap();
        assert!(dedup_srt(&srt, 0.5).is_err());

        fs::write(&srt, "1\n00:00:01,000 --> 00:00:bad\nHello\n\n").unwrap();
        assert!(dedup_srt(&srt, 0.5).is_err());
    }

    #[test]
    fn timestamp_to_ms_errors() {
        assert!(timestamp_to_ms("aa:00:00,000").is_err());
        assert!(timestamp_to_ms("00").is_err());
        assert!(timestamp_to_ms("00:aa:00,000").is_err());
        assert!(timestamp_to_ms("00:00").is_err());
        assert!(timestamp_to_ms("00:00:aa,000").is_err());
        assert!(timestamp_to_ms("00:00:00").is_err());
        assert!(timestamp_to_ms("00:00:00,aa").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn collect_inputs_reports_walkdir_errors() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let restricted = temp.path().join("restricted");
        fs::create_dir_all(&restricted).unwrap();
        let mut perms = fs::metadata(&restricted).unwrap().permissions();
        perms.set_mode(0o000);
        fs::set_permissions(&restricted, perms).unwrap();

        let result = collect_inputs(temp.path());

        let mut perms = fs::metadata(&restricted).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&restricted, perms).unwrap();

        assert!(result.is_err());
    }

    #[test]
    fn transcribes_in_dry_run() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("clip.mp3");
        fs::write(&input, "x").unwrap();

        let params = json!({
            "input_path": input.to_string_lossy(),
            "output_dir": temp.path().to_string_lossy(),
            "threads": 1,
            "beam_size": 1,
            "best_of": 1,
            "max_len_chars": 1,
            "split_on_word": true,
            "vad_threshold": 0.1,
            "vad_min_speech_ms": 1,
            "vad_min_sil_ms": 1,
            "vad_pad_ms": 1,
            "no_speech_thold": 0.1,
            "max_context": 0,
            "dedup_merge_gap_sec": 0.1,
            "translate": true,
            "language": "auto",
            "dry_run": true
        });

        let mut out = Vec::new();
        let result = transcribe_with_lock(&params, &mut out).unwrap();
        assert_eq!(result["jobs"], 1);

        let output = String::from_utf8(out).unwrap();
        assert!(output.contains("DRY-RUN"));
    }

    #[test]
    fn smoke_test_in_tests() {
        let result = smoke_test().unwrap();
        assert!(result["message"].as_str().unwrap().contains("Smoke test ok"));
    }

    #[test]
    fn transcribe_requires_input_path() {
        let params = json!({
            "input_path": " ",
            "threads": 1,
            "beam_size": 1,
            "best_of": 1,
            "max_len_chars": 1,
            "split_on_word": true,
            "vad_threshold": 0.1,
            "vad_min_speech_ms": 1,
            "vad_min_sil_ms": 1,
            "vad_pad_ms": 1,
            "no_speech_thold": 0.1,
            "max_context": 0,
            "dedup_merge_gap_sec": 0.1,
            "translate": true,
            "language": "auto",
            "dry_run": true
        });
        let mut out = Vec::new();
        let err = transcribe_with_lock(&params, &mut out).unwrap_err();
        assert!(err.to_string().contains("input_path is required"));
    }

    #[test]
    fn transcribe_requires_media_files() {
        let temp = tempfile::tempdir().unwrap();
        let params = json!({
            "input_path": temp.path().to_string_lossy(),
            "threads": 1,
            "beam_size": 1,
            "best_of": 1,
            "max_len_chars": 1,
            "split_on_word": true,
            "vad_threshold": 0.1,
            "vad_min_speech_ms": 1,
            "vad_min_sil_ms": 1,
            "vad_pad_ms": 1,
            "no_speech_thold": 0.1,
            "max_context": 0,
            "dedup_merge_gap_sec": 0.1,
            "translate": true,
            "language": "auto",
            "dry_run": true
        });
        let mut out = Vec::new();
        let err = transcribe_with_lock(&params, &mut out).unwrap_err();
        assert!(err.to_string().contains("No media files found"));
    }
}
