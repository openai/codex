use super::PrefixedWrappedHistoryCell;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Text;

/// Cyan info bullet with optional dim hint text.
pub(crate) fn new_info_event(message: String, hint: Option<String>) -> PrefixedWrappedHistoryCell {
    let mut line = vec!["• ".dim(), message.into()];
    if let Some(hint) = hint {
        line.push(" ".into());
        line.push(hint.dark_gray());
    }
    PrefixedWrappedHistoryCell::new(Text::from(Line::from(line)), Line::from(""), Line::from(""))
}

/// Yellow warning with a hair-space prefix gap.
#[allow(clippy::disallowed_methods)]
pub(crate) fn new_warning_event(message: String) -> PrefixedWrappedHistoryCell {
    PrefixedWrappedHistoryCell::new(message.yellow(), "⚠ ".yellow(), "  ")
}

/// Red error bullet used for transient error notifications.
pub(crate) fn new_error_event(message: String) -> PrefixedWrappedHistoryCell {
    let line = vec![format!("■ {message}").red()];
    PrefixedWrappedHistoryCell::new(Line::from(line), Line::from(""), Line::from(""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history_cell::HistoryCell;
    use insta::assert_snapshot;

    #[test]
    fn info_event_renders() {
        let cell = new_info_event(
            "Indexed docs are up to date.".to_string(),
            Some("No action needed.".to_string()),
        );

        assert_snapshot!(cell.display_string(42));
    }

    #[test]
    fn warning_event_wraps() {
        let cell = new_warning_event(
            "Retry after reconnecting to VPN so the registry is reachable.".into(),
        );

        assert_snapshot!(cell.display_string(48));
    }

    #[test]
    fn error_event_renders() {
        let cell = new_error_event("Patch apply failed; see stderr for details.".into());

        assert_snapshot!(cell.display_string(50));
    }
}
