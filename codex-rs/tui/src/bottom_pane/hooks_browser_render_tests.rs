//! Wide hook titles preserve the selection fill and per-span index emphasis.

use super::*;
use pretty_assertions::assert_eq;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;

#[test]
fn wide_titles_preserve_reverse_video_selection_and_dim_index() {
    let style = Style::default()
        .fg(Color::Reset)
        .bg(Color::Reset)
        .bold()
        .not_dim()
        .reversed();
    let row = Line::from(vec![
        "› [x] ".into(),
        " 1".dim(),
        "  检查 🦀 shell commands".into(),
    ])
    .style(style);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 1,
    );
    let mut buf = Buffer::empty(area);

    render_line_rows(area, &mut buf, vec![row], ScrollState::new());

    assert_eq!(
        buf.content
            .iter()
            .map(|cell| (cell.fg, cell.bg, cell.modifier.contains(Modifier::REVERSED)))
            .collect::<Vec<_>>(),
        vec![(Color::Reset, Color::Reset, true); usize::from(area.width)]
    );
    assert!(buf[(7, 0)].modifier.contains(Modifier::DIM));
    assert!(!buf[(10, 0)].modifier.contains(Modifier::DIM));
    assert_eq!(buf[(10, 0)].symbol(), "检");
}
