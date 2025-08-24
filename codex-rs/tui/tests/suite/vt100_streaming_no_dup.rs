#![cfg(feature = "vt100-tests")]

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::text::Line;

fn term(viewport: Rect) -> codex_tui::custom_terminal::Terminal<TestBackend> {
    let backend = TestBackend::new(20, 6);
    let mut term = codex_tui::custom_terminal::Terminal::with_options(backend)
        .unwrap_or_else(|e| panic!("failed to construct terminal: {e}"));
    term.set_viewport_area(viewport);
    term
}

#[test]
fn stream_commit_trickle_no_duplication() {
    // Viewport is the last row (height=1 at y=5)
    let area = Rect::new(0, 5, 20, 1);
    let mut t = term(area);

    // Step 1: commit first row
    let mut out1 = Vec::new();
    codex_tui::insert_history::insert_history_lines_to_writer(
        &mut t,
        &mut out1,
        vec![Line::from("one")],
    );

    // Step 2: later commit next row
    let mut out2 = Vec::new();
    codex_tui::insert_history::insert_history_lines_to_writer(
        &mut t,
        &mut out2,
        vec![Line::from("two")],
    );

    let combined = [out1, out2].concat();
    let s = String::from_utf8_lossy(&combined);
    assert_eq!(
        s.matches("one").count(),
        1,
        "history line duplicated: {s:?}"
    );
    assert_eq!(
        s.matches("two").count(),
        1,
        "history line duplicated: {s:?}"
    );
    assert!(
        !s.contains("three"),
        "live-only content leaked into history: {s:?}"
    );
}

#[test]
fn live_ring_rows_not_inserted_into_history() {
    let area = Rect::new(0, 5, 20, 1);
    let mut t = term(area);

    // Commit two rows to history.
    let mut buf = Vec::new();
    codex_tui::insert_history::insert_history_lines_to_writer(
        &mut t,
        &mut buf,
        vec![Line::from("one"), Line::from("two")],
    );

    // The live ring might display tail+head rows like ["two", "three"],
    // but only committed rows should be present in the history ANSI stream.
    let s = String::from_utf8_lossy(&buf);
    assert!(s.contains("one"));
    assert!(s.contains("two"));
    assert!(
        !s.contains("three"),
        "uncommitted live-ring content should not be inserted into history: {s:?}"
    );
}
