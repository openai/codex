use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Widget;

use crate::key_hint;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;

use super::search_mode::SearchMode;

pub(super) fn render_footer(area: Rect, buf: &mut Buffer, search_mode: SearchMode) {
    let right_line = search_mode_indicator_line(search_mode);
    let right_width = right_line.width() as u16;
    let gap = u16::from(right_width > 0);
    let left_width = area.width.saturating_sub(right_width).saturating_sub(gap);
    let left_line =
        truncate_line_with_ellipsis_if_overflow(footer_hint_line(), left_width as usize);
    left_line.render(
        Rect {
            x: area.x,
            y: area.y,
            width: left_width,
            height: 1,
        },
        buf,
    );
    if right_width > 0 && right_width <= area.width {
        right_line.render(
            Rect {
                x: area.x + area.width - right_width,
                y: area.y,
                width: right_width,
                height: 1,
            },
            buf,
        );
    }
}

fn footer_hint_line() -> Line<'static> {
    Line::from(vec![
        key_hint::plain(KeyCode::Enter).into(),
        " insert · ".dim(),
        key_hint::plain(KeyCode::Esc).into(),
        " close · ".dim(),
        key_hint::plain(KeyCode::Left).into(),
        "/".dim(),
        key_hint::plain(KeyCode::Right).into(),
        " switch search modes · ".dim(),
        key_hint::alt(KeyCode::Char('f')).into(),
        " to remember search mode".dim(),
    ])
}

fn search_mode_indicator_line(active_search_mode: SearchMode) -> Line<'static> {
    let modes = [
        SearchMode::Results,
        SearchMode::FilesystemOnly,
        SearchMode::Tools,
    ];
    let mut spans = Vec::with_capacity(modes.len() * 2 - 1);

    for (index, search_mode) in modes.into_iter().enumerate() {
        if index > 0 {
            spans.push("  ".dim());
        }

        if search_mode == active_search_mode {
            spans.push(format!("[{}]", search_mode.label()).cyan().bold());
        } else {
            spans.push(format!(" {} ", search_mode.label()).dim());
        }
    }

    Line::from(spans)
}
