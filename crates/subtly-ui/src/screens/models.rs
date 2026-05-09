//! Model manager screen.

use iced::widget::{button, column, container, progress_bar, row, text, Column, Space};
use iced::{Alignment, Element, Length};

use subtly_core::models::{ModelDescriptor, VAD_MODEL, WHISPER_MODELS};

use crate::app::{App, Message};
use crate::theme as t;

pub fn view<'a>(app: &'a App) -> Element<'a, Message> {
    let intro = column![
        text("Models".to_string()).size(20),
        text(
            "Larger Whisper models are more accurate but slower. \
             VAD is required for all transcriptions."
                .to_string()
        )
        .size(12)
        .color(t::TEXT_MUTED),
    ]
    .spacing(4);

    // VAD requirement (single highlighted card).
    let vad_card = model_card(app, &VAD_MODEL, true);

    // Whisper models grid.
    let mut whisper_section: Column<'_, Message> = column![
        text("Whisper models".to_string()).size(15),
        text("Pick one as the active transcription model.".to_string())
            .size(12)
            .color(t::TEXT_MUTED),
    ]
    .spacing(4);

    for m in WHISPER_MODELS {
        whisper_section = whisper_section.push(model_card(app, m, false));
    }

    column![intro, vad_card, whisper_section]
        .spacing(20)
        .max_width(820.0)
        .into()
}

fn model_card<'a>(
    app: &'a App,
    descriptor: &'a ModelDescriptor,
    is_vad: bool,
) -> Element<'a, Message> {
    let installed = app
        .installed
        .iter()
        .find(|m| m.descriptor.id == descriptor.id && m.complete);
    let downloading = app.downloading.get(descriptor.id);
    let selected = !is_vad && app.settings.selected_model.as_deref() == Some(descriptor.id);

    // Header
    let mut header = row![
        column![
            row![
                text(descriptor.name.to_string()).size(15),
                if descriptor.recommended {
                    pill("Recommended", t::ACCENT)
                } else {
                    Space::new(Length::Fixed(0.0), Length::Fixed(0.0)).into()
                },
                if selected {
                    pill("Selected", t::SUCCESS)
                } else if installed.is_some() {
                    pill("Installed", t::SUCCESS)
                } else {
                    Space::new(Length::Fixed(0.0), Length::Fixed(0.0)).into()
                }
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            text(descriptor.description.to_string())
                .size(12)
                .color(t::TEXT_MUTED),
            text(descriptor.size.to_string())
                .size(11)
                .color(t::TEXT_FAINT),
        ]
        .spacing(2)
        .width(Length::Fill),
    ]
    .align_y(Alignment::Start)
    .spacing(12);

    // Progress / actions
    let actions: Element<'_, Message> = if let Some(progress) = downloading {
        column![
            progress_bar(0.0..=100.0, progress.progress as f32).height(Length::Fixed(8.0)),
            text(format!(
                "{}% — {} of {}",
                progress.progress,
                crate::widgets::progress_modal::format_bytes(progress.downloaded_bytes),
                crate::widgets::progress_modal::format_bytes(progress.total_bytes)
            ))
            .size(11)
            .color(t::TEXT_MUTED),
        ]
        .spacing(6)
        .width(Length::Fixed(280.0))
        .into()
    } else if installed.is_some() {
        let mut btns = row![].spacing(6);
        if !is_vad && !selected {
            btns = btns.push(
                button(text("Select".to_string()).size(12))
                    .padding([6, 12])
                    .on_press(Message::SelectModel(descriptor.id.to_string()))
                    .style(t::secondary_button),
            );
        }
        btns = btns.push(
            button(text("Delete".to_string()).size(12))
                .padding([6, 12])
                .on_press(Message::DeleteModel(descriptor.id.to_string()))
                .style(t::danger_button),
        );
        btns.into()
    } else {
        button(text("Download".to_string()).size(12))
            .padding([8, 14])
            .on_press(Message::StartDownload(descriptor.id.to_string()))
            .style(t::primary_button)
            .into()
    };

    header = header.push(container(actions).align_right(Length::Shrink));

    container(header)
        .style(t::surface)
        .padding(16)
        .width(Length::Fill)
        .into()
}

fn pill<'a>(label: &str, color: iced::Color) -> Element<'a, Message> {
    container(text(label.to_string()).size(10).color(color))
        .padding([2, 8])
        .style(move |_theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(iced::Color {
                a: 0.15,
                ..color
            })),
            border: iced::Border {
                color: iced::Color { a: 0.4, ..color },
                width: 1.0,
                radius: iced::border::Radius::new(10.0),
            },
            text_color: Some(color),
            ..Default::default()
        })
        .into()
}
