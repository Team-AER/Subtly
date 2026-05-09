//! Custom Iced theme + reusable widget styles.

use iced::border::Radius;
use iced::theme::{Custom, Palette};
use iced::widget::{button, container, scrollable, text_input};
use iced::{color, Background, Border, Color, Shadow, Theme};
use std::sync::Arc;

// Palette ------------------------------------------------------------------

pub const BG: Color = color!(0x0a, 0x0e, 0x17);
pub const SURFACE: Color = color!(0x12, 0x18, 0x25);
pub const SURFACE_2: Color = color!(0x18, 0x20, 0x32);
pub const SURFACE_HOVER: Color = color!(0x1f, 0x29, 0x3f);
pub const BORDER: Color = color!(0x29, 0x33, 0x4a);
pub const BORDER_STRONG: Color = color!(0x3a, 0x47, 0x63);
pub const TEXT: Color = color!(0xe5, 0xea, 0xf2);
pub const TEXT_MUTED: Color = color!(0x98, 0xa3, 0xb8);
pub const TEXT_FAINT: Color = color!(0x68, 0x73, 0x88);
pub const ACCENT: Color = color!(0x6c, 0x8c, 0xff);
pub const ACCENT_DIM: Color = color!(0x37, 0x4d, 0xa3);
pub const SUCCESS: Color = color!(0x4a, 0xd6, 0x95);
pub const WARN: Color = color!(0xfb, 0xb6, 0x4f);
pub const DANGER: Color = color!(0xf6, 0x73, 0x73);

pub fn subtly_theme() -> Theme {
    let palette = Palette {
        background: BG,
        text: TEXT,
        primary: ACCENT,
        success: SUCCESS,
        danger: DANGER,
    };
    Theme::Custom(Arc::new(Custom::new("Subtly".into(), palette)))
}

// Container styles ---------------------------------------------------------

pub fn surface(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(SURFACE)),
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: Radius::new(10.0),
        },
        text_color: Some(TEXT),
        shadow: Shadow::default(),
    }
}

pub fn surface_inset(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG)),
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: Radius::new(8.0),
        },
        text_color: Some(TEXT),
        shadow: Shadow::default(),
    }
}

pub fn sidebar(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(SURFACE)),
        border: Border {
            color: BORDER,
            width: 0.0,
            radius: Radius::new(0.0),
        },
        text_color: Some(TEXT),
        shadow: Shadow::default(),
    }
}

pub fn modal_backdrop(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color {
            a: 0.65,
            ..Color::BLACK
        })),
        ..Default::default()
    }
}

pub fn modal_card(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(SURFACE)),
        border: Border {
            color: BORDER_STRONG,
            width: 1.0,
            radius: Radius::new(14.0),
        },
        text_color: Some(TEXT),
        shadow: Shadow {
            color: Color {
                a: 0.45,
                ..Color::BLACK
            },
            offset: iced::Vector::new(0.0, 8.0),
            blur_radius: 32.0,
        },
    }
}

pub fn status_dot_ok(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(SUCCESS)),
        border: Border {
            radius: Radius::new(8.0),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn status_dot_warn(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(WARN)),
        border: Border {
            radius: Radius::new(8.0),
            ..Default::default()
        },
        ..Default::default()
    }
}

// Button styles ------------------------------------------------------------

pub fn primary_button(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => Color {
            r: ACCENT.r * 1.1,
            g: ACCENT.g * 1.1,
            b: ACCENT.b * 1.1,
            a: ACCENT.a,
        },
        button::Status::Pressed => ACCENT_DIM,
        button::Status::Disabled => Color {
            a: 0.4,
            ..ACCENT
        },
        _ => ACCENT,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: Color::WHITE,
        border: Border {
            radius: Radius::new(6.0),
            ..Default::default()
        },
        shadow: Shadow::default(),
    }
}

pub fn secondary_button(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => SURFACE_HOVER,
        button::Status::Pressed => SURFACE_2,
        _ => SURFACE_2,
    };
    let text_color = match status {
        button::Status::Disabled => TEXT_FAINT,
        _ => TEXT,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color,
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: Radius::new(6.0),
        },
        shadow: Shadow::default(),
    }
}

pub fn ghost_button(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => SURFACE_HOVER,
        _ => Color::TRANSPARENT,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: TEXT_MUTED,
        border: Border {
            radius: Radius::new(6.0),
            ..Default::default()
        },
        shadow: Shadow::default(),
    }
}

pub fn nav_button(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let (bg, text_color) = if active {
            (SURFACE_HOVER, TEXT)
        } else {
            match status {
                button::Status::Hovered => (SURFACE_HOVER, TEXT),
                _ => (Color::TRANSPARENT, TEXT_MUTED),
            }
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color,
            border: Border {
                color: if active { ACCENT_DIM } else { Color::TRANSPARENT },
                width: if active { 1.0 } else { 0.0 },
                radius: Radius::new(6.0),
            },
            shadow: Shadow::default(),
        }
    }
}

pub fn danger_button(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => Color {
            a: 0.18,
            ..DANGER
        },
        _ => Color::TRANSPARENT,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: DANGER,
        border: Border {
            color: Color {
                a: 0.4,
                ..DANGER
            },
            width: 1.0,
            radius: Radius::new(6.0),
        },
        shadow: Shadow::default(),
    }
}

// Input ---------------------------------------------------------------------

pub fn text_input_style(_theme: &Theme, status: text_input::Status) -> text_input::Style {
    let border_color = match status {
        text_input::Status::Focused { .. } => ACCENT,
        text_input::Status::Hovered => BORDER_STRONG,
        _ => BORDER,
    };
    text_input::Style {
        background: Background::Color(BG),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: Radius::new(6.0),
        },
        icon: TEXT_MUTED,
        placeholder: TEXT_FAINT,
        value: TEXT,
        selection: ACCENT_DIM,
    }
}

// Scrollable ---------------------------------------------------------------

pub fn scrollable_style(_theme: &Theme, _status: scrollable::Status) -> scrollable::Style {
    let rail = scrollable::Rail {
        background: Some(Background::Color(Color::TRANSPARENT)),
        border: Border::default(),
        scroller: scrollable::Scroller {
            color: BORDER_STRONG,
            border: Border {
                radius: Radius::new(4.0),
                ..Default::default()
            },
        },
    };
    scrollable::Style {
        container: container::Style::default(),
        vertical_rail: rail.clone(),
        horizontal_rail: rail,
        gap: None,
    }
}
