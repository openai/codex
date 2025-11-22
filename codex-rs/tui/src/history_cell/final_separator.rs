use super::HistoryCell;
use crate::status_indicator_widget;
use ratatui::style::Stylize;
use ratatui::text::Line;
use unicode_width::UnicodeWidthStr;

/// Divider shown before the final agent message.
///
/// Draws a horizontal rule and optionally appends a compact elapsed-time summary to show how long
/// the session ran. Used at the end of the history transcript to separate the concluding response
/// from prior activity.
///
/// # Output
///
/// ```plain
/// ─ Worked for 2m14s ──────
/// ```
#[derive(Debug)]
pub struct FinalMessageSeparator {
    elapsed_seconds: Option<u64>,
}

impl FinalMessageSeparator {
    pub(crate) fn new(elapsed_seconds: Option<u64>) -> Self {
        Self { elapsed_seconds }
    }
}

impl HistoryCell for FinalMessageSeparator {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let elapsed_seconds = self
            .elapsed_seconds
            .map(status_indicator_widget::fmt_elapsed_compact);
        if let Some(elapsed_seconds) = elapsed_seconds {
            let worked_for = format!("─ Worked for {elapsed_seconds} ─");
            let worked_for_width = worked_for.width();
            vec![
                Line::from_iter([
                    worked_for,
                    "─".repeat((width as usize).saturating_sub(worked_for_width)),
                ])
                .dim(),
            ]
        } else {
            vec![Line::from_iter(["─".repeat(width as usize).dim()])]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn renders_with_elapsed() {
        let sep = FinalMessageSeparator::new(Some(134));
        let rendered = sep.display_string(32);

        assert!(rendered.contains("Worked for"));
        assert!(rendered.starts_with('─'));
    }

    #[test]
    fn renders_simple_rule_when_no_elapsed() {
        let sep = FinalMessageSeparator::new(None);
        let rendered = sep.display_string(20);

        assert_eq!(rendered, "────────────────────");
    }

    #[test]
    fn separator_with_elapsed() {
        let sep = FinalMessageSeparator::new(Some(372));

        assert_snapshot!(sep.display_string(40));
    }

    #[test]
    fn separator_without_elapsed() {
        let sep = FinalMessageSeparator::new(None);

        assert_snapshot!(sep.display_string(34));
    }
}
