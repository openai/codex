use super::HistoryCell;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_lines;
use ratatui::style::Stylize;
use ratatui::text::Line;

/// Agent-produced lines shown in the scrolling history pane after user input or tool output.
///
/// Displays assistant replies with a dim bullet on the first line and a hanging indent on wrapped
/// lines so finalized responses match the streamed appearance in the transcript view.
///
/// # Output
///
/// ```plain
/// • hello world from the agent reply
/// ```
#[derive(Debug)]
pub(crate) struct AgentMessageCell {
    pub(crate) lines: Vec<Line<'static>>,
    pub(crate) is_first_line: bool,
}

impl AgentMessageCell {
    /// Create a new agent message block, marking whether this is the first line.
    ///
    /// `is_first_line` controls whether a bullet is shown; continuations keep the same indent but
    /// omit the bullet so multi-line answers look like a single paragraph.
    pub(crate) fn new(lines: Vec<Line<'static>>, is_first_line: bool) -> Self {
        Self {
            lines,
            is_first_line,
        }
    }
}

impl HistoryCell for AgentMessageCell {
    /// Wrap agent text to the available width with a bullet then hanging indent.
    ///
    /// Uses `word_wrap_lines`, prefixed with a dim bullet on the first line and two-space indent on
    /// following lines so agent replies are visually grouped in the history list.
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        word_wrap_lines(
            &self.lines,
            RtOptions::new(width as usize)
                .initial_indent(if self.is_first_line {
                    "• ".dim().into()
                } else {
                    "  ".into()
                })
                .subsequent_indent("  ".into()),
        )
    }

    /// Treat follow-up lines as stream continuations so separators are avoided.
    fn is_stream_continuation(&self) -> bool {
        !self.is_first_line
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn empty_transcript_line() {
        let cell = AgentMessageCell::new(vec![Line::default()], false);
        assert_eq!(cell.transcript_lines(80), vec![Line::from("  ")]);
        assert_eq!(cell.desired_transcript_height(80), 1);
    }

    #[test]
    fn wraps_with_hanging_indent() {
        let cell = AgentMessageCell::new(
            vec![Line::from(
                "Here is how to fix the failing tests by adjusting the mock responses.",
            )],
            true,
        );

        assert_snapshot!(cell.display_string(34));
    }

    #[test]
    fn continuation_line_omits_bullet() {
        let cell = AgentMessageCell::new(
            vec![Line::from("Then continue streaming without a new bullet.")],
            false,
        );

        assert_snapshot!(cell.display_string(32));
    }
}
