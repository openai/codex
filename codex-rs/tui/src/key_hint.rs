use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Span;
use std::fmt::Display;

#[cfg(test)]
const ALT_PREFIX: &str = "⌥";
#[cfg(all(not(test), target_os = "macos"))]
const ALT_PREFIX: &str = "⌥";
#[cfg(all(not(test), not(target_os = "macos")))]
const ALT_PREFIX: &str = "Alt+";

fn key_hint_style() -> Style {
    Style::default().bold()
}

fn modifier_span(prefix: &str, key: impl Display) -> Span<'static> {
    Span::styled(format!("{prefix}{key}"), key_hint_style())
}

pub(crate) fn alt(key: impl Display) -> Span<'static> {
    modifier_span(ALT_PREFIX, key)
}
