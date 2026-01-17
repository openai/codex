//! Helpers for rendering selectable list rows with aligned numeric prefixes.
//!
//! The functions in this module build `Renderable` rows that show a numeric index and label with
//! consistent alignment. They compute the display width of the prefix so multi-digit indices keep
//! the label column aligned, and they apply selection/dim styling so the caller can reuse the
//! helpers across different UI contexts.

use crate::render::renderable::Renderable;
use crate::render::renderable::RowRenderable;
use ratatui::style::Style;
use ratatui::style::Styled as _;
use ratatui::style::Stylize as _;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use unicode_width::UnicodeWidthStr;

/// Builds a selectable row without dimming the label when it is not selected.
pub(crate) fn selection_option_row(
    index: usize,
    label: String,
    is_selected: bool,
) -> Box<dyn Renderable> {
    selection_option_row_with_dim(index, label, is_selected, false)
}

/// Builds a selectable row with optional dim styling for the unselected state.
pub(crate) fn selection_option_row_with_dim(
    index: usize,
    label: String,
    is_selected: bool,
    dim: bool,
) -> Box<dyn Renderable> {
    let prefix = if is_selected {
        format!("› {}. ", index + 1)
    } else {
        format!("  {}. ", index + 1)
    };
    let style = if is_selected {
        Style::default().cyan()
    } else if dim {
        Style::default().dim()
    } else {
        Style::default()
    };
    let prefix_width = UnicodeWidthStr::width(prefix.as_str()) as u16;
    let mut row = RowRenderable::new();
    row.push(prefix_width, prefix.set_style(style));
    row.push(
        u16::MAX,
        Paragraph::new(label)
            .style(style)
            .wrap(Wrap { trim: false }),
    );
    row.into()
}
