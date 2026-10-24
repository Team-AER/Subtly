//! Subtly app state, Message enum, update/view, subscription.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use iced::widget::{button, column, container, row, scrollable, text, text_editor, Space};
use iced::{Alignment, Element, Length, Subscription, Task, Theme};
use tokio::sync::{mpsc, watch};

use subtly_core::devices::{list_devices, ping, smoke_test, DeviceInfo, PingResult};
use subtly_core::download::{delete_model, download_model, model_path_if_present};
use subtly_core::events::{DetectedLanguage, Event};
use subtly_core::models::{installed_models, InstalledModel};
use subtly_core::output::replace::ReplaceRule;
use subtly_core::paths::models_directory;
use subtly_core::settings::{load_settings, save_settings, Settings};
use subtly_core::transcribe::{transcribe, TranscribeOutcome};

use crate::screens::{advanced, models as models_screen, workspace};
use crate::theme as t;

const MAX_LOG_ENTRIES: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Workspace,
    Models,
    Advanced,
    Logs,
}

impl Screen {
    pub fn label(self) -> &'static str {
        match self {
            Screen::Workspace => "Workspace",
            Screen::Models => "Models",
            Screen::Advanced => "Advanced",
            Screen::Logs => "Activity",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    #[allow(dead_code)]
    pub id: u64,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct DownloadInfo {
    pub progress: u8,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub cancelling: bool,
}

#[derive(Debug, Clone)]
pub struct TranscribeProgress {
    pub progress: u8,
    pub status_message: String,
    pub current_item: Option<u64>,
    pub total_items: Option<u64>,
    pub cancelling: bool,
}

#[derive(Debug, Clone)]
pub enum TranscribeResult {
    Completed {
        jobs: usize,
        outputs: Vec<String>,
        dry_run: bool,
    },
    Failed(String),
    Cancelled,
}

pub struct App {
    pub screen: Screen,
    pub settings: Settings,
    pub devices: Vec<DeviceInfo>,
    pub selected_device: Option<DeviceInfo>,
    pub installed: Vec<InstalledModel>,
    pub logs: Vec<LogEntry>,
    pub log_seq: u64,
    pub ping_status: Option<PingResult>,
    pub is_transcribing: bool,
    pub workspace_refinements_expanded: bool,
    pub subtitle_preview: Option<crate::subtitle_preview::Preview>,
    pub live_cues: Vec<crate::subtitle_preview::Cue>,
    pub result_reveal: f32,
    pub workspace_width: f32,
    system_reduced_motion: bool,
    reveal_started: Option<std::time::Instant>,
    preview_request: u64,
    pub transcribe_progress: Option<TranscribeProgress>,
    pub transcribe_result: Option<TranscribeResult>,
    pub downloading: HashMap<String, DownloadInfo>,
    pub log_content: text_editor::Content,
    /// Last language Whisper detected for the running/most-recent job.
    /// Cleared when a new transcription starts.
    pub detected_language: Option<DetectedLanguage>,
    transcribe_cancel: Option<watch::Sender<bool>>,
    download_cancel: HashMap<String, watch::Sender<bool>>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Message {
    Loaded,
    MotionTick(std::time::Instant),
    MotionPreferenceTick,
    WindowResized(f32),
    ToggleReduceMotion(bool),
    SubtitlePreviewReady(u64, Option<crate::subtitle_preview::Preview>),
    Tick,
    Ping(PingResult),
    DevicesLoaded(Vec<DeviceInfo>),
    InstalledLoaded(Vec<InstalledModel>),

    NavigateTo(Screen),

    SetInputPath(String),
    SetOutputDir(String),
    PickFile,
    PickFileResult(Option<String>),
    PickDir,
    PickDirResult(Option<String>),
    SelectDevice(DeviceInfo),
    SelectModel(String),
    ToggleExportFormat(String),

    SetThreads(String),
    SetBeamSize(String),
    SetBestOf(String),
    SetMaxLenChars(String),
    SetVadThreshold(String),
    SetVadMinSpeechMs(String),
    SetVadMinSilMs(String),
    SetVadPadMs(String),
    SetNoSpeechThold(String),
    SetMaxContext(String),
    SetDedupMergeGapSec(String),
    SetLanguage(String),
    ToggleSplitOnWord(bool),
    ToggleFlashAttn(bool),
    ToggleTranslate(bool),
    ToggleDryRun(bool),

    ToggleWorkspaceRefinements,
    CloseWorkspaceRefinements,
    SetInitialPrompt(String),
    AddReplacement,
    RemoveReplacement(usize),
    SetReplacementFrom(usize, String),
    SetReplacementTo(usize, String),
    ToggleReplacementCaseSensitive(usize, bool),
    ToggleReplacementWholeWord(usize, bool),

    ToggleResegmentEnabled(bool),
    SetMaxCueChars(String),
    SetMaxCueMs(String),
    SetMinCueMs(String),
    ToggleTokenTimestamps(bool),

    StartTranscribe,
    TranscribeEvent(Event),
    TranscribeFinished {
        outcome: Result<TranscribeOutcome, String>,
        dry_run: bool,
    },
    CancelTranscribe,
    OpenOutputFolder(String),

    StartDownload(String),
    DownloadEvent(String, Event),
    DownloadFinished(String, Result<String, String>),
    CancelDownload(String),
    DeleteModel(String),

    SmokeTest,
    SmokeTestResult(Result<String, String>),
    AddLog(String),
    SaveSettings,
    ClearLogs,
    LogAction(text_editor::Action),
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        let settings = load_settings();
        #[allow(unused_mut)]
        let mut app = App {
            screen: Screen::Workspace,
            settings,
            devices: Vec::new(),
            selected_device: None,
            installed: installed_models(&models_directory()),
            logs: Vec::new(),
            log_seq: 0,
            ping_status: None,
            is_transcribing: false,
            workspace_refinements_expanded: false,
            subtitle_preview: None,
            live_cues: Vec::new(),
            result_reveal: 1.0,
            workspace_width: 1100.0,
            system_reduced_motion: crate::motion::system_reduced(),
            reveal_started: None,
            preview_request: 0,
            transcribe_progress: None,
            transcribe_result: None,
            downloading: HashMap::new(),
            log_content: text_editor::Content::new(),
            detected_language: None,
            transcribe_cancel: None,
            download_cancel: HashMap::new(),
        };

        // Deterministic debug-only inspection of the real result card: no model,
        // transcription, update check or settings write is needed.
        #[cfg(debug_assertions)]
        if std::env::args().any(|arg| arg == "--ui-result-preview") {
            app.settings = Settings::default();
            app.settings.input_path = "Synthetic preview — no transcription performed".to_string();
            // Retry can exercise the normal missing-model prerequisite without
            // ever starting transcription, even on a machine with models.
            app.settings.selected_model = None;
            app.transcribe_result = Some(
                if std::env::args().any(|arg| arg == "--ui-preview-failed") {
                    TranscribeResult::Failed("Synthetic preview: the media file could not be decoded. No transcription was performed.".to_string())
                } else if std::env::args().any(|arg| arg == "--ui-preview-cancelled") {
                    TranscribeResult::Cancelled
                } else {
                    TranscribeResult::Completed {
                        jobs: 1,
                        dry_run: std::env::args().any(|arg| arg == "--ui-preview-dry-run"),
                        outputs: if std::env::args().any(|arg| arg == "--ui-preview-empty") {
                            Vec::new()
                        } else {
                            vec![
                                "/tmp/subtly-synthetic-preview/interview.srt".to_string(),
                                "/tmp/subtly-synthetic-preview/interview.vtt".to_string(),
                            ]
                        },
                    }
                },
            );
            if std::env::args().any(|arg| arg == "--ui-preview-idle") {
                app.transcribe_result = None;
            }
            let preview = app.load_subtitle_preview();
            return (app, Task::batch([preview, Self::measure_window()]));
        }

        let init = Task::batch([
            Task::done(Message::Loaded),
            Self::measure_window(),
            Task::perform(async { ping() }, Message::Ping),
            Task::perform(async { list_devices() }, Message::DevicesLoaded),
        ]);
        (app, init)
    }

    pub fn theme(&self) -> Theme {
        t::subtly_theme()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let keys = if self.screen == Screen::Workspace {
            iced::event::listen_with(|event, _, _| {
                let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key, modifiers, ..
                }) = event
                else {
                    return None;
                };
                match key.as_ref() {
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) => {
                        Some(Message::CloseWorkspaceRefinements)
                    }
                    iced::keyboard::Key::Character("r" | "R")
                        if modifiers.command() && modifiers.shift() =>
                    {
                        Some(Message::ToggleWorkspaceRefinements)
                    }
                    _ => None,
                }
            })
        } else {
            Subscription::none()
        };
        let motion = if self.reveal_started.is_some() {
            iced::time::every(Duration::from_millis(16)).map(Message::MotionTick)
        } else {
            Subscription::none()
        };
        Subscription::batch([
            motion,
            iced::time::every(Duration::from_millis(250)).map(|_| Message::MotionPreferenceTick),
            iced::window::resize_events().map(|(_, size)| Message::WindowResized(size.width)),
            iced::time::every(Duration::from_secs(10)).map(|_| Message::Tick),
            keys,
        ])
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Loaded => Task::none(),
            Message::WindowResized(width) => {
                self.workspace_width = width;
                Task::none()
            }
            Message::MotionPreferenceTick => {
                self.system_reduced_motion = crate::motion::system_reduced();
                if self.reduced_motion() {
                    self.result_reveal = 1.0;
                    self.reveal_started = None;
                }
                Task::none()
            }
            Message::MotionTick(now) => {
                self.system_reduced_motion = crate::motion::system_reduced();
                if self.reduced_motion() {
                    self.result_reveal = 1.0;
                    self.reveal_started = None;
                } else if let Some(start) = self.reveal_started {
                    let fraction = now.saturating_duration_since(start).as_secs_f32() / 0.48;
                    self.result_reveal = 1.0 - (1.0 - fraction.clamp(0.0, 1.0)).powi(3);
                    if fraction >= 1.0 {
                        self.reveal_started = None;
                    }
                }
                Task::none()
            }
            Message::ToggleReduceMotion(reduced) => {
                self.settings.reduce_motion = reduced;
                if self.reduced_motion() {
                    self.result_reveal = 1.0;
                    self.reveal_started = None;
                }
                self.persist()
            }
            Message::SubtitlePreviewReady(request, preview) => {
                if request == self.preview_request {
                    self.subtitle_preview = preview;
                    if self.subtitle_preview.is_some() && !self.reduced_motion() {
                        self.result_reveal = 0.0;
                        self.reveal_started = Some(std::time::Instant::now());
                    }
                }
                Task::none()
            }
            Message::Tick => Task::perform(async { ping() }, Message::Ping),
            Message::Ping(p) => {
                self.ping_status = Some(p);
                Task::none()
            }
            Message::DevicesLoaded(devices) => {
                if self.selected_device.is_none() {
                    self.selected_device =
                        subtly_core::devices::select_best_device(&devices).cloned();
                    if let Some(d) = &self.selected_device {
                        self.add_log(format!("Auto-selected device: {} ({})", d.name, d.backend));
                    }
                }
                self.add_log(format!("Detected {} device(s)", devices.len()));
                self.devices = devices;
                Task::none()
            }
            Message::InstalledLoaded(installed) => {
                self.installed = installed;
                Task::none()
            }
            Message::NavigateTo(s) => {
                self.screen = s;
                Task::none()
            }
            Message::ToggleWorkspaceRefinements => {
                self.workspace_refinements_expanded = !self.workspace_refinements_expanded;
                if self.workspace_refinements_expanded {
                    Task::batch([
                        scrollable::snap_to(
                            scrollable::Id::new("workspace-body"),
                            scrollable::RelativeOffset::END,
                        ),
                        iced::widget::text_input::focus("workspace-vocabulary"),
                    ])
                } else {
                    self.focus_workspace_output()
                }
            }
            Message::CloseWorkspaceRefinements => {
                if self.workspace_refinements_expanded {
                    self.workspace_refinements_expanded = false;
                    self.focus_workspace_output()
                } else {
                    Task::none()
                }
            }
            Message::SetInputPath(v) => {
                self.settings.input_path = v;
                self.persist()
            }
            Message::SetOutputDir(v) => {
                self.settings.output_dir = v;
                self.persist()
            }
            Message::PickFile => Task::perform(pick_file(), Message::PickFileResult),
            Message::PickFileResult(Some(p)) => {
                self.settings.input_path = p;
                self.persist()
            }
            Message::PickFileResult(None) => Task::none(),
            Message::PickDir => Task::perform(pick_dir(), Message::PickDirResult),
            Message::PickDirResult(Some(p)) => {
                self.settings.input_path = p;
                self.persist()
            }
            Message::PickDirResult(None) => Task::none(),
            Message::SelectDevice(d) => {
                self.selected_device = Some(d);
                Task::none()
            }
            Message::SelectModel(id) => {
                self.settings.selected_model = Some(id);
                self.persist()
            }
            Message::ToggleExportFormat(fmt) => {
                let formats = &mut self.settings.export_formats;
                if let Some(idx) = formats.iter().position(|f| f == &fmt) {
                    if formats.len() > 1 {
                        formats.remove(idx);
                    }
                } else {
                    formats.push(fmt);
                }
                self.persist()
            }
            Message::SetThreads(v) => {
                if let Ok(n) = v.parse() {
                    self.settings.threads = n;
                }
                self.persist()
            }
            Message::SetBeamSize(v) => {
                if let Ok(n) = v.parse() {
                    self.settings.beam_size = n;
                }
                self.persist()
            }
            Message::SetBestOf(v) => {
                if let Ok(n) = v.parse() {
                    self.settings.best_of = n;
                }
                self.persist()
            }
            Message::SetMaxLenChars(v) => {
                if let Ok(n) = v.parse() {
                    self.settings.max_len_chars = n;
                }
                self.persist()
            }
            Message::SetVadThreshold(v) => {
                if let Ok(n) = v.parse() {
                    self.settings.vad_threshold = n;
                }
                self.persist()
            }
            Message::SetVadMinSpeechMs(v) => {
                if let Ok(n) = v.parse() {
                    self.settings.vad_min_speech_ms = n;
                }
                self.persist()
            }
            Message::SetVadMinSilMs(v) => {
                if let Ok(n) = v.parse() {
                    self.settings.vad_min_sil_ms = n;
                }
                self.persist()
            }
            Message::SetVadPadMs(v) => {
                if let Ok(n) = v.parse() {
                    self.settings.vad_pad_ms = n;
                }
                self.persist()
            }
            Message::SetNoSpeechThold(v) => {
                if let Ok(n) = v.parse() {
                    self.settings.no_speech_thold = n;
                }
                self.persist()
            }
            Message::SetMaxContext(v) => {
                if let Ok(n) = v.parse() {
                    self.settings.max_context = n;
                }
                self.persist()
            }
            Message::SetDedupMergeGapSec(v) => {
                if let Ok(n) = v.parse() {
                    self.settings.dedup_merge_gap_sec = n;
                }
                self.persist()
            }
            Message::SetLanguage(v) => {
                self.settings.language = v;
                self.persist()
            }
            Message::ToggleSplitOnWord(v) => {
                self.settings.split_on_word = v;
                self.persist()
            }
            Message::ToggleFlashAttn(v) => {
                self.settings.flash_attn = v && subtly_core::whisper::flash_attention_is_safe();
                self.persist()
            }
            Message::ToggleTranslate(v) => {
                self.settings.translate = v;
                self.persist()
            }
            Message::ToggleDryRun(v) => {
                self.settings.dry_run = v;
                self.persist()
            }

            Message::SetInitialPrompt(v) => {
                self.settings.initial_prompt = v;
                self.persist()
            }
            Message::AddReplacement => {
                self.settings.replacements.push(ReplaceRule::default());
                self.persist()
            }
            Message::RemoveReplacement(idx) => {
                if idx < self.settings.replacements.len() {
                    self.settings.replacements.remove(idx);
                }
                self.persist()
            }
            Message::SetReplacementFrom(idx, v) => {
                if let Some(r) = self.settings.replacements.get_mut(idx) {
                    r.from = v;
                }
                self.persist()
            }
            Message::SetReplacementTo(idx, v) => {
                if let Some(r) = self.settings.replacements.get_mut(idx) {
                    r.to = v;
                }
                self.persist()
            }
            Message::ToggleReplacementCaseSensitive(idx, v) => {
                if let Some(r) = self.settings.replacements.get_mut(idx) {
                    r.case_sensitive = v;
                }
                self.persist()
            }
            Message::ToggleReplacementWholeWord(idx, v) => {
                if let Some(r) = self.settings.replacements.get_mut(idx) {
                    r.whole_word = v;
                }
                self.persist()
            }
            Message::ToggleResegmentEnabled(v) => {
                self.settings.resegment_enabled = v;
                if v {
                    // The resegmenter relies on word-aligned timing; flip the
                    // dependent flag on so users don't have to know.
                    self.settings.token_timestamps = true;
                }
                self.persist()
            }
            Message::SetMaxCueChars(v) => {
                if let Ok(n) = v.parse() {
                    self.settings.max_cue_chars = n;
                }
                self.persist()
            }
            Message::SetMaxCueMs(v) => {
                if let Ok(n) = v.parse() {
                    self.settings.max_cue_ms = n;
                }
                self.persist()
            }
            Message::SetMinCueMs(v) => {
                if let Ok(n) = v.parse() {
                    self.settings.min_cue_ms = n;
                }
                self.persist()
            }
            Message::ToggleTokenTimestamps(v) => {
                self.settings.token_timestamps = v;
                if !v {
                    self.settings.resegment_enabled = false;
                }
                self.persist()
            }

            Message::StartTranscribe => {
                self.preview_request = self.preview_request.wrapping_add(1);
                self.subtitle_preview = None;
                self.live_cues.clear();
                self.reveal_started = None;
                self.result_reveal = 1.0;
                self.start_transcribe()
            }
            Message::TranscribeEvent(event) => {
                self.handle_transcribe_event(event);
                Task::none()
            }
            Message::TranscribeFinished {
                outcome: result,
                dry_run,
            } => {
                self.is_transcribing = false;
                self.transcribe_cancel = None;
                self.transcribe_progress = None;
                match result {
                    Ok(outcome) => {
                        self.add_log(format!(
                            "{} {} job(s).",
                            if dry_run {
                                "Dry run completed:"
                            } else {
                                "Completed"
                            },
                            outcome.jobs
                        ));
                        for o in &outcome.outputs {
                            self.add_log(format!(
                                "{}: {o}",
                                if dry_run { "Planned" } else { "Wrote" }
                            ));
                        }
                        self.transcribe_result = Some(TranscribeResult::Completed {
                            jobs: outcome.jobs,
                            outputs: outcome.outputs,
                            dry_run,
                        });
                    }
                    Err(e) => {
                        self.transcribe_result = Some(if e.to_lowercase().contains("cancelled") {
                            TranscribeResult::Cancelled
                        } else {
                            TranscribeResult::Failed(e.clone())
                        });
                        self.add_log(format!("Transcription failed: {e}"));
                    }
                }
                self.load_subtitle_preview()
            }
            Message::OpenOutputFolder(path) => {
                #[cfg(target_os = "macos")]
                let result = std::process::Command::new("open").arg(&path).spawn();
                #[cfg(target_os = "windows")]
                let result = std::process::Command::new("explorer").arg(&path).spawn();
                #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                let result = std::process::Command::new("xdg-open").arg(&path).spawn();
                if let Err(error) = result {
                    self.add_log(format!("Could not open output folder: {error}"));
                }
                Task::none()
            }
            Message::CancelTranscribe => {
                if let Some(tx) = &self.transcribe_cancel {
                    let _ = tx.send(true);
                    self.add_log("Cancellation requested…".to_string());
                    if let Some(progress) = &mut self.transcribe_progress {
                        progress.status_message = "Cancelling…".to_string();
                        progress.cancelling = true;
                    }
                }
                Task::none()
            }
            Message::CancelDownload(model_id) => {
                if let Some(tx) = self.download_cancel.get(&model_id) {
                    let _ = tx.send(true);
                    self.add_log(format!("Download cancellation requested: {model_id}"));
                }
                if let Some(info) = self.downloading.get_mut(&model_id) {
                    info.cancelling = true;
                }
                Task::none()
            }
            Message::StartDownload(model_id) => self.start_download(model_id),
            Message::DownloadEvent(model_id, event) => {
                if let Event::Progress(p) = &event {
                    if let (Some(progress), Some(curr), Some(total)) =
                        (p.progress, p.current, p.total)
                    {
                        let cancelling = self
                            .downloading
                            .get(&model_id)
                            .map(|d| d.cancelling)
                            .unwrap_or(false);
                        self.downloading.insert(
                            model_id.clone(),
                            DownloadInfo {
                                progress,
                                downloaded_bytes: curr,
                                total_bytes: total,
                                cancelling,
                            },
                        );
                    }
                }
                Task::none()
            }
            Message::DownloadFinished(model_id, result) => {
                self.downloading.remove(&model_id);
                self.download_cancel.remove(&model_id);
                match result {
                    Ok(_) => {
                        self.add_log(format!("Downloaded: {model_id}"));
                        self.installed = installed_models(&models_directory());
                        let selected_installed =
                            self.settings.selected_model.as_deref().is_some_and(|id| {
                                self.installed
                                    .iter()
                                    .any(|m| m.descriptor.id == id && m.complete)
                            });
                        if model_id != "silero-vad" && !selected_installed {
                            self.settings.selected_model = Some(model_id);
                            return self.persist();
                        }
                    }
                    Err(e) => self.add_log(format!("Download failed: {e}")),
                }
                Task::none()
            }
            Message::DeleteModel(model_id) => {
                match delete_model(&model_id) {
                    Ok(true) => {
                        self.add_log(format!("Deleted model: {model_id}"));
                        if self.settings.selected_model.as_deref() == Some(model_id.as_str()) {
                            self.settings.selected_model = None;
                        }
                        self.installed = installed_models(&models_directory());
                    }
                    Ok(false) => self.add_log(format!("Model not present: {model_id}")),
                    Err(e) => self.add_log(format!("Delete failed: {e}")),
                }
                self.persist()
            }
            Message::SmokeTest => {
                self.add_log("Running GPU smoke test...".to_string());
                Task::perform(
                    async { smoke_test().map_err(|e| e.to_string()) },
                    Message::SmokeTestResult,
                )
            }
            Message::SmokeTestResult(Ok(msg)) => {
                self.add_log(msg);
                Task::none()
            }
            Message::SmokeTestResult(Err(e)) => {
                self.add_log(format!("Smoke test failed: {e}"));
                Task::none()
            }
            Message::AddLog(message) => {
                self.add_log(message);
                Task::none()
            }
            Message::SaveSettings => self.persist(),
            Message::ClearLogs => {
                self.logs.clear();
                self.log_content = text_editor::Content::new();
                Task::none()
            }
            Message::LogAction(action) => {
                if !action.is_edit() {
                    self.log_content.perform(action);
                }
                Task::none()
            }
        }
    }

    fn measure_window() -> Task<Message> {
        iced::window::get_latest().and_then(|id| {
            iced::window::get_size(id).map(|size| Message::WindowResized(size.width))
        })
    }

    fn reduced_motion(&self) -> bool {
        self.settings.reduce_motion || self.system_reduced_motion
    }

    fn load_subtitle_preview(&mut self) -> Task<Message> {
        self.preview_request = self.preview_request.wrapping_add(1);
        self.subtitle_preview = None;
        let request = self.preview_request;
        let Some(TranscribeResult::Completed {
            outputs,
            dry_run: false,
            ..
        }) = &self.transcribe_result
        else {
            return Task::none();
        };
        let outputs = outputs.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || crate::subtitle_preview::read(&outputs, false))
                    .await
                    .ok()
                    .flatten()
            },
            move |preview| Message::SubtitlePreviewReady(request, preview),
        )
    }

    fn focus_workspace_output(&self) -> Task<Message> {
        Task::batch([
            scrollable::snap_to(
                scrollable::Id::new("workspace-body"),
                scrollable::RelativeOffset::START,
            ),
            iced::widget::text_input::focus("workspace-output"),
        ])
    }

    pub fn view(&self) -> Element<'_, Message> {
        let sidebar = self.sidebar();
        // The Logs screen renders a text_editor that handles its own
        // scrolling. It must NOT be placed inside a scrollable (an
        // unbounded-height parent makes text_editor's layout panic).
        let body_area: Element<'_, Message> = match self.screen {
            Screen::Logs => container(self.logs_view())
                .padding([24, 28])
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
            _ => {
                let inner: Element<'_, Message> = match self.screen {
                    Screen::Workspace => workspace::view(self),
                    Screen::Models => models_screen::view(self),
                    Screen::Advanced => advanced::view(self),
                    Screen::Logs => unreachable!(),
                };
                scrollable(container(inner).padding([24, 28]).width(Length::Fill))
                    .id(scrollable::Id::new("workspace-body"))
                    .style(t::scrollable_style)
                    .height(Length::Fill)
                    .into()
            }
        };

        let main_area = column![
            self.top_bar(),
            container(body_area)
                .height(Length::Fill)
                .width(Length::Fill),
            self.status_bar(),
        ];

        row![sidebar, main_area].into()
    }

    fn sidebar(&self) -> Element<'_, Message> {
        let item = |s: Screen, hint: &str| {
            let label = column![
                text(s.label()).size(14),
                text(hint.to_string()).size(11).color(t::TEXT_FAINT),
            ]
            .spacing(2);
            button(label)
                .padding([10, 14])
                .width(Length::Fill)
                .on_press(Message::NavigateTo(s))
                .style(t::nav_button(self.screen == s))
        };

        let dl_count = self.downloading.len();
        let installed_count = self
            .installed
            .iter()
            .filter(|m| m.complete && m.descriptor.id != "silero-vad")
            .count();

        let nav = column![
            text("Subtly".to_string()).size(22),
            text("Whisper subtitle studio".to_string())
                .size(11)
                .color(t::TEXT_MUTED),
            Space::with_height(Length::Fixed(20.0)),
            item(Screen::Workspace, "Generate subtitles"),
            item(
                Screen::Models,
                if dl_count > 0 {
                    "Downloading…"
                } else if installed_count == 0 {
                    "Download a model"
                } else {
                    "Manage models"
                }
            ),
            item(Screen::Advanced, "Pipeline settings"),
            item(Screen::Logs, "Activity log"),
        ]
        .spacing(4)
        .padding(20)
        .width(Length::Fixed(220.0));

        container(nav).height(Length::Fill).style(t::sidebar).into()
    }

    fn top_bar(&self) -> Element<'_, Message> {
        let (status_color_fn, status_text): (fn(&Theme) -> container::Style, String) =
            match &self.ping_status {
                Some(p) if p.gpu_enabled => (
                    t::status_dot_ok,
                    format!("{} · {}", p.message, p.gpu_backend),
                ),
                Some(p) => (
                    t::status_dot_warn,
                    format!("{} · {}", p.message, p.gpu_backend),
                ),
                None => (t::status_dot_warn, "Booting…".to_string()),
            };

        let dot =
            container(Space::new(Length::Fixed(8.0), Length::Fixed(8.0))).style(status_color_fn);

        let title = text(self.screen.label().to_string()).size(20);

        container(
            row![
                title,
                Space::with_width(Length::Fill),
                row![dot, text(status_text).size(12).color(t::TEXT_MUTED)]
                    .spacing(8)
                    .align_y(Alignment::Center),
            ]
            .align_y(Alignment::Center)
            .spacing(12),
        )
        .padding([14, 28])
        .width(Length::Fill)
        .into()
    }

    fn status_bar(&self) -> Element<'_, Message> {
        let device = self
            .selected_device
            .as_ref()
            .map(|d| d.name.clone())
            .unwrap_or_else(|| "No device".to_string());
        let model = self
            .settings
            .selected_model
            .clone()
            .unwrap_or_else(|| "No model".to_string());
        let installed = self
            .installed
            .iter()
            .filter(|m| m.complete && m.descriptor.id != "silero-vad")
            .count();

        let last_log = self.logs.last().map(|e| e.message.clone());

        container(
            row![
                text(format!("Device: {device}"))
                    .size(11)
                    .color(t::TEXT_MUTED),
                Space::with_width(Length::Fixed(20.0)),
                text(format!("Model: {model}"))
                    .size(11)
                    .color(t::TEXT_MUTED),
                Space::with_width(Length::Fixed(20.0)),
                text(format!("{installed} model(s) installed"))
                    .size(11)
                    .color(t::TEXT_MUTED),
                Space::with_width(Length::Fill),
                if let Some(last) = last_log {
                    text(truncate(&last, 80)).size(11).color(t::TEXT_FAINT)
                } else {
                    text(String::new())
                },
            ]
            .align_y(Alignment::Center),
        )
        .padding([8, 28])
        .width(Length::Fill)
        .into()
    }

    fn logs_view(&self) -> Element<'_, Message> {
        let actions = row![
            text(format!("{} entries", self.logs.len()))
                .size(12)
                .color(t::TEXT_MUTED),
            Space::with_width(Length::Fill),
            button(text("Clear".to_string()).size(12))
                .on_press(Message::ClearLogs)
                .style(t::ghost_button),
        ]
        .align_y(Alignment::Center);

        let body: Element<'_, Message> = if self.logs.is_empty() {
            container(
                text("No activity yet.".to_string())
                    .size(13)
                    .color(t::TEXT_MUTED),
            )
            .style(t::surface_inset)
            .padding(14)
            .width(Length::Fill)
            .into()
        } else {
            text_editor(&self.log_content)
                .on_action(Message::LogAction)
                .padding(14)
                .size(12)
                .height(Length::Fill)
                .style(t::log_editor_style)
                .into()
        };

        column![actions, Space::with_height(Length::Fixed(8.0)), body,]
            .height(Length::Fill)
            .into()
    }

    fn add_log(&mut self, message: String) {
        self.log_seq += 1;
        self.logs.push(LogEntry {
            id: self.log_seq,
            message,
        });
        if self.logs.len() > MAX_LOG_ENTRIES {
            let drop_count = self.logs.len() - MAX_LOG_ENTRIES;
            self.logs.drain(..drop_count);
        }
        self.rebuild_log_content();
    }

    fn rebuild_log_content(&mut self) {
        let joined = self
            .logs
            .iter()
            .map(|e| e.message.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        self.log_content = text_editor::Content::with_text(&joined);
    }

    fn persist(&self) -> Task<Message> {
        #[cfg(debug_assertions)]
        if std::env::args().any(|arg| arg == "--ui-result-preview") {
            return Task::none();
        }

        let _ = save_settings(&self.settings);
        Task::none()
    }

    fn handle_transcribe_event(&mut self, event: Event) {
        match event {
            Event::Log(msg) => self.add_log(msg),
            Event::Progress(p) => {
                if let Some(progress) = &mut self.transcribe_progress {
                    if let Some(pct) = p.progress {
                        progress.progress = pct;
                    }
                    if let Some(phase) = p.phase {
                        if !progress.cancelling {
                            progress.status_message = phase;
                        }
                    }
                    if p.current.is_some() {
                        progress.current_item = p.current;
                    }
                    if p.total.is_some() {
                        progress.total_items = p.total;
                    }
                }
            }
            Event::Segment(s) => {
                self.live_cues.push(crate::subtitle_preview::Cue {
                    timecode: format!(
                        "{} --> {}",
                        crate::subtitle_preview::timecode(s.start_ms),
                        crate::subtitle_preview::timecode(s.end_ms)
                    ),
                    text: s.text.chars().take(300).collect(),
                });
                if self.live_cues.len() > 3 {
                    self.live_cues.remove(0);
                }
                self.add_log(format!("[{:>7.2}s] {}", s.start_ms as f32 / 1000.0, s.text));
            }
            Event::DetectedLanguage(d) => {
                self.add_log(format!("Detected language: {} ({})", d.name, d.code));
                self.detected_language = Some(d);
            }
            Event::OutputWritten(_) => {}
            Event::Done => {}
        }
    }

    fn start_transcribe(&mut self) -> Task<Message> {
        if self.is_transcribing {
            return Task::none();
        }
        let whisper_path = self
            .settings
            .selected_model
            .as_deref()
            .and_then(model_path_if_present);
        let vad_path = model_path_if_present("silero-vad");
        let (Some(whisper), Some(vad)) = (whisper_path, vad_path) else {
            self.add_log(
                "Cannot transcribe: download a Whisper model and the Silero VAD model first."
                    .to_string(),
            );
            self.screen = Screen::Models;
            return Task::none();
        };
        let params = match self
            .settings
            .build_transcribe_params(&whisper.to_string_lossy(), &vad.to_string_lossy())
        {
            Ok(p) => p,
            Err(e) => {
                self.add_log(format!("Invalid settings: {e}"));
                return Task::none();
            }
        };
        self.is_transcribing = true;
        self.transcribe_result = None;
        self.detected_language = None;
        self.transcribe_progress = Some(TranscribeProgress {
            progress: 0,
            status_message: "Initializing…".to_string(),
            current_item: None,
            total_items: None,
            cancelling: false,
        });
        self.add_log("Starting subtitle generation...".to_string());

        let (tx, rx) = mpsc::channel::<Event>(64);
        let (cancel_tx, cancel_rx) = watch::channel(false);
        self.transcribe_cancel = Some(cancel_tx);
        let event_stream = receiver_stream(rx).map(Message::TranscribeEvent);

        let dry_run = params.dry_run.unwrap_or(false);
        let work = async move {
            transcribe(params, tx, cancel_rx)
                .await
                .map_err(|e| e.to_string())
        };

        Task::batch([
            event_stream,
            Task::perform(work, move |outcome| Message::TranscribeFinished {
                outcome,
                dry_run,
            }),
        ])
    }

    fn start_download(&mut self, model_id: String) -> Task<Message> {
        if self.downloading.contains_key(&model_id) {
            return Task::none();
        }
        self.downloading.insert(
            model_id.clone(),
            DownloadInfo {
                progress: 0,
                downloaded_bytes: 0,
                total_bytes: 0,
                cancelling: false,
            },
        );
        self.add_log(format!("Starting download: {model_id}"));

        let (tx, rx) = mpsc::channel::<Event>(64);
        let (cancel_tx, cancel_rx) = watch::channel(false);
        self.download_cancel.insert(model_id.clone(), cancel_tx);
        let id_for_stream = model_id.clone();
        let event_stream = receiver_stream(rx)
            .map(move |event| Message::DownloadEvent(id_for_stream.clone(), event));

        let id_for_finish = model_id.clone();
        let id_arc = Arc::new(model_id);
        let id_for_work = id_arc.clone();
        let work = async move {
            download_model(&id_for_work, tx, cancel_rx)
                .await
                .map(|p| p.display().to_string())
                .map_err(|e| e.to_string())
        };

        Task::batch([
            event_stream,
            Task::perform(work, move |r| {
                Message::DownloadFinished(id_for_finish.clone(), r)
            }),
        ])
    }
}

async fn pick_file() -> Option<String> {
    rfd::AsyncFileDialog::new()
        .add_filter("Media", &["mp4", "mkv", "mov", "wav", "mp3", "m4a"])
        .pick_file()
        .await
        .map(|f| f.path().to_string_lossy().into_owned())
}

async fn pick_dir() -> Option<String> {
    rfd::AsyncFileDialog::new()
        .pick_folder()
        .await
        .map(|f| f.path().to_string_lossy().into_owned())
}

fn receiver_stream<T>(rx: mpsc::Receiver<T>) -> Task<T>
where
    T: Send + 'static,
{
    use tokio_stream::wrappers::ReceiverStream;
    Task::stream(ReceiverStream::new(rx))
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

#[cfg(test)]
mod presentation_tests {
    use super::*;
    fn app() -> App {
        let (mut app, _) = App::new();
        app.settings = Settings::default();
        app.system_reduced_motion = false;
        app
    }
    fn preview(text: &str) -> crate::subtitle_preview::Preview {
        crate::subtitle_preview::Preview {
            path: "synthetic-test.srt".into(),
            cues: vec![crate::subtitle_preview::Cue {
                timecode: "00:00:01.000 --> 00:00:02.000".into(),
                text: text.into(),
            }],
        }
    }
    #[test]
    fn old_result_cannot_replace_current_caption_deck() {
        let mut app = app();
        app.preview_request = 2;
        let _ = app.update(Message::SubtitlePreviewReady(
            2,
            Some(preview("Current result")),
        ));
        let _ = app.update(Message::SubtitlePreviewReady(
            1,
            Some(preview("Old result")),
        ));
        assert_eq!(
            app.subtitle_preview.as_ref().unwrap().cues[0].text,
            "Current result"
        );
    }
    #[test]
    fn reduction_settles_active_typesetting_and_future_motion_can_restore() {
        let mut app = app();
        let _ = app.update(Message::SubtitlePreviewReady(
            0,
            Some(preview("Current result")),
        ));
        assert_eq!(app.result_reveal, 0.0);
        assert!(app.reveal_started.is_some());
        app.settings.reduce_motion = true;
        let _ = app.update(Message::MotionTick(std::time::Instant::now()));
        assert_eq!(app.result_reveal, 1.0);
        assert!(app.reveal_started.is_none());
        assert_eq!(
            app.subtitle_preview.as_ref().unwrap().cues[0].text,
            "Current result"
        );
        app.settings.reduce_motion = false;
        app.system_reduced_motion = false;
        let _ = app.update(Message::SubtitlePreviewReady(
            0,
            Some(preview("Next result")),
        ));
        assert_eq!(app.result_reveal, 0.0);
        assert!(app.reveal_started.is_some());
    }
    #[test]
    fn live_excerpt_keeps_only_the_latest_actual_segments() {
        let mut app = app();
        for i in 0..5 {
            app.handle_transcribe_event(Event::Segment(subtly_core::events::SegmentEvent {
                start_ms: i * 1000,
                end_ms: i * 1000 + 750,
                text: format!("Actual event {i}"),
            }));
        }
        assert_eq!(app.live_cues.len(), 3);
        assert_eq!(app.live_cues[0].text, "Actual event 2");
        assert_eq!(app.live_cues[2].timecode, "00:00:04.000 --> 00:00:04.750");
    }
}
