#![cfg(feature = "vt100-tests")]

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::text::Line;

#[test]
fn live_001_commit_on_overflow() {
    let backend = TestBackend::new(20, 6);
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
    let lines: Vec<Line<'static>> = commit_rows
        .into_iter()
        .map(|r| Line::from(r.text))
        .collect();

    let mut buf: Vec<u8> = Vec::new();
    codex_tui::insert_history::insert_history_lines_to_writer(&mut term, &mut buf, lines);

    let mut parser = vt100::Parser::new(6, 20, 0);
    parser.process(&buf);
    let screen = parser.screen();

    // The words "one" and "two" should appear above the viewport.
    let mut joined = String::new();
    for row in 0..6 {
        for col in 0..20 {
            if let Some(cell) = screen.cell(row, col) {
                if let Some(ch) = cell.contents().chars().next() {
                    joined.push(ch);
                } else {
                    joined.push(' ');
                }
            }
        }
        joined.push('\n');
    }
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

#[test]
fn live_002_pre_scroll_and_commit() {
    let backend = TestBackend::new(20, 6);
    let mut term = match codex_tui::custom_terminal::Terminal::with_options(backend) {
        Ok(t) => t,
        Err(e) => panic!("failed to construct terminal: {e}"),
    };
    // Viewport not at bottom: y=3
    let area = Rect::new(0, 3, 20, 1);
    term.set_viewport_area(area);

    let mut rb = codex_tui::live_wrap::RowBuilder::new(20);
    rb.push_fragment("alpha\n");
    rb.push_fragment("beta\n");
    rb.push_fragment("gamma\n");
    rb.push_fragment("delta\n");

    // Keep 3, commit 1.
    let commit_rows = rb.drain_commit_ready(3);
    let lines: Vec<Line<'static>> = commit_rows
        .into_iter()
        .map(|r| Line::from(r.text))
        .collect();

    let mut buf: Vec<u8> = Vec::new();
    codex_tui::insert_history::insert_history_lines_to_writer(&mut term, &mut buf, lines);
    let s = String::from_utf8_lossy(&buf);

    // Expect a SetScrollRegion to [area.top()+1 .. screen_height] and a cursor move to top of that region.
    assert!(
        s.contains("\u{1b}[4;6r"),
        "expected pre-scroll region 4..6, got: {s:?}"
    );
    assert!(
        s.contains("\u{1b}[4;1H"),
        "expected cursor CUP 4;1H, got: {s:?}"
    );
}
