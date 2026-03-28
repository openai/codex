#![cfg(feature = "vt100-tests")]

use crate::test_backend::VT100Backend;
use ratatui::layout::Rect;
use ratatui::text::Line;

#[test]
fn live_001_commit_on_overflow() {
    let backend = VT100Backend::new(20, 6);
    let mut term = match codex_tui::custom_terminal::Terminal::with_options(backend) {
        Ok(t) => t,
        Err(e) => panic!("failed to construct terminal: {e}"),
    };
    let area = Rect::new(0, 5, 20, 1);
    term.set_viewport_area(area);

    // Build 5 explicit rows at width 20.
    let mut rb = codex_tui::live_wrap::RowBuilder::new(20);
    rb.push_fragment("one\n");
    rb.push_fragment("two\n");
    rb.push_fragment("three\n");
    rb.push_fragment("four\n");
    rb.push_fragment("five\n");

    // Keep the last 3 in the live ring; commit the first 2.
    let commit_rows = rb.drain_commit_ready(3);
    let lines: Vec<Line<'static>> = commit_rows.into_iter().map(|r| r.text.into()).collect();

    codex_tui::insert_history::insert_history_lines(&mut term, lines)
        .expect("Failed to insert history lines in test");

    let screen = term.backend().vt100().screen();

    // The words "one" and "two" should appear above the viewport.
    let joined = screen.contents();
    assert!(
        joined.contains("one"),
        "expected committed 'one' to be visible\n{joined}"
    );
    assert!(
        joined.contains("two"),
        "expected committed 'two' to be visible\n{joined}"
    );
    // The last three (three,four,five) remain in the live ring, not committed here.
}
