//! Activity follows visible rows and Escape respects transcript interaction ownership.

use super::*;
use crate::history_cell::PlainHistoryCell;
use crate::motion::MotionMode;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;

fn cell(text: &str) -> Arc<dyn HistoryCell> {
    Arc::new(PlainHistoryCell::new(
        text.lines().map(|line| line.to_owned().into()).collect(),
    ))
}

fn render(view: &mut TranscriptView, cells: &[Arc<dyn HistoryCell>], height: u16) {
    let area = Rect::new(/*x*/ 0, /*y*/ 0, /*width*/ 32, height);
    view.render(area, &mut Buffer::empty(area), cells);
}

#[test]
fn visible_updates_and_empty_tail_cells_do_not_report_activity() {
    let mut cells = vec![cell("one\ntwo\nthree\nfour")];
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*height*/ 3);
    view.scroll(&cells, /*rows*/ -3);
    cells.push(cell(""));
    render(&mut view, &cells, /*height*/ 3);
    assert!(!view.unseen_activity);
    cells.extend([cell("visible output"), cell("")]);
    render(&mut view, &cells, /*height*/ 3);
    assert!(view.unseen_activity);
    render(&mut view, &cells, /*height*/ 8);
    assert!(!view.unseen_activity);
    assert_eq!(
        view.footer(/*width*/ 80, MotionMode::Reduced)
            .unwrap()
            .text
            .to_string(),
        "esc latest"
    );
}

#[test]
fn unchanged_live_revisions_do_not_report_activity_but_hidden_text_does() {
    let cells = vec![cell("one\ntwo\nthree\nfour")];
    let mut view = TranscriptView::default();
    for (revision, text, unseen) in [
        (1, "tail", false),
        (2, "tail", false),
        (3, "new tail", true),
    ] {
        view.sync_live_tail(
            /*width*/ 32,
            Some(ActiveCellTranscriptKey {
                revision,
                animation_tick: None,
                is_stream_continuation: false,
                cacheable: true,
            }),
            |_| Some(vec![HyperlinkLine::from(text)]),
        );
        render(&mut view, &cells, /*height*/ 3);
        assert_eq!(view.unseen_activity, unseen);
        view.scroll(&cells, /*rows*/ -3);
    }
}

#[test]
fn escape_closes_selection_and_search_before_returning_to_latest() {
    let mut cells = vec![cell("one\ntwo\nthree\nfour")];
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*height*/ 3);
    view.scroll(&cells, /*rows*/ -1);
    cells.push(cell("new output"));
    render(&mut view, &cells, /*height*/ 3);
    view.begin_search();
    view.begin_selection(&cells, /*column*/ 0, /*row*/ 0, /*clicks*/ 3);
    let escape = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    view.handle_key(escape, &cells);
    assert!(view.selection.is_none());
    assert!(view.is_search_active());
    view.handle_key(escape, &cells);
    assert!(!view.is_search_active());
    assert!(!view.is_following());
    assert!(matches!(
        view.handle_key(escape, &cells),
        Some(ViewAction::Changed)
    ));
    assert!(view.is_following());
    assert!(view.footer(/*width*/ 80, MotionMode::Reduced).is_none());
    assert!(view.handle_key(escape, &cells).is_none());
}
