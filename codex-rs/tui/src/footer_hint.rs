//! Packs footer hints into rows without separating a shortcut from its label.
//! Shared tips style keyboard tokens separately from their labels; callers own clipping.

use crate::style::secondary_text_style;
use ratatui::style::Styled;
use ratatui::text::Line;

/// Show the configured scroll keys for directions with rows beyond the viewport.
pub(crate) fn scroll_keys(
    offset: usize,
    max_offset: usize,
    up: Option<String>,
    down: Option<String>,
) -> Option<String> {
    match (
        up.filter(|_| offset > 0),
        down.filter(|_| offset < max_offset),
    ) {
        (Some(up), Some(down)) => Some(format!("{up}/{down}")),
        (up, down) => up.or(down),
    }
}

/// A complete hint uses the same styled content for measurement and rendering.
pub(crate) fn shortcut(keys: &str, label: &str) -> Line<'static> {
    let mut spans = crate::key_hint::key_label_spans(keys);
    spans.push(format!(" {label}").set_style(secondary_text_style()));
    Line::from(spans)
}

/// The same measured spans determine which hints fit and where each one can be clicked.
pub(crate) struct HintLine {
    pub(crate) line: Line<'static>,
    pub(crate) items: Vec<std::ops::Range<u16>>,
}

impl HintLine {
    pub(crate) fn new(hints: impl IntoIterator<Item = Line<'static>>, width: u16) -> Self {
        let mut hints = hints.into_iter().peekable();
        let mut line = Line::default();
        let mut items = Vec::new();
        let mut end = 0;
        while let Some(hint) = hints.next() {
            let start = end + if items.is_empty() { 0 } else { 3 };
            let next = start + hint.width();
            let overflow_space = if hints.peek().is_some() { 2 } else { 0 };
            if next + overflow_space > usize::from(width) {
                if width > 0 {
                    let marker = if items.is_empty() { "…" } else { " …" };
                    line.push_span(marker.set_style(secondary_text_style()));
                }
                break;
            }
            if !items.is_empty() {
                line.push_span(" · ".set_style(secondary_text_style()));
            }
            line.extend(hint.spans);
            items.push(start as u16..next as u16);
            end = next;
        }
        Self { line, items }
    }
}

/// Choose a complete hint variant so narrow surfaces never display half a shortcut.
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
