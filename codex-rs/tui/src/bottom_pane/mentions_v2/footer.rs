//! Keyboard help for the live mention popup; filter tabs live above the results.

use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Widget;

use crate::key_hint;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;

pub(super) fn render_footer(area: Rect, buf: &mut Buffer) {
    let mut spans = Vec::new();
    for (keys, label) in [
        (vec![KeyCode::Enter, KeyCode::Tab], " insert · "),
        (vec![KeyCode::Esc], " close · "),
        (vec![KeyCode::Up, KeyCode::Down], " select · "),
        (vec![KeyCode::Left, KeyCode::Right], " filter"),
    ] {
        let keys = keys
            .into_iter()
            .map(|key| key_hint::plain(key).display_label())
            .collect::<Vec<_>>()
            .join("/");
        spans.extend(key_hint::key_label_spans(&keys));
        spans.push(label.dim());
    }
    let line = Line::from(spans);
    truncate_line_with_ellipsis_if_overflow(line, usize::from(area.width)).render(area, buf);
}
