//! Full-screen progress overlay.

use iced::widget::{button, column, container, progress_bar, row, text, Stack, Space};
use iced::{Alignment, Element, Length};

use crate::app::Message;
use crate::theme as t;

pub struct ProgressModalProps<'a> {
    pub title: &'a str,
    pub description: &'a str,
    pub status_message: &'a str,
    pub progress: u8,
    pub current_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub current_item: Option<u64>,
    pub total_items: Option<u64>,
    pub can_cancel: bool,
}

pub fn view<'a>(
    props: ProgressModalProps<'a>,
    behind: Element<'a, Message>,
) -> Element<'a, Message> {
    let mut card = column![
        text(props.title.to_string()).size(18),
        text(truncate(props.description, 80))
            .size(12)
            .color(t::TEXT_MUTED),
        Space::with_height(Length::Fixed(6.0)),
        progress_bar(0.0..=100.0, props.progress as f32).height(Length::Fixed(8.0)),
        text(format!(
            "{}%  ·  {}",
            props.progress,
            props.status_message
        ))
        .size(12)
        .color(t::TEXT_MUTED),
    ]
    .spacing(8);

    if let (Some(c), Some(t_)) = (props.current_bytes, props.total_bytes) {
        card = card.push(
            text(format!("{} / {}", format_bytes(c), format_bytes(t_)))
                .size(11)
                .color(t::TEXT_FAINT),
        );
    }
    if let (Some(c), Some(t_)) = (props.current_item, props.total_items) {
        card = card.push(
            text(format!("File {c} of {t_}")).size(11).color(t::TEXT_FAINT),
        );
    }
    if props.can_cancel {
        card = card.push(Space::with_height(Length::Fixed(6.0)));
        card = card.push(
            row![
                Space::with_width(Length::Fill),
                button(text("Cancel".to_string()).size(12))
                    .padding([6, 14])
                    .on_press(Message::CancelLongTask)
                    .style(t::secondary_button),
            ]
            .align_y(Alignment::Center),
        );
    }

    let card = container(card)
        .style(t::modal_card)
        .padding(22)
        .max_width(440.0);

    let backdrop = container(card)
        .style(t::modal_backdrop)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .width(Length::Fill)
        .height(Length::Fill);

    Stack::new().push(behind).push(backdrop).into()
}

pub fn format_bytes(bytes: u64) -> String {
    const K: f64 = 1024.0;
    if bytes == 0 {
        return "0 B".to_string();
    }
    let units = ["B", "KB", "MB", "GB", "TB"];
    let i = (bytes as f64).log(K).floor() as usize;
    let i = i.min(units.len() - 1);
    let value = bytes as f64 / K.powi(i as i32);
    format!("{value:.1} {}", units[i])
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("…{}", s.chars().rev().take(n).collect::<String>().chars().rev().collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
    }
}
