//! Shared visual tokens and overflow hints for the picker component.
//!
//! Resolve RGB colors when rendering so terminal palette fallbacks stay effective.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Widget;

use super::selection_popup_common::RenderedRows;
use crate::color::is_light;
pub(crate) use crate::style::selection_style;
use crate::terminal_palette::best_color;
use crate::terminal_palette::default_bg;
use ratatui::style::Color;

/// The shared picker can blend into the transcript or form a shaded browser panel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PickerSurface {
    #[default]
    Terminal,
    Panel,
}

pub(super) fn active_tab_style() -> Style {
    let fallback = Style::default()
        .fg(Color::Reset)
        .bg(Color::Reset)
        .bold()
        .not_dim()
        .underlined();
    let Some(background) = default_bg() else {
        return fallback;
    };
    let fill = best_color(if is_light(background) {
        (220, 220, 220)
    } else {
        (76, 76, 76)
    });
    if fill == Color::Reset {
        return fallback;
    }
    Style::default()
        .fg(crate::style::readable_color_on(Color::Reset, Some(fill)))
        .bg(fill)
        .bold()
        .not_dim()
        .not_underlined()
}

/// Draw overflow hints against the background already painted beneath each cell.
pub(super) fn render_scroll_indicators(area: Rect, buf: &mut Buffer, rendered: RenderedRows) {
    if area.height < 3 || area.width == 0 {
        return;
    }
    for (visible, symbol, y) in [
        (rendered.has_above, "↑", area.y),
        (rendered.has_below, "↓", area.bottom() - 1),
    ] {
        if visible {
            Line::from(symbol)
                .fg(crate::style::accent_color_on(Some(buf[(area.x, y)].bg)))
                .render(Rect::new(area.x, y, /*width*/ 1, /*height*/ 1), buf);
        }
    }
}
