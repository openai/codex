use ratatui::style::{Style, Styled as _, Stylize as _};
use ratatui::widgets::{Paragraph, Wrap};
use unicode_width::UnicodeWidthStr;

use crate::render::renderable::{Renderable, RowRenderable};

pub(crate) fn selection_option_row(
    index: usize,
    label: String,
    is_selected: bool,
) -> Box<dyn Renderable> {
    let prefix = if is_selected {
        format!("› {}. ", index + 1)
    } else {
        format!("  {}. ", index + 1)
    };
    let style = if is_selected {
        Style::default().cyan()
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
