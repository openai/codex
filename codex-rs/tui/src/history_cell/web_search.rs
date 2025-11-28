use super::HistoryCell;
use super::padded_emoji;
use ratatui::text::Line;
use textwrap::wrap;
use unicode_width::UnicodeWidthStr;

/// Agent-issued web search entry shown in the history list.
///
/// Used when the agent triggers the web search tool so users can see the outbound query. Displays a
/// padded globe emoji followed by the query text. Wrapped lines align under the text, preserving
/// the emoji width so searches stand out without adding extra padding.
///
/// # Output
///
/// ```plain
/// 🌐 find ratatui styling
///    tips for codex tui
///    with wrapping
/// ```
#[derive(Debug)]
pub(crate) struct WebSearchCallCell {
    query: String,
}

impl WebSearchCallCell {
    /// Create a web search cell for the given outbound query text.
    pub(crate) fn new(query: String) -> Self {
        Self { query }
    }
}

impl HistoryCell for WebSearchCallCell {
    /// Render the query with a globe prefix and wrapped continuation lines.
    ///
    /// The globe is followed by a hair space so the emoji doesn’t crowd the text. Wrapped lines are
    /// indented by the emoji width to maintain alignment, and the text is wrapped against the
    /// available width minus the prefix.
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let prefix = padded_emoji("🌐");
        let prefix_width = UnicodeWidthStr::width(prefix.as_str());
        let wrap_width = usize::from(width).saturating_sub(prefix_width).max(1);
        let wrapped = wrap(&self.query, wrap_width);
        let mut lines: Vec<Line<'static>> = Vec::new();
        for (idx, segment) in wrapped.into_iter().enumerate() {
            if idx == 0 {
                lines.push(Line::from(format!("{prefix}{segment}")));
            } else {
                lines.push(Line::from(format!("{}{segment}", " ".repeat(prefix_width))));
            }
        }
        lines
    }
}

/// Factory hook used by the module re-export to build a web search cell.
pub(crate) fn new_web_search_call(query: String) -> WebSearchCallCell {
    WebSearchCallCell::new(query)
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn formats_query_with_globe_prefix() {
        let cell = WebSearchCallCell::new("two words".into());

        assert_eq!(
            cell.display_string(80),
            format!("{}two words", padded_emoji("🌐"))
        );
    }

    #[test]
    fn snapshot_wide() {
        let cell =
            WebSearchCallCell::new("find ratatui styling tips for codex tui with wrapping".into());
        assert_snapshot!(cell.display_string(80));
    }

    #[test]
    fn snapshot_narrow() {
        let cell =
            WebSearchCallCell::new("find ratatui styling tips for codex tui with wrapping".into());
        assert_snapshot!(cell.display_string(24));
    }
}
