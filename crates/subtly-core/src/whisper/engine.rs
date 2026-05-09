//! Whisper inference. Loads a `WhisperContext`, runs `state.full(...)` on
//! pre-conditioned 16 kHz mono samples, and yields finalized `Segment`s
//! through both a return value and live `Event::Segment` notifications.

use super::params::{flash_attention_is_safe, WhisperConfig};
use crate::audio::TARGET_SAMPLE_RATE;
use crate::events::{DetectedLanguage, Event, ProgressUpdate, SegmentEvent};
use crate::output::{Segment, Word};
use anyhow::{anyhow, Context, Result};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, watch};
use whisper_rs::{
    FullParams, SamplingStrategy, SegmentCallbackData, WhisperContext, WhisperContextParameters,
    WhisperVadParams,
};

/// Run inference on `samples` (mono, 16 kHz, f32) using the model at
/// `config.model_path`. Emits per-segment + per-progress events as the work
/// proceeds and respects the cancel watch by polling between segments and
/// via whisper-rs's abort callback.
pub async fn transcribe_samples(
    samples: Vec<f32>,
    config: WhisperConfig,
    events: mpsc::Sender<Event>,
    cancel: watch::Receiver<bool>,
) -> Result<Vec<Segment>> {
    if samples.is_empty() {
        return Err(anyhow!("no audio samples to transcribe"));
    }
    tokio::task::spawn_blocking(move || run_blocking(samples, config, events, cancel))
        .await
        .context("whisper task panicked")?
}

fn run_blocking(
    samples: Vec<f32>,
    config: WhisperConfig,
    events: mpsc::Sender<Event>,
    cancel: watch::Receiver<bool>,
) -> Result<Vec<Segment>> {
    let mut ctx_params = WhisperContextParameters::default();
    ctx_params.use_gpu = true;
    ctx_params.flash_attn = config.flash_attn && flash_attention_is_safe();

    let ctx = WhisperContext::new_with_params(&config.model_path, ctx_params)
        .with_context(|| format!("loading whisper model {}", config.model_path))?;
    let mut state = ctx.create_state().context("creating whisper state")?;

    let mut params = FullParams::new(SamplingStrategy::BeamSearch {
        beam_size: config.beam_size as i32,
        patience: 1.0,
    });
    params.set_n_threads(config.threads as i32);
    params.set_translate(config.translate);
    params.set_language(Some(&config.language));
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_special(false);
    params.set_print_timestamps(false);
    params.set_no_context(true);
    params.set_suppress_blank(true);
    params.set_suppress_nst(true);
    params.set_no_speech_thold(config.no_speech_thold);
    params.set_max_len(config.max_len_chars as i32);
    params.set_split_on_word(config.split_on_word);
    params.set_token_timestamps(config.token_timestamps);
    if config.max_context > 0 {
        params.set_n_max_text_ctx(config.max_context as i32);
    }
    // Initial prompt biases the decoder toward user-supplied vocabulary
    // (names, brands, jargon). whisper-rs holds a borrow into the &str for
    // the lifetime of `params`, so the source string must outlive the
    // `state.full(...)` call below — keep `prompt_owned` in scope until
    // after inference returns.
    let prompt_trimmed = config.initial_prompt.trim();
    let prompt_owned = if prompt_trimmed.is_empty() {
        None
    } else {
        Some(prompt_trimmed.to_string())
    };
    if let Some(p) = prompt_owned.as_deref() {
        params.set_initial_prompt(p);
    }

    if !config.vad_model_path.is_empty() && std::path::Path::new(&config.vad_model_path).exists() {
        params.set_vad_model_path(Some(&config.vad_model_path));
        let mut vad = WhisperVadParams::new();
        vad.set_threshold(config.vad_threshold);
        vad.set_min_speech_duration(config.vad_min_speech_ms as i32);
        vad.set_min_silence_duration(config.vad_min_sil_ms as i32);
        vad.set_speech_pad(config.vad_pad_ms as i32);
        params.set_vad_params(vad);
        params.enable_vad(true);
    }

    let total_ms = ((samples.len() as i64) * 1000 / TARGET_SAMPLE_RATE as i64).max(1);

    // Drive smooth progress + streaming segments off the per-segment callback —
    // whisper.cpp's progress callback only fires a handful of times per file
    // (e.g. 0 / 45 / 93 / 100 % for a 60 s clip), which makes the bar look stuck.
    let segment_events = events.clone();
    params.set_segment_callback_safe(move |data: SegmentCallbackData| {
        let start_ms = data.start_timestamp * 10;
        let end_ms = data.end_timestamp * 10;
        let trimmed = data.text.trim().to_string();
        if !trimmed.is_empty() {
            let _ = segment_events.try_send(Event::Segment(SegmentEvent {
                start_ms,
                end_ms,
                text: trimmed,
            }));
        }
        let pct = (end_ms.saturating_mul(100) / total_ms).clamp(0, 100) as u8;
        let _ = segment_events.try_send(Event::Progress(ProgressUpdate {
            progress: Some(pct),
            phase: Some("Transcribing".to_string()),
            ..Default::default()
        }));
    });

    // Bridge `watch::Receiver<bool>` → `Arc<AtomicBool>` so the FFI abort
    // callback can read it from C without going through tokio. We avoid
    // `set_abort_callback_safe` because whisper-rs 0.16 monomorphizes its
    // trampoline as `trampoline::<F>` while the user_data it stores is a
    // `*mut Box<dyn FnMut() -> bool>` — different layouts, so the callback
    // returns garbage in practice and never aborts. (The repo is archived,
    // so no upstream fix is coming.)
    let abort_flag = Arc::new(AtomicBool::new(*cancel.borrow()));
    let abort_for_thread = abort_flag.clone();
    let cancel_for_thread = cancel.clone();
    let stop_watcher = Arc::new(AtomicBool::new(false));
    let stop_for_thread = stop_watcher.clone();
    let watcher = std::thread::spawn(move || loop {
        if stop_for_thread.load(Ordering::Relaxed) {
            break;
        }
        if *cancel_for_thread.borrow() {
            abort_for_thread.store(true, Ordering::Relaxed);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    });

    let abort_raw = Arc::into_raw(abort_flag.clone()) as *mut c_void;
    unsafe {
        params.set_abort_callback(Some(abort_trampoline));
        params.set_abort_callback_user_data(abort_raw);
    }

    let result = state.full(params, &samples);

    // Drop the FFI Arc clone we leaked above. Whisper has already returned, so
    // it's safe to release this reference.
    unsafe { Arc::decrement_strong_count(abort_raw as *const AtomicBool) };

    stop_watcher.store(true, Ordering::Relaxed);
    let _ = watcher.join();

    result.context("whisper inference")?;

    if abort_flag.load(Ordering::Relaxed) {
        return Err(anyhow!("Transcription cancelled"));
    }

    let n = state.full_n_segments();
    let mut out = Vec::with_capacity(n.max(0) as usize);
    for i in 0..n {
        let Some(seg) = state.get_segment(i) else {
            continue;
        };
        let start_ms = seg.start_timestamp() * 10;
        let end_ms = seg.end_timestamp() * 10;
        let text = match seg.to_str_lossy() {
            Ok(s) => s.into_owned(),
            Err(_) => continue,
        };
        let trimmed = text.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        let words = if config.token_timestamps {
            collect_words(&seg)
        } else {
            Vec::new()
        };
        out.push(Segment {
            start_ms,
            end_ms,
            text: trimmed,
            words,
        });
    }

    // Surface the detected language (only meaningful for `auto`; for an
    // explicit language code whisper.cpp echoes that code back). UI uses
    // this to confirm Whisper picked the language the user expected.
    let lang_id = state.full_lang_id_from_state();
    if lang_id >= 0 {
        if let Some(code) = whisper_rs::get_lang_str(lang_id) {
            let name = whisper_rs::get_lang_str_full(lang_id)
                .unwrap_or(code)
                .to_string();
            let _ = events.try_send(Event::DetectedLanguage(DetectedLanguage {
                code: code.to_string(),
                name,
            }));
        }
    }

    let _ = events.try_send(Event::Progress(ProgressUpdate {
        progress: Some(100),
        phase: Some("Transcribing".to_string()),
        ..Default::default()
    }));
    drop(prompt_owned);
    Ok(out)
}

/// Stitch sub-word tokens into whitespace-delimited words with timing
/// taken from the first/last token of each word.
///
/// Whisper emits special tokens (e.g. `[_BEG_]`, `[_TT_*]`, `<|...|>`) which
/// we drop, plus regular text tokens. By convention a leading-space token
/// marks the start of a new word; tokens without a leading space are stuck
/// to the previous one (BPE sub-word pieces).
fn collect_words(seg: &whisper_rs::WhisperSegment<'_>) -> Vec<Word> {
    let n = seg.n_tokens();
    let mut words: Vec<Word> = Vec::new();
    let mut buf = String::new();
    let mut buf_start: Option<i64> = None;
    let mut buf_end: i64 = 0;
    for j in 0..n {
        let Some(tok) = seg.get_token(j) else {
            continue;
        };
        let text = match tok.to_str_lossy() {
            Ok(s) => s.into_owned(),
            Err(_) => continue,
        };
        // Drop whisper special tokens — they appear inside `<|...|>` brackets
        // and carry no transcribed audio.
        if text.starts_with('[') || text.starts_with('<') {
            continue;
        }
        let data = tok.token_data();
        let starts_word = text.starts_with(' ') || text.starts_with('\n');
        if starts_word && !buf.is_empty() {
            if let Some(s) = buf_start.take() {
                let trimmed = buf.trim().to_string();
                if !trimmed.is_empty() {
                    words.push(Word {
                        start_ms: s * 10,
                        end_ms: buf_end * 10,
                        text: trimmed,
                    });
                }
            }
            buf.clear();
        }
        if buf_start.is_none() {
            // Token timestamps are in centiseconds (matching whisper.cpp's
            // segment timestamps). Convert to ms when emitting.
            buf_start = Some(data.t0.max(0));
        }
        buf.push_str(&text);
        buf_end = data.t1.max(buf_end);
    }
    if !buf.trim().is_empty() {
        if let Some(s) = buf_start {
            words.push(Word {
                start_ms: s * 10,
                end_ms: buf_end * 10,
                text: buf.trim().to_string(),
            });
        }
    }
    words
}

/// FFI abort callback. The user_data pointer is an `Arc<AtomicBool>` raw
/// pointer (we leak one strong reference for the lifetime of `state.full`,
/// then `Arc::decrement_strong_count` it after the call returns).
unsafe extern "C" fn abort_trampoline(user_data: *mut c_void) -> bool {
    if user_data.is_null() {
        return false;
    }
    let flag = &*(user_data as *const AtomicBool);
    flag.load(Ordering::Relaxed)
}
