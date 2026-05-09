//! Subtly app state, Message enum, update/view, subscription.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use iced::widget::{button, column, container, row, scrollable, text, text_editor, Space};
use iced::{Alignment, Element, Length, Subscription, Task, Theme};
use tokio::sync::{mpsc, watch};

use subtly_core::devices::{list_devices, ping, smoke_test, DeviceInfo, PingResult};
use subtly_core::download::{delete_model, download_model, model_path_if_present};
use subtly_core::events::Event;
use subtly_core::models::{installed_models, InstalledModel};
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
    pub transcribe_progress: Option<TranscribeProgress>,
    pub downloading: HashMap<String, DownloadInfo>,
    pub log_content: text_editor::Content,
    transcribe_cancel: Option<watch::Sender<bool>>,
    download_cancel: HashMap<String, watch::Sender<bool>>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Message {
    Loaded,
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

    StartTranscribe,
    TranscribeEvent(Event),
    TranscribeFinished(Result<TranscribeOutcome, String>),
    CancelTranscribe,

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
        let app = App {
            screen: Screen::Workspace,
            settings,
            devices: Vec::new(),
            selected_device: None,
            installed: installed_models(&models_directory()),
            logs: Vec::new(),
            log_seq: 0,
            ping_status: None,
            is_transcribing: false,
            transcribe_progress: None,
            downloading: HashMap::new(),
            log_content: text_editor::Content::new(),
            transcribe_cancel: None,
            download_cancel: HashMap::new(),
        };

        let init = Task::batch([
            Task::done(Message::Loaded),
            Task::perform(async { ping() }, Message::Ping),
            Task::perform(async { list_devices() }, Message::DevicesLoaded),
        ]);
        (app, init)
    }

    pub fn theme(&self) -> Theme {
        t::subtly_theme()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        iced::time::every(Duration::from_secs(10)).map(|_| Message::Tick)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Loaded => Task::none(),
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
                self.settings.flash_attn = v;
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

            Message::StartTranscribe => self.start_transcribe(),
            Message::TranscribeEvent(event) => {
                self.handle_transcribe_event(event);
                Task::none()
            }
            Message::TranscribeFinished(result) => {
                self.is_transcribing = false;
                self.transcribe_cancel = None;
                self.transcribe_progress = None;
                match result {
                    Ok(outcome) => {
                        self.add_log(format!("Completed {} job(s).", outcome.jobs));
                        for o in outcome.outputs {
                            self.add_log(format!("Wrote: {o}"));
                        }
                    }
                    Err(e) => self.add_log(format!("Transcription failed: {e}")),
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

    pub fn view(&self) -> Element<'_, Message> {
        let sidebar = self.sidebar();
        let body: Element<'_, Message> = match self.screen {
            Screen::Workspace => workspace::view(self),
            Screen::Models => models_screen::view(self),
            Screen::Advanced => advanced::view(self),
            Screen::Logs => self.logs_view(),
        };
        let body_scrollable = scrollable(container(body).padding([24, 28]).width(Length::Fill))
            .style(t::scrollable_style)
            .height(Length::Fill);

        let main_area = column![
            self.top_bar(),
            container(body_scrollable).height(Length::Fill).width(Length::Fill),
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

        container(nav)
            .height(Length::Fill)
            .style(t::sidebar)
            .into()
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

        let dot = container(Space::new(Length::Fixed(8.0), Length::Fixed(8.0)))
            .style(status_color_fn);

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
                text(format!("Device: {device}")).size(11).color(t::TEXT_MUTED),
                Space::with_width(Length::Fixed(20.0)),
                text(format!("Model: {model}")).size(11).color(t::TEXT_MUTED),
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

        column![
            actions,
            Space::with_height(Length::Fixed(8.0)),
            body,
        ]
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
                self.add_log(format!(
                    "[{:>7.2}s] {}",
                    s.start_ms as f32 / 1000.0,
                    s.text
                ));
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

        let work = async move {
            transcribe(params, tx, cancel_rx)
                .await
                .map_err(|e| e.to_string())
        };

        Task::batch([
            event_stream,
            Task::perform(work, Message::TranscribeFinished),
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
