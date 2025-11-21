use super::HistoryCell;
use ratatui::text::Line;

/// Preformatted content to drop directly into the history.
///
/// Used for banners, summaries, and helper lines that already include styling/indentation so they
/// should be passed through unchanged.
#[derive(Debug)]
pub(crate) struct PlainHistoryCell {
    pub(crate) lines: Vec<Line<'static>>,
}

impl PlainHistoryCell {
    /// Wrap the given lines in a pass-through cell.
    pub(crate) fn new(lines: Vec<Line<'static>>) -> Self {
        Self { lines }
    }
}

impl HistoryCell for PlainHistoryCell {
    /// Return the provided lines without modification.
    ///
    /// Width is ignored because callers pre-wrap/pre-style the content before constructing this
    /// cell.
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        self.lines.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn passes_through_lines_unchanged() {
        let lines = vec![Line::from("Hello"), Line::from("world")];
        let cell = PlainHistoryCell::new(lines.clone());

        assert_eq!(cell.display_lines(10), lines);
        assert_eq!(cell.desired_height(10), 2);
    }

    #[test]
    fn transcript_height_treats_whitespace_as_single_line() {
        let cell = PlainHistoryCell::new(vec![Line::from(" ")]);

        assert_eq!(cell.desired_transcript_height(24), 1);
    }

    #[test]
    fn renders_multiline_plain_text() {
        let cell = PlainHistoryCell::new(vec![
            Line::from("Summary: Updated the deployment script."),
            Line::from("Details: Added retries and improved logging output."),
        ]);

        assert_snapshot!(cell.display_string(60));
    }
}
