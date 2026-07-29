use std::borrow::Cow;

use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use unicode_segmentation::UnicodeSegmentation;

use super::selection_popup_common::GenericDisplayRow;
use crate::line_truncation::line_width;
use crate::width::display_width;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_line;

/// Controls whether selection-row descriptions stay in a column or move below
/// their labels when the description column becomes too narrow to read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SelectionDescriptionLayout {
    #[default]
    Columns,
    StackBelowWhenNarrow {
        min_description_width: u16,
    },
}

impl SelectionDescriptionLayout {
    pub(super) fn should_stack(self, width: u16, desc_col: usize) -> bool {
        let Self::StackBelowWhenNarrow {
            min_description_width,
        } = self
        else {
            return false;
        };
        let desc_col = desc_col.min(width as usize) as u16;
        width.saturating_sub(desc_col) < min_description_width
    }
}

pub(super) fn line_to_owned(line: Line<'_>) -> Line<'static> {
    Line {
        style: line.style,
        alignment: line.alignment,
        spans: line
            .spans
            .into_iter()
            .map(|span| Span {
                style: span.style,
                content: Cow::Owned(span.content.into_owned()),
            })
            .collect(),
    }
}

fn combined_description(
    row: &GenericDisplayRow,
    description_layout: SelectionDescriptionLayout,
) -> Option<String> {
    match (&row.description, &row.disabled_reason) {
        (Some(desc), Some(reason)) => Some(format!("{desc} (disabled: {reason})")),
        (Some(desc), None) => Some(desc.clone()),
        (None, Some(reason))
            if matches!(
                description_layout,
                SelectionDescriptionLayout::StackBelowWhenNarrow { .. }
            ) =>
        {
            Some(reason.clone())
        }
        (None, Some(reason)) => Some(format!("disabled: {reason}")),
        (None, None) => None,
    }
}

fn stacked_description(row: &GenericDisplayRow) -> Option<String> {
    match (&row.description, &row.disabled_reason) {
        (Some(desc), Some(reason)) => Some(format!("{desc} (disabled: {reason})")),
        (Some(desc), None) => Some(desc.clone()),
        (None, Some(reason)) => Some(reason.clone()),
        (None, None) => None,
    }
}

fn build_name_spans(row: &GenericDisplayRow, name_limit: usize) -> Vec<Span<'static>> {
    let mut name_spans = Vec::with_capacity(row.name.len());
    let mut used_width = 0usize;
    let mut truncated = false;

    let mut match_indices = row.match_indices.iter().flatten().peekable();
    let mut char_idx = 0usize;
    for grapheme in row.name.graphemes(/*is_extended*/ true) {
        let next_width = used_width.saturating_add(display_width(grapheme));
        if next_width > name_limit {
            truncated = true;
            break;
        }
        used_width = next_width;

        let mut matched = false;
        for _ in grapheme.chars() {
            matched |= match_indices.next_if(|next| **next == char_idx).is_some();
            char_idx += 1;
        }

        let grapheme = grapheme.to_string();
        name_spans.push(if matched {
            grapheme.bold()
        } else {
            grapheme.into()
        });
    }

    if truncated {
        name_spans.push("…".into());
    }
    if row.disabled_reason.is_some() {
        name_spans.push(" (disabled)".dim());
    }
    name_spans
}

fn append_shortcut(row: &GenericDisplayRow, spans: &mut Vec<Span<'static>>) {
    if let Some(display_shortcut) = row.display_shortcut {
        spans.push(" (".into());
        spans.push(display_shortcut.into());
        spans.push(")".into());
    }
}

fn append_category_tag(row: &GenericDisplayRow, spans: &mut Vec<Span<'static>>) {
    if let Some(tag) = row.category_tag.as_deref().filter(|tag| !tag.is_empty()) {
        spans.push("  ".into());
        spans.push(tag.to_string().dim());
    }
}

/// Build the full display line for a row with the description padded to start
/// at `desc_col`.
pub(super) fn build_full_line(
    row: &GenericDisplayRow,
    desc_col: usize,
    description_layout: SelectionDescriptionLayout,
) -> Line<'static> {
    let description = combined_description(row, description_layout);
    let name_prefix_width = line_width(&Line::from(row.name_prefix_spans.clone()));
    let name_limit = description
        .as_ref()
        .map(|_| desc_col.saturating_sub(2).saturating_sub(name_prefix_width))
        .unwrap_or(usize::MAX);
    let name_spans = build_name_spans(row, name_limit);
    let name_width = name_prefix_width + line_width(&Line::from(name_spans.clone()));

    let mut spans = row.name_prefix_spans.clone();
    spans.extend(name_spans);
    append_shortcut(row, &mut spans);
    if let Some(description) = description {
        let gap = desc_col.saturating_sub(name_width);
        if gap > 0 {
            spans.push(" ".repeat(gap).into());
        }
        spans.push(description.dim());
    }
    append_category_tag(row, &mut spans);
    Line::from(spans)
}

/// Render a row as a full-width label followed by an indented description.
pub(super) fn wrap_stacked_row(row: &GenericDisplayRow, width: u16) -> Vec<Line<'static>> {
    let width = width.max(1);
    let prefix_width = line_width(&Line::from(row.name_prefix_spans.clone()))
        .min(width.saturating_sub(1) as usize);
    let indent = " ".repeat(prefix_width);

    let mut label_spans = row.name_prefix_spans.clone();
    label_spans.extend(build_name_spans(row, usize::MAX));
    append_shortcut(row, &mut label_spans);
    append_category_tag(row, &mut label_spans);
    let label = Line::from(label_spans);
    let label_options = RtOptions::new(width as usize)
        .initial_indent(Line::from(""))
        .subsequent_indent(Line::from(indent.clone()));
    let mut lines = word_wrap_line(&label, label_options)
        .into_iter()
        .map(line_to_owned)
        .collect::<Vec<_>>();

    if let Some(description) = stacked_description(row) {
        let description = Line::from(description.dim());
        let description_options = RtOptions::new(width as usize)
            .initial_indent(Line::from(indent.clone()))
            .subsequent_indent(Line::from(indent));
        lines.extend(
            word_wrap_line(&description, description_options)
                .into_iter()
                .map(line_to_owned),
        );
    }
    lines
}
