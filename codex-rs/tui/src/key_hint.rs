use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::text::Span;
use std::fmt::Display;

#[cfg(test)]
const ALT_PREFIX: &str = "⌥";
#[cfg(all(not(test), target_os = "macos"))]
const ALT_PREFIX: &str = "⌥";
#[cfg(all(not(test), not(target_os = "macos")))]
const ALT_PREFIX: &str = "Alt+";

#[cfg(test)]
const CTRL_PREFIX: &str = "⌃";
#[cfg(all(not(test), target_os = "macos"))]
const CTRL_PREFIX: &str = "⌃";
#[cfg(all(not(test), not(target_os = "macos")))]
const CTRL_PREFIX: &str = "Ctrl+";

#[cfg(test)]
const SHIFT_PREFIX: &str = "⇧";
#[cfg(all(not(test), target_os = "macos"))]
const SHIFT_PREFIX: &str = "⇧";
#[cfg(all(not(test), not(target_os = "macos")))]
const SHIFT_PREFIX: &str = "Shift+";

fn key_hint_style() -> Style {
    Style::default().fg(Color::Cyan)
}

fn modifier_span(prefix: &str, key: impl Display) -> Span<'static> {
    Span::styled(format!("{prefix}{key}"), key_hint_style())
}

pub(crate) fn ctrl(key: impl Display) -> Span<'static> {
    modifier_span(CTRL_PREFIX, key)
}

pub(crate) fn alt(key: impl Display) -> Span<'static> {
    modifier_span(ALT_PREFIX, key)
}

pub(crate) fn shift(key: impl Display) -> Span<'static> {
    modifier_span(SHIFT_PREFIX, key)
}

pub(crate) fn plain(key: impl Display) -> Span<'static> {
    Span::styled(format!("{key}"), key_hint_style())
}
