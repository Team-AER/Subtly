//! Advanced settings: pipeline knobs, GPU device list, smoke test.

use iced::widget::{button, checkbox, column, container, row, text, text_input, Column, Space};
use iced::{Alignment, Element, Length};

use crate::app::{App, Message};
use crate::theme as t;

pub fn view<'a>(app: &'a App) -> Element<'a, Message> {
    let s = &app.settings;
    let flash_attention_safe = subtly_core::whisper::flash_attention_is_safe();
    let flash_attention_label = if flash_attention_safe {
        "Flash attention (Metal)"
    } else {
        "Flash attention (unavailable on this platform)"
    };
    let flash_attention = checkbox(flash_attention_label, s.flash_attn && flash_attention_safe)
        .size(14)
        .text_size(12);
    let flash_attention = if flash_attention_safe {
        flash_attention.on_toggle(Message::ToggleFlashAttn)
    } else {
        flash_attention
    };

    let runtime = group(
        "Runtime",
        "GPU backend, smoke test, device selection.",
        column![
            row![
                column![
                    label_row(
                        "Backend",
                        text(
                            app.ping_status
                                .as_ref()
                                .map(|p| p.gpu_backend.clone())
                                .unwrap_or_else(|| "—".into())
                        )
                        .size(13)
                        .into(),
                    ),
                    label_row(
                        "Selected device",
                        text(
                            app.selected_device
                                .as_ref()
                                .map(|d| d.name.clone())
                                .unwrap_or_else(|| "—".into())
                        )
                        .size(13)
                        .into(),
                    ),
                ]
                .spacing(6)
                .width(Length::Fill),
                button(text("Run smoke test".to_string()).size(12))
                    .padding([8, 14])
                    .on_press(Message::SmokeTest)
                    .style(t::secondary_button),
            ]
            .align_y(Alignment::Start)
            .spacing(12),
            Space::with_height(Length::Fixed(6.0)),
            device_list(app),
        ]
        .spacing(10)
        .into(),
    );

    let pipeline = group(
        "Whisper pipeline",
        "Decoder, beam search, VAD windowing.",
        column![
            two_col(
                num_field("Threads", s.threads.to_string(), Message::SetThreads),
                num_field("Beam size", s.beam_size.to_string(), Message::SetBeamSize),
            ),
            two_col(
                num_field("Best of", s.best_of.to_string(), Message::SetBestOf),
                num_field(
                    "Max line length",
                    s.max_len_chars.to_string(),
                    Message::SetMaxLenChars,
                ),
            ),
            two_col(
                num_field(
                    "No-speech threshold",
                    s.no_speech_thold.to_string(),
                    Message::SetNoSpeechThold,
                ),
                num_field(
                    "Max context",
                    s.max_context.to_string(),
                    Message::SetMaxContext,
                ),
            ),
            two_col(
                num_field(
                    "Dedup merge gap (sec)",
                    s.dedup_merge_gap_sec.to_string(),
                    Message::SetDedupMergeGapSec,
                ),
                str_field("Language", s.language.clone(), Message::SetLanguage, "auto"),
            ),
            row![
                checkbox("Split on word boundaries", s.split_on_word)
                    .on_toggle(Message::ToggleSplitOnWord)
                    .size(14)
                    .text_size(12),
                checkbox("Translate to English", s.translate)
                    .on_toggle(Message::ToggleTranslate)
                    .size(14)
                    .text_size(12),
                checkbox("Dry run", s.dry_run)
                    .on_toggle(Message::ToggleDryRun)
                    .size(14)
                    .text_size(12),
            ]
            .spacing(20),
            flash_attention,
        ]
        .spacing(10)
        .into(),
    );

    let vad = group(
        "Voice activity detection",
        "Silero VAD windowing parameters.",
        column![
            two_col(
                num_field(
                    "VAD threshold",
                    s.vad_threshold.to_string(),
                    Message::SetVadThreshold,
                ),
                num_field(
                    "Min speech (ms)",
                    s.vad_min_speech_ms.to_string(),
                    Message::SetVadMinSpeechMs,
                ),
            ),
            two_col(
                num_field(
                    "Min silence (ms)",
                    s.vad_min_sil_ms.to_string(),
                    Message::SetVadMinSilMs,
                ),
                num_field("Pad (ms)", s.vad_pad_ms.to_string(), Message::SetVadPadMs),
            ),
        ]
        .spacing(10)
        .into(),
    );

    let cue_layout = group(
        "Subtitle layout",
        "Repackages Whisper's irregular cues into evenly sized subtitle blocks. Needs token timestamps.",
        column![
            row![
                checkbox("Resegment cues for readability", s.resegment_enabled)
                    .on_toggle(Message::ToggleResegmentEnabled)
                    .size(14)
                    .text_size(12),
                checkbox("Token timestamps", s.token_timestamps)
                    .on_toggle(Message::ToggleTokenTimestamps)
                    .size(14)
                    .text_size(12),
            ]
            .spacing(20),
            two_col(
                num_field(
                    "Max cue chars",
                    s.max_cue_chars.to_string(),
                    Message::SetMaxCueChars,
                ),
                num_field(
                    "Max cue duration (ms)",
                    s.max_cue_ms.to_string(),
                    Message::SetMaxCueMs,
                ),
            ),
            num_field(
                "Min cue duration (ms)",
                s.min_cue_ms.to_string(),
                Message::SetMinCueMs,
            ),
        ]
        .spacing(10)
        .into(),
    );

    column![runtime, pipeline, cue_layout, vad]
        .spacing(20)
        .max_width(820.0)
        .into()
}

fn group<'a>(title: &str, hint: &str, body: Element<'a, Message>) -> Element<'a, Message> {
    let header = column![
        text(title.to_string()).size(15),
        text(hint.to_string()).size(11).color(t::TEXT_MUTED),
    ]
    .spacing(2);
    container(column![header, body].spacing(14))
        .style(t::surface)
        .padding(18)
        .width(Length::Fill)
        .into()
}

fn label_row<'a>(label: &str, value: Element<'a, Message>) -> Element<'a, Message> {
    row![
        text(label.to_string())
            .size(11)
            .color(t::TEXT_MUTED)
            .width(Length::Fixed(140.0)),
        value,
    ]
    .align_y(Alignment::Center)
    .spacing(8)
    .into()
}

fn device_list<'a>(app: &'a App) -> Element<'a, Message> {
    if app.devices.is_empty() {
        return text("No GPU devices detected. CPU fallback in use.".to_string())
            .size(12)
            .color(t::TEXT_MUTED)
            .into();
    }
    let mut col: Column<'_, Message> = Column::new().spacing(6);
    col = col.push(
        text("Available devices".to_string())
            .size(11)
            .color(t::TEXT_MUTED),
    );
    for d in &app.devices {
        let selected = app.selected_device.as_ref().map(|s| &s.name) == Some(&d.name);
        let prefix = if selected { "● " } else { "○ " };
        let label = format!("{prefix}{}  ({} · {})", d.name, d.backend, d.device_type);
        col = col.push(
            button(text(label).size(12))
                .padding([6, 10])
                .width(Length::Fill)
                .on_press(Message::SelectDevice(d.clone()))
                .style(if selected {
                    t::secondary_button
                } else {
                    t::ghost_button
                }),
        );
    }
    col.into()
}

fn two_col<'a>(a: Element<'a, Message>, b: Element<'a, Message>) -> Element<'a, Message> {
    row![
        container(a).width(Length::FillPortion(1)),
        container(b).width(Length::FillPortion(1)),
    ]
    .spacing(12)
    .into()
}

fn num_field<'a, F>(label: &str, value: String, on_input: F) -> Element<'a, Message>
where
    F: 'a + Fn(String) -> Message,
{
    column![
        text(label.to_string()).size(11).color(t::TEXT_MUTED),
        text_input("", &value)
            .on_input(on_input)
            .padding(8)
            .style(t::text_input_style),
    ]
    .spacing(4)
    .into()
}

fn str_field<'a, F>(
    label: &str,
    value: String,
    on_input: F,
    placeholder: &str,
) -> Element<'a, Message>
where
    F: 'a + Fn(String) -> Message,
{
    column![
        text(label.to_string()).size(11).color(t::TEXT_MUTED),
        text_input(placeholder, &value)
            .on_input(on_input)
            .padding(8)
            .style(t::text_input_style),
    ]
    .spacing(4)
    .into()
}
