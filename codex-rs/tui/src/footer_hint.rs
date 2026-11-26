//! Packs footer hints into rows without separating a shortcut from its label.
//! Resolve focus before measuring. Only visible buttons get mouse targets.

use crate::key_hint::ShortcutHint;
use crate::style::secondary_text_style;
use ratatui::style::Styled;
use ratatui::style::Stylize;
use ratatui::text::Line;
use std::borrow::Cow;

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

/// Omit unavailable actions entirely. An available button still works without a binding.
pub(crate) enum Hint<A = ()> {
    Key(String, Cow<'static, str>),
    Button {
        key: Option<String>,
        label: &'static str,
        action: A,
        before_focus: bool,
    },
    Status(Line<'static>),
}

impl Hint {
    pub(crate) fn key(keys: impl Into<String>, label: impl Into<Cow<'static, str>>) -> Self {
        Self::Key(keys.into(), label.into())
    }
}

#[derive(Clone, Copy)]
pub(crate) enum HintFocus {
    Active,
    Inactive,
    Focusable(ShortcutHint, &'static str),
}

impl HintFocus {
    pub(crate) fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }

    pub(crate) fn render_row<A>(
        self,
        hints: impl IntoIterator<Item = Hint<A>>,
        width: u16,
    ) -> HintLine<A> {
        self.render_hints(hints, width, /*max_rows*/ 1)
            .pop()
            .unwrap_or_else(|| HintLine::new([], width))
    }

    pub(crate) fn render_lines(
        self,
        hints: impl IntoIterator<Item = Hint>,
        width: u16,
        max_rows: usize,
    ) -> Vec<Line<'static>> {
        self.render_hints(hints, width, max_rows)
            .into_iter()
            .map(|row| row.line)
            .collect()
    }

    // Keep the active pane's order. In an inactive pane, put status and primary
    // mouse controls ahead of the focus cue so they survive narrow widths.
    fn render_hints<A>(
        self,
        hints: impl IntoIterator<Item = Hint<A>>,
        width: u16,
        max_rows: usize,
    ) -> Vec<HintLine<A>> {
        let active = self.is_active();
        let mut items = Vec::new();
        let mut focus_index = 0;
        for hint in hints {
            let (line, action, priority) = match hint {
                Hint::Key(..) if !active => continue,
                Hint::Key(keys, label) => (shortcut(&keys, &label), None, false),
                Hint::Status(line) => (line, None, true),
                Hint::Button {
                    key,
                    label,
                    action,
                    before_focus,
                } => {
                    let line = match key {
                        Some(key) if active => shortcut(&key, label),
                        _ => label.bold().into(),
                    };
                    (line, Some(action), before_focus)
                }
            };
            if !active && priority {
                items.insert(focus_index, (line, action));
                focus_index += 1;
            } else {
                items.push((line, action));
            }
        }
        if let Self::Focusable(key, label) = self {
            items.insert(focus_index, (shortcut(&key.display_label(), label), None));
        }
        HintLine::wrapped(items, width, max_rows)
    }
}

pub(crate) struct HintLine<A> {
    pub(crate) line: Line<'static>,
    pub(crate) targets: Vec<(std::ops::Range<u16>, A)>,
}

impl<A> HintLine<A> {
    /// Wrap complete shortcuts into at most `max_rows`, marking any remaining overflow.
    fn wrapped(hints: Vec<(Line<'static>, Option<A>)>, width: u16, max_rows: usize) -> Vec<Self> {
        if max_rows == 0 || hints.is_empty() {
            return Vec::new();
        }
        if max_rows == 1 {
            return vec![Self::new(hints, width)];
        }
        let mut rows = wrap_hint_rows(
            hints,
            width,
            /*separator_width*/ 3,
            |(line, _)| line.width(),
        )
        .into_iter();
        let mut lines = Vec::new();
        while let Some(row) = rows.next() {
            if lines.len() + 1 == max_rows {
                lines.push(Self::new(row.into_iter().chain(rows.flatten()), width));
                break;
            }
            lines.push(Self::new(row, width));
        }
        lines
    }

    fn new(hints: impl IntoIterator<Item = (Line<'static>, Option<A>)>, width: u16) -> Self {
        let mut hints = hints.into_iter().peekable();
        let mut line = Line::default();
        let mut targets = Vec::new();
        let mut end = 0;
        while let Some((hint, action)) = hints.next() {
            let start = end + if line.spans.is_empty() { 0 } else { 3 };
            let next = start + hint.width();
            let overflow_space = if hints.peek().is_some() { 2 } else { 0 };
            if next + overflow_space > usize::from(width) {
                if width > 0 {
                    let marker = if line.spans.is_empty() { "…" } else { " …" };
                    line.push_span(marker.set_style(secondary_text_style()));
                }
                break;
            }
            if !line.spans.is_empty() {
                line.push_span(" · ".set_style(secondary_text_style()));
            }
            line.extend(hint.spans);
            if let Some(action) = action {
                targets.push((start as u16..next as u16, action));
            }
            end = next;
        }
        Self { line, targets }
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
