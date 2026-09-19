use std::borrow::Cow;

use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use unicode_segmentation::UnicodeSegmentation;

use super::selection_popup_common::GenericDisplayRow;
use crate::line_truncation::line_width;
use crate::width::display_width;

/// Controls whether selection-row descriptions remain visible when their column is narrow.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SelectionDescriptionLayout {
    #[default]
    Columns,
    HideWhenNarrow {
        min_description_width: u16,
    },
}

impl SelectionDescriptionLayout {
    pub(super) fn should_hide(self, width: u16, desc_col: usize) -> bool {
        let Self::HideWhenNarrow {
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
                SelectionDescriptionLayout::HideWhenNarrow { .. }
            ) =>
        {
            Some(reason.clone())
        }
        (None, Some(reason)) => Some(format!("disabled: {reason}")),
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
        spans.extend(display_shortcut.spans());
        spans.push(")".into());
    }
}

/// Build the full display line for a row with the description padded to start
/// at `desc_col`.
pub(super) fn build_full_line(
    row: &GenericDisplayRow,
    desc_col: usize,
    width: u16,
    description_layout: SelectionDescriptionLayout,
) -> Line<'static> {
    let description = (desc_col > 0)
        .then(|| combined_description(row, description_layout))
        .flatten();
    let name_prefix_width = line_width(&Line::from(row.name_prefix_spans.clone()));
    // Category tags identify the result even when secondary descriptions hide.
    // Reserve their cells before truncating the name on narrow rows.
    let metadata_col = row
        .category_tag
        .as_deref()
        .map(|tag| {
            let desired_col = if desc_col > 0 {
                desc_col
            } else {
                name_prefix_width + display_width(&row.name) + 2
            };
            desired_col.min(usize::from(width).saturating_sub(display_width(tag)))
        })
        .or_else(|| description.as_ref().map(|_| desc_col));
    let name_limit = metadata_col
        .map(|column| {
            column
                .saturating_sub(/*rhs*/ 2)
                .saturating_sub(name_prefix_width)
        })
        .unwrap_or(usize::MAX);
    let name_limit = if row.category_tag.is_some() && display_width(&row.name) > name_limit {
        name_limit.saturating_sub(/*rhs*/ 1)
    } else {
        name_limit
    };
    let name_spans = if row.category_tag.is_some()
        && metadata_col.is_some_and(|column| column <= name_prefix_width)
    {
        Vec::new()
    } else {
        build_name_spans(row, name_limit)
    };
    let name_width = name_prefix_width + line_width(&Line::from(name_spans.clone()));

    let mut spans = row.name_prefix_spans.clone();
    spans.extend(name_spans);
    append_shortcut(row, &mut spans);
    if let Some(metadata_col) = metadata_col {
        let gap = metadata_col.saturating_sub(name_width);
        if gap > 0 {
            spans.push(" ".repeat(gap).into());
        }
    }
    if let Some(tag) = &row.category_tag {
        spans.push(tag.clone().dim());
        if description.is_some() {
            spans.push(" ".dim());
        }
    }
    if let Some(description) = description {
        spans.push(description.dim());
    }
    Line::from(spans)
}
