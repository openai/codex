use crate::render::adapter_ratatui::from_ratatui_style;
use crate::render::model::RenderLine as Line;
use crate::render::renderable::Renderable;
use crate::render::renderable::RowRenderable;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use unicode_width::UnicodeWidthStr;

pub(crate) fn selection_option_row(
    index: usize,
    label: String,
    is_selected: bool,
) -> Box<dyn Renderable> {
    selection_option_row_with_dim(index, label, is_selected, false)
}

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
        Style::default().fg(Color::Cyan)
    } else if dim {
        Style::default().add_modifier(Modifier::DIM)
    } else {
        Style::default()
    };
    let prefix_width = UnicodeWidthStr::width(prefix.as_str()) as u16;
    let mut row = RowRenderable::new();
    row.push(
        prefix_width,
        Line::from(prefix).style(from_ratatui_style(style)),
    );
    row.push(
        u16::MAX,
        Paragraph::new(label)
            .style(style)
            .wrap(Wrap { trim: false }),
    );
    row.into()
}
