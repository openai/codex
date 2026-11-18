#![cfg(test)]

use super::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn buffered_answer_requires_a_paint_before_wheel_input() {
    let cwd = AbsolutePathBuf::try_from(std::env::current_dir().unwrap()).unwrap();
    let mut pane = SideCompanion::empty(cwd);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 20, /*height*/ 4,
    );
    pane.view
        .render(area, &mut Buffer::empty(area), &pane.cells);
    assert!(pane.can_scroll_without_refresh());

    let mut answer = SideAnswer::new(("turn".into(), "item".into()), /*index*/ 0);
    answer.pending = "new reply".into();
    pane.answer = Some(answer);
    assert!(!pane.can_scroll_without_refresh());

    pane.flush_answer();
    pane.view
        .render(area, &mut Buffer::empty(area), &pane.cells);
    assert!(pane.can_scroll_without_refresh());
}
