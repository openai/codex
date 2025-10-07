use crate::color::blend;
use crate::color::is_light;
use crate::color::perceptual_distance;
use crate::terminal_palette::default_bg;
use crate::terminal_palette::terminal_palette;
use ratatui::style::Color;
use ratatui::style::Style;

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
    let bottom = terminal_bg;
    let Some(color_level) = supports_color::on_cached(supports_color::Stream::Stdout) else {
        return Color::default();
    };

    let target = blend(top, bottom, 0.1);
    if color_level.has_16m {
        let (r, g, b) = target;
        Color::Rgb(r, g, b)
    } else if color_level.has_256
        && let Some(palette) = terminal_palette()
        && let Some((i, _)) = palette.into_iter().enumerate().min_by(|(_, a), (_, b)| {
            perceptual_distance(*a, target)
                .partial_cmp(&perceptual_distance(*b, target))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    {
        Color::Indexed(i as u8)
    } else {
        Color::default()
    }
}
