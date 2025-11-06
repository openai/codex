use ratatui::style::{Color, Style};

use crate::color::{blend, is_light};
use crate::terminal_palette::{best_color, default_bg};

pub fn user_message_style() -> Style {
    user_message_style_for(default_bg())
}

/// Returns the style for a user-authored message using the provided terminal background.
pub fn user_message_style_for(terminal_bg: Option<(u8, u8, u8)>) -> Style {
    match terminal_bg {
        Some(bg) => Style::default().bg(user_message_bg(bg)),
        None => Style::default(),
    }
}

#[allow(clippy::disallowed_methods)]
pub fn user_message_bg(terminal_bg: (u8, u8, u8)) -> Color {
    let top = if is_light(terminal_bg) {
        (0, 0, 0)
    } else {
        (255, 255, 255)
    };
    best_color(blend(top, terminal_bg, 0.1))
}
