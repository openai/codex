use super::HistoryCell;
use ratatui::text::Line;

/// Concatenates several child cells into a single history entry.
///
/// Used for multi-part rows like the session header plus onboarding hints so the history shows a
/// single item with intentional spacing. Each child renders with the provided width; non-empty
/// parts are joined with a blank line separator to preserve breathing room without the caller
/// managing padding manually.
#[derive(Debug)]
pub(crate) struct CompositeHistoryCell {
    pub(crate) parts: Vec<Box<dyn HistoryCell>>,
}

impl CompositeHistoryCell {
    /// Build a composite from pre-renderable child cells.
    ///
    /// Empty children are skipped; adjacent non-empty children are separated by a blank line to
    /// keep their blocks visually distinct while staying inside one history slot.
    pub(crate) fn new(parts: Vec<Box<dyn HistoryCell>>) -> Self {
        Self { parts }
    }
}

impl HistoryCell for CompositeHistoryCell {
    /// Render each child and join them with blank lines to form one entry.
    ///
    /// Children render at the provided width; empty children are elided and non-empty neighbors are
    /// separated by a single blank line to preserve their padding without extra caller logic.
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut out: Vec<Line<'static>> = Vec::new();
        let mut first = true;
        for part in &self.parts {
            let mut lines = part.display_lines(width);
            if !lines.is_empty() {
                if !first {
                    out.push(Line::from(""));
                }
                out.append(&mut lines);
                first = false;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history_cell::PlainHistoryCell;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;

    #[test]
    fn joins_non_empty_parts_with_blank_line() {
        let composite = CompositeHistoryCell::new(vec![
            Box::new(PlainHistoryCell::new(vec![Line::from("first")])),
            Box::new(PlainHistoryCell::new(Vec::new())),
            Box::new(PlainHistoryCell::new(vec![Line::from("second")])),
        ]);

        assert_eq!(composite.display_string(80), "first\n\nsecond");
    }

    #[test]
    fn snapshot_with_wrapped_children() {
        let composite = CompositeHistoryCell::new(vec![
            Box::new(PlainHistoryCell::new(vec![Line::from(
                "Session header: OpenAI Codex (v1.2)",
            )])),
            Box::new(PlainHistoryCell::new(vec![Line::from(
                "Help: Press ? to see keyboard shortcuts for navigating history.",
            )])),
        ]);

        assert_snapshot!(composite.display_string(48));
    }
}
