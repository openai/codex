use crate::color::blend;
use crate::color::is_light;
use crate::terminal_palette::best_color;
use crate::theme::Theme;
use ratatui::style::Color;
use ratatui::style::Style;

/// Get the current theme. This can be overridden by setting a thread-local theme.
pub fn current_theme() -> Theme {
    THEME.with(|t| t.borrow().clone())
}

/// Set the current theme for this thread
pub fn set_current_theme(theme: Theme) {
    THEME.with(|t| *t.borrow_mut() = theme);
}

thread_local! {
    static THEME: std::cell::RefCell<Theme> = std::cell::RefCell::new(Theme::default());
}

pub fn user_message_style() -> Style {
    let theme = current_theme();
    Style::default().bg(theme.user_message_bg())
}

pub fn assistant_message_style() -> Style {
    let theme = current_theme();
    match theme.assistant_message_bg() {
        Some(color) => Style::default().bg(color),
        None => Style::default(),
    }
}

/// Returns the style for a user-authored message using the provided terminal background.
/// This function is kept for backwards compatibility but now uses the theme system.
#[allow(dead_code)]
pub fn user_message_style_for(terminal_bg: Option<(u8, u8, u8)>) -> Style {
    match terminal_bg {
        Some(bg) => Style::default().bg(user_message_bg(bg)),
        None => Style::default(),
    }
}

/// Legacy function - kept for backwards compatibility
#[allow(dead_code, clippy::disallowed_methods)]
pub fn user_message_bg(terminal_bg: (u8, u8, u8)) -> Color {
    let top = if is_light(terminal_bg) {
        (0, 0, 0)
    } else {
        (255, 255, 255)
    };
    best_color(blend(top, terminal_bg, 0.1))
}
