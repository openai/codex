//! Session information appears only once all older transcript pages are available.

use super::*;
use crate::pager_overlay::TranscriptHistoryState;
use crate::transcript_view::TranscriptView;
use crossterm::event::KeyModifiers;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::sync::Arc;

#[test]
fn wheel_at_hidden_session_header_keeps_the_loaded_message_anchor() {
    let header: Arc<dyn HistoryCell> = Arc::new(SessionInfoCell(CompositeHistoryCell {
        parts: vec![Box::new(PlainHistoryCell::new(vec![
            "session announcement".into(),
        ]))],
    }));
    let recent: Arc<dyn HistoryCell> = Arc::new(PlainHistoryCell::new(
        (0..6)
            .map(|row| format!("recent row {row}").into())
            .collect(),
    ));
    let mut cells = vec![header, recent];
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 30, /*height*/ 3,
    );
    let mut view = TranscriptView::default();
    view.history = TranscriptHistoryState::Partial;
    let mut before = Buffer::empty(area);
    view.render(area, &mut before, &cells);
    for _ in 0..2 {
        view.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::ScrollUp,
                column: 0,
                row: 0,
                modifiers: KeyModifiers::NONE,
            },
            &cells,
        );
        view.render(area, &mut before, &cells);
    }
    assert!(view.needs_history(&cells));
    let page = (0..3).map(|row| {
        Arc::new(PlainHistoryCell::new(vec![
            format!("older row {row}").into(),
        ])) as Arc<dyn HistoryCell>
    });
    cells.splice(1..1, page);
    view.history_loaded(&cells, 1..4);
    assert!(
        !view.needs_history(&cells),
        "one wheel gesture must not keep requesting older pages"
    );
    let mut after = Buffer::empty(area);
    view.render(area, &mut after, &cells);
    assert_eq!(after, before);
}

#[test]
fn session_information_waits_for_older_pages_and_reappears_after_completion() {
    let header: Arc<dyn HistoryCell> = Arc::new(SessionInfoCell(CompositeHistoryCell {
        parts: vec![Box::new(PlainHistoryCell::new(vec![
            "session announcement".into(),
        ]))],
    }));
    let cells = vec![header];
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 30, /*height*/ 2,
    );
    let mut view = TranscriptView::default();
    let mut output = Vec::new();
    for history in [
        TranscriptHistoryState::Complete,
        TranscriptHistoryState::Partial,
        TranscriptHistoryState::LoadingOlder,
        TranscriptHistoryState::LoadingBeginning,
        TranscriptHistoryState::Failed,
        TranscriptHistoryState::Complete,
    ] {
        view.history = history;
        let mut buffer = Buffer::empty(area);
        view.render(area, &mut buffer, &cells);
        output.push(
            buffer
                .content
                .iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
                .trim()
                .to_owned(),
        );
    }
    assert_eq!(
        output,
        [
            "session announcement",
            "",
            "",
            "",
            "",
            "session announcement"
        ]
    );
}
