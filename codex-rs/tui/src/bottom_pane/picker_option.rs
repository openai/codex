//! Shared numbered rows for standalone pickers that retain their own keyboard policy.
//!
//! Use the same measurement and rendering as selection menus without granting actions here.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;

use super::picker_rows;
use super::picker_style::selection_style;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::measure_rows_height;
use super::selection_popup_common::render_rows;
use crate::render::renderable::Renderable;

pub(crate) fn picker_option_row(
    index: usize,
    label: String,
    is_selected: bool,
) -> Box<dyn Renderable> {
    Box::new(PickerOptionRow {
        rows: [numbered_row(index, label.into(), is_selected)],
        state: ScrollState {
            selected_idx: if is_selected { Some(0) } else { None },
            ..Default::default()
        },
    })
}

/// A bounded list of numbered choices; the caller retains its keyboard and confirmation policy.
pub(crate) fn picker_option_list(
    labels: Vec<impl Into<Line<'static>>>,
    selected_index: usize,
) -> Box<dyn Renderable> {
    Box::new(PickerOptionList {
        rows: labels
            .into_iter()
            .enumerate()
            .map(|(index, label)| numbered_row(index, label.into(), index == selected_index))
            .collect(),
        state: ScrollState {
            selected_idx: Some(selected_index),
            ..Default::default()
        },
    })
}

fn numbered_row(index: usize, label: Line<'static>, is_selected: bool) -> GenericDisplayRow {
    let marker = if is_selected { '›' } else { ' ' };
    let prefix = format!("{marker} {}. ", index + 1);
    let prefix_width = prefix.chars().count();
    let mut spans = vec![prefix.into()];
    // Styled labels carry semantic status colors and accelerator underlines.
    spans.extend(label.spans.into_iter().map(|mut span| {
        span.style = label.style.patch(span.style);
        span
    }));
    GenericDisplayRow {
        name_prefix_spans: spans,
        selection_style: Some(selection_style()),
        wrap_indent: Some(prefix_width),
        ..Default::default()
    }
}

struct PickerOptionRow {
    rows: [GenericDisplayRow; 1],
    state: ScrollState,
}

impl Renderable for PickerOptionRow {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        render_rows(area, buf, &self.rows, &self.state, self.rows.len(), "");
    }

    fn desired_height(&self, width: u16) -> u16 {
        measure_rows_height(&self.rows, &self.state, self.rows.len(), width)
    }
}

struct PickerOptionList {
    rows: Vec<GenericDisplayRow>,
    state: ScrollState,
}

impl Renderable for PickerOptionList {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        picker_rows::render_rows(area, buf, &self.rows, &self.state, self.rows.len(), "");
    }

    fn desired_height(&self, width: u16) -> u16 {
        picker_rows::measure_rows_height(&self.rows, &self.state, self.rows.len(), width)
    }
}
