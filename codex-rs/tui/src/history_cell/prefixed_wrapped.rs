use super::HistoryCell;
use crate::render::line_utils::push_owned_lines;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_lines;
use ratatui::text::Line;
use ratatui::text::Text;

/// A wrapped text block with distinct prefixes for the first and subsequent lines.
///
/// Callers supply text and prefixes; the cell handles wrapping and prefixing for approval banners,
/// warnings, and other note-style entries.
///
/// # Output
///
/// ```plain
/// ✔ You approved codex
///   to run echo ...
/// ```
#[derive(Debug)]
pub(crate) struct PrefixedWrappedHistoryCell {
    pub(crate) text: Text<'static>,
    pub(crate) initial_prefix: Line<'static>,
    pub(crate) subsequent_prefix: Line<'static>,
}

impl PrefixedWrappedHistoryCell {
    /// Construct a prefix-aware wrapped cell.
    ///
    /// Callers provide the content plus distinct prefixes for the first and subsequent lines;
    /// wrapping is handled here so long notes and banners keep a consistent hanging indent.
    pub(crate) fn new(
        text: impl Into<Text<'static>>,
        initial_prefix: impl Into<Line<'static>>,
        subsequent_prefix: impl Into<Line<'static>>,
    ) -> Self {
        Self {
            text: text.into(),
            initial_prefix: initial_prefix.into(),
            subsequent_prefix: subsequent_prefix.into(),
        }
    }
}

impl HistoryCell for PrefixedWrappedHistoryCell {
    /// Wrap text and apply distinct prefixes on the first vs subsequent lines.
    ///
    /// Uses `word_wrap_lines` to honor the available width, then emits the caller-provided
    /// `initial_prefix` on the first line and `subsequent_prefix` on following lines. Useful for
    /// note-style blocks like approvals and warnings that need a consistent hanging indent.
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        if width == 0 {
            return Vec::new();
        }
        let opts = RtOptions::new(width.max(1) as usize)
            .initial_indent(self.initial_prefix.clone())
            .subsequent_indent(self.subsequent_prefix.clone());
        let wrapped = word_wrap_lines(&self.text, opts);
        let mut out = Vec::new();
        push_owned_lines(&wrapped, &mut out);
        out
    }

    /// Measure by counting wrapped lines.
    fn desired_height(&self, width: u16) -> u16 {
        self.display_lines(width).len() as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;
    use ratatui::style::Stylize;

    #[test]
    fn indents_wrapped_lines() {
        let summary = Line::from(vec![
            "You ".into(),
            "approved".bold(),
            " codex to run ".into(),
            "echo something really long to ensure wrapping happens".dim(),
            " this time".bold(),
        ]);
        let cell = PrefixedWrappedHistoryCell::new(summary, "✔ ".green(), "  ");
        let rendered = cell.display_string(24);
        assert_eq!(
            rendered,
            indoc::indoc! {"
                ✔ You approved codex
                  to run echo something
                  really long to ensure
                  wrapping happens this
                  time"}
        );
    }

    #[test]
    fn warning_message_wraps_with_hanging_indent() {
        let cell = PrefixedWrappedHistoryCell::new(
            Text::from(
                "Warning: reconnect to the VPN before retrying so the service endpoint is reachable.",
            ),
            #[expect(clippy::disallowed_methods)]
            "⚠ ".yellow(),
            "  ",
        );

        assert_snapshot!(cell.display_string(38));
    }
}
