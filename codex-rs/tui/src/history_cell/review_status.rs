use super::HistoryCell;
use ratatui::style::Stylize;
use ratatui::text::Line;

/// Single review status line rendered in cyan.
///
/// Shown in the history when a review op is in progress or completed. The message is pre-styled and
/// unwrapped, so it stays compact within the available width.
///
/// # Output
///
/// ```plain
/// • Review approved
/// ```
#[derive(Debug)]
pub(crate) struct ReviewStatusCell {
    message: String,
}

impl ReviewStatusCell {
    pub(crate) fn new(message: String) -> Self {
        Self { message }
    }
}

impl HistoryCell for ReviewStatusCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        vec![Line::from(self.message.clone().cyan())]
    }
}

pub(crate) fn new_review_status_line(message: String) -> ReviewStatusCell {
    ReviewStatusCell::new(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn renders_cyan_message() {
        let cell = ReviewStatusCell::new("Review status".into());

        assert_eq!(cell.display_string(80), "Review status");
    }

    #[test]
    fn long_status_message_renders() {
        let cell = ReviewStatusCell::new(
            "Review in progress: applying feedback and rerunning checks".into(),
        );

        assert_snapshot!(cell.display_string(64));
    }
}
