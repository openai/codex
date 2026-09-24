//! Task-list checkboxes replace unordered bullets and retain ordered numbering.
//! Keep the hanging indent aligned with the task text, including in loose lists.

use super::Writer;
use ratatui::text::Span;

impl Writer<'_, '_> {
    pub(super) fn task_list_marker(&mut self, checked: bool) {
        let Some(context) = self
            .indent_stack
            .iter_mut()
            .rev()
            .find(|context| context.is_list)
        else {
            return;
        };
        if let Some(marker) = context.marker.as_mut()
            && let Some(span) = marker.last_mut()
        {
            let prefix = span.content.strip_suffix("• ").unwrap_or(&span.content);
            let checkbox = if checked { "☑" } else { "☐" };
            span.content = format!("{prefix}{checkbox} ").into();
            context.prefix = vec![Span::from(" ".repeat(Self::spans_display_width(marker)))];
        }
        if self.current_line_content.is_some() {
            // Loose lists start their paragraph before the task marker arrives.
            self.current_initial_indent = self.prefix_spans(/*pending_marker_line*/ true);
            self.current_subsequent_indent = self.prefix_spans(/*pending_marker_line*/ false);
        }
    }
}
