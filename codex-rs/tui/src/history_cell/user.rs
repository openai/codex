use super::HistoryCell;
use crate::render::line_utils::prefix_lines;
use crate::style::user_message_style;
use crate::ui_consts::LIVE_PREFIX_COLS;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_lines;
use ratatui::style::Stylize;
use ratatui::text::Line;

/// Represents the text a human typed into the chat composer.
///
/// Used in the scrolling history view to show user input with a distinct prefix and background so
/// it’s easy to tell apart from agent replies and tool output.
///
/// # Output
///
/// ```plain
///
/// › hello there
///   wrapped lines continue here
///
/// ```
#[derive(Debug)]
pub(crate) struct UserHistoryCell {
    pub message: String,
}

/// Construct a user history entry from the raw chat input text.
///
/// Allows callers to pass the message directly without knowing about the cell internals; the render
/// preserves the shaded background and `›` prefix that distinguish user input in the history list.
pub(crate) fn new_user_prompt(message: String) -> UserHistoryCell {
    UserHistoryCell { message }
}

impl HistoryCell for UserHistoryCell {
    /// Render the user message with background shading, leading `›`, and vertical padding.
    ///
    /// The content is wrapped to `width - LIVE_PREFIX_COLS - 1` (keeping a single-column right
    /// margin). A blank line is inserted above and below, all styled with `user_message_style` to
    /// give a subtle background. Wrapped lines are prefixed with a dim bold `› ` on the first line
    /// and two spaces on subsequent lines, preserving a hanging indent so the block reads like a
    /// quoted user prompt inside the history list.
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();

        let wrap_width = width
            .saturating_sub(
                LIVE_PREFIX_COLS + 1, /* keep a one-column right margin for wrapping */
            )
            .max(1);

        let style = user_message_style();

        let wrapped = word_wrap_lines(
            self.message.lines().map(|l| Line::from(l).style(style)),
            // Wrap algorithm matches textarea.rs.
            RtOptions::new(usize::from(wrap_width))
                .wrap_algorithm(textwrap::WrapAlgorithm::FirstFit),
        );

        lines.push(Line::from("").style(style));
        lines.extend(prefix_lines(wrapped, "› ".bold().dim(), "  ".into()));
        lines.push(Line::from("").style(style));
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_and_prefixes_each_line_snapshot() {
        let msg = "one two three four five six seven";
        let cell = UserHistoryCell {
            message: msg.to_string(),
        };

        // Small width to force wrapping more clearly. Effective wrap width is width-2 due to the ▌
        // prefix and trailing space.
        let width: u16 = 12;
        let rendered = cell.display_string(width);

        insta::assert_snapshot!(rendered);
    }
}
