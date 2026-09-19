//! Packs footer hints into rows without separating a shortcut from its label.
//! Shared tips style keyboard tokens separately from their labels; callers own clipping.

use crate::style::secondary_text_style;
use ratatui::style::Styled;
use ratatui::text::Line;
/// A complete hint uses the same styled content for measurement and rendering.
pub(crate) fn shortcut(keys: &str, label: &str) -> Line<'static> {
    let mut spans = crate::key_hint::key_label_spans(keys);
    spans.push(format!(" {label}").set_style(secondary_text_style()));
    Line::from(spans)
}

/// Choose a complete hint variant so narrow surfaces never display half a shortcut.
#[allow(dead_code, reason = "Used by later layers of the TUI refresh stack.")]
pub(crate) fn first_fitting_line(
    candidates: impl IntoIterator<Item = Line<'static>>,
    width: u16,
) -> Line<'static> {
    candidates
        .into_iter()
        .find(|line| line.width() <= usize::from(width))
        .unwrap_or_default()
}

pub(crate) fn wrap_hint_rows<T>(
    hints: impl IntoIterator<Item = T>,
    width: u16,
    separator_width: usize,
    hint_width: impl Fn(&T) -> usize,
) -> Vec<Vec<T>> {
    let width = usize::from(width.max(1));
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut used = 0usize;
    for hint in hints {
        let hint_width = hint_width(&hint).min(width);
        let extra = if row.is_empty() {
            hint_width
        } else {
            separator_width.saturating_add(hint_width)
        };
        if !row.is_empty() && used.saturating_add(extra) > width {
            rows.push(std::mem::take(&mut row));
            used = 0;
        }
        if row.is_empty() {
            used = hint_width;
        } else {
            used = used
                .saturating_add(separator_width)
                .saturating_add(hint_width);
        }
        row.push(hint);
    }
    rows.push(row);
    rows
}

#[cfg(test)]
#[path = "footer_hint_tests.rs"]
mod tests;
