//! Workspace screen — the primary "transcribe a file" surface.

use iced::widget::{button, column, container, progress_bar, row, text, text_input, Row, Space};
use iced::{Alignment, Element, Length};

use crate::app::{App, Message, Screen};
use crate::theme as t;

pub fn view<'a>(app: &'a App) -> Element<'a, Message> {
    let s = &app.settings;
    let busy = app.is_transcribing;

    let mut pick_file_btn = button(text("Pick file".to_string()).size(13))
        .padding([8, 14])
        .style(t::secondary_button);
    let mut pick_dir_btn = button(text("Pick folder".to_string()).size(13))
        .padding([8, 14])
        .style(t::secondary_button);
    if !busy {
        pick_file_btn = pick_file_btn.on_press(Message::PickFile);
        pick_dir_btn = pick_dir_btn.on_press(Message::PickDir);
    }
    let pickers = row![pick_file_btn, pick_dir_btn].spacing(8);

    let input_display: Element<'_, Message> = if s.input_path.is_empty() {
        text("No input selected".to_string())
            .size(13)
            .color(t::TEXT_FAINT)
            .into()
    } else {
        text(s.input_path.clone()).size(13).color(t::TEXT).into()
    };

    let input_block = section(
        "Input",
        "Pick a media file or a folder to batch-transcribe.",
        column![
            pickers,
            container(input_display)
                .style(t::surface_inset)
                .padding([10, 14])
                .width(Length::Fill),
        ]
        .spacing(10)
        .into(),
    );

    let mut output_input = text_input("Defaults to input file/folder", &s.output_dir)
        .padding(10)
        .style(t::text_input_style);
    if !busy {
        output_input = output_input.on_input(Message::SetOutputDir);
    }
    let output_block = section(
        "Output",
        "Optional. Leave blank to write next to the input file.",
        output_input.into(),
    );

    let formats_block = section(
        "Export formats",
        "Whisper writes every selected format in one pass.",
        format_selector(s, busy).into(),
    );

    // Model & device summary
    let whisper_ok = s
        .selected_model
        .as_deref()
        .map(|id| {
            app.installed
                .iter()
                .any(|m| m.descriptor.id == id && m.complete)
        })
        .unwrap_or(false);
    let vad_ok = app
        .installed
        .iter()
        .any(|m| m.descriptor.id == "silero-vad" && m.complete);
    let model_label = match (&s.selected_model, whisper_ok) {
        (Some(id), true) => format!("Model: {id}"),
        (Some(id), false) => format!("Model {id} not installed"),
        (None, _) => "No model selected".to_string(),
    };
    let device_label = app
        .selected_device
        .as_ref()
        .map(|d| format!("Device: {} ({})", d.name, d.backend))
        .unwrap_or_else(|| "No device".to_string());

    let context_chips = row![
        chip(&model_label, whisper_ok),
        chip(
            if vad_ok {
                "VAD: installed"
            } else {
                "VAD: missing"
            },
            vad_ok,
        ),
        chip(&device_label, app.selected_device.is_some()),
    ]
    .spacing(8)
    .wrap();

    // Action area: either a Generate button + helper, or an inline progress card.
    let action_area: Element<'_, Message> = if let Some(progress) = &app.transcribe_progress {
        progress_card(progress, &s.input_path)
    } else {
        let mut transcribe = button(text("Generate subtitles".to_string()).size(14))
            .padding([12, 22])
            .style(t::primary_button);
        if !s.input_path.is_empty() && whisper_ok && vad_ok {
            transcribe = transcribe.on_press(Message::StartTranscribe);
        }

        let mut helper: Option<Element<'_, Message>> = None;
        if !whisper_ok || !vad_ok {
            helper = Some(
                row![
                    text("⚠".to_string()).size(13).color(t::WARN),
                    text("Models missing — open the Models tab to download.".to_string())
                        .size(13)
                        .color(t::TEXT_MUTED),
                    Space::with_width(Length::Fixed(8.0)),
                    button(text("Open Models".to_string()).size(12))
                        .padding([6, 12])
                        .on_press(Message::NavigateTo(Screen::Models))
                        .style(t::ghost_button),
                ]
                .spacing(8)
                .align_y(Alignment::Center)
                .into(),
            );
        } else if s.input_path.is_empty() {
            helper = Some(
                text("Select an input to enable subtitle generation.".to_string())
                    .size(12)
                    .color(t::TEXT_MUTED)
                    .into(),
            );
        }

        let mut action_col = column![transcribe].spacing(8);
        if let Some(h) = helper {
            action_col = action_col.push(h);
        }
        action_col.into()
    };

    column![
        input_block,
        output_block,
        formats_block,
        context_chips,
        action_area,
    ]
    .spacing(20)
    .max_width(820.0)
    .into()
}

fn progress_card<'a>(
    progress: &'a crate::app::TranscribeProgress,
    input_path: &str,
) -> Element<'a, Message> {
    let header = column![
        text("Generating subtitles".to_string()).size(15),
        text(truncate_middle(input_path, 90))
            .size(11)
            .color(t::TEXT_FAINT),
    ]
    .spacing(2);

    let bar = progress_bar(0.0..=100.0, progress.progress as f32).height(Length::Fixed(8.0));

    let mut status_line = format!("{}%  ·  {}", progress.progress, progress.status_message);
    if let (Some(c), Some(t_)) = (progress.current_item, progress.total_items) {
        if t_ > 1 {
            status_line.push_str(&format!("  ·  File {c} of {t_}"));
        }
    }

    let mut cancel_btn = button(
        text(if progress.cancelling {
            "Cancelling…"
        } else {
            "Cancel"
        })
        .size(12),
    )
    .padding([6, 14])
    .style(t::secondary_button);
    if !progress.cancelling {
        cancel_btn = cancel_btn.on_press(Message::CancelTranscribe);
    }

    let footer = row![
        text(status_line).size(12).color(t::TEXT_MUTED),
        Space::with_width(Length::Fill),
        button(text("View activity".to_string()).size(12))
            .padding([6, 12])
            .on_press(Message::NavigateTo(Screen::Logs))
            .style(t::ghost_button),
        cancel_btn,
    ]
    .align_y(Alignment::Center)
    .spacing(8);

    container(column![header, bar, footer].spacing(10))
        .style(t::surface)
        .padding(18)
        .width(Length::Fill)
        .into()
}

fn truncate_middle(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let tail: String = s.chars().rev().take(n).collect::<Vec<_>>().into_iter().rev().collect();
    format!("…{tail}")
}

fn section<'a>(title: &str, hint: &str, body: Element<'a, Message>) -> Element<'a, Message> {
    let header = column![
        text(title.to_string()).size(15),
        text(hint.to_string()).size(11).color(t::TEXT_MUTED),
    ]
    .spacing(2);

    container(column![header, body].spacing(12))
        .style(t::surface)
        .padding(18)
        .width(Length::Fill)
        .into()
}

fn chip<'a>(label: &str, active: bool) -> Element<'a, Message> {
    let color = if active { t::TEXT } else { t::WARN };
    container(text(label.to_string()).size(12).color(color))
        .style(t::surface_inset)
        .padding([6, 12])
        .into()
}

fn format_selector<'a>(s: &subtly_core::settings::Settings, busy: bool) -> Row<'a, Message> {
    let formats = [
        ("srt", "SRT"),
        ("vtt", "VTT"),
        ("txt", "Text"),
        ("json", "JSON"),
        ("csv", "CSV"),
    ];
    let mut row_widget: Row<'_, Message> = Row::new().spacing(6);
    for (id, label) in formats {
        let selected = s.export_formats.iter().any(|f| f == id);
        let prefix = if selected { "✓ " } else { "  " };
        let color = if selected { t::TEXT } else { t::TEXT_MUTED };
        let mut btn = button(text(format!("{prefix}{label}")).size(12).color(color))
            .padding([6, 12])
            .style(if selected {
                t::secondary_button
            } else {
                t::ghost_button
            });
        if !busy {
            btn = btn.on_press(Message::ToggleExportFormat(id.to_string()));
        }
        row_widget = row_widget.push(btn);
    }
    row_widget
}
