//! Loading feedback preserves the transcript's selection, search editor, and passive context.

use super::*;
use crate::history_cell::PlainHistoryCell;
use crate::style::accent_color;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;

fn render(view: &mut TranscriptView, cells: &[Arc<dyn HistoryCell>]) -> Buffer {
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 32, /*height*/ 3,
    );
    let mut buffer = Buffer::empty(area);
    view.render(area, &mut buffer, cells);
    buffer
}

#[test]
fn loading_preserves_selected_content_and_copy_action() {
    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(PlainHistoryCell::new(vec![
        "selected words".into(),
    ]))];
    let mut view = TranscriptView::default();
    render(&mut view, &cells);
    view.begin_selection(&cells, /*column*/ 0, /*row*/ 0, /*clicks*/ 3);
    view.end_drag();
    let before = render(&mut view, &cells);
    let position = view.position;

    for state in [
        TranscriptHistoryState::LoadingOlder,
        TranscriptHistoryState::LoadingBeginning,
    ] {
        view.history = state;
        let footer = view
            .footer(/*width*/ 80, MotionMode::Reduced)
            .expect("loading selection footer");
        assert_eq!(footer.cursor_column, None);
        insta::allow_duplicates! {
            insta::assert_snapshot!(footer.text.to_string(), @"↑ Loading earlier messages… · ctrl+c copy · enter copy & follow · esc clear");
        }
        let Some(ViewAction::Copy(copied)) = view.handle_key(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &cells,
        ) else {
            panic!("copy selection while loading");
        };
        assert_eq!(copied, "selected words");
        assert_eq!(
            (render(&mut view, &cells), view.position),
            (before.clone(), position)
        );
    }
    view.history = TranscriptHistoryState::Complete;
    let footer = view
        .footer(/*width*/ 80, MotionMode::Reduced)
        .expect("selection footer after loading");
    insta::assert_snapshot!(footer.text.to_string(), @"ctrl+c copy · enter copy & follow · esc clear");
    assert_eq!(
        view.selected_text(&cells),
        Some("selected words".to_string())
    );
    view.history = TranscriptHistoryState::Failed;
    insta::assert_snapshot!(view.footer(/*width*/ 38, MotionMode::Reduced).unwrap().text.to_string(),
        @"enter copy & follow · esc clear");
}

#[test]
fn loading_completion_restores_unseen_activity_and_failure_stops_motion() {
    let mut cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(PlainHistoryCell::new(vec![
        "one".into(),
        "two".into(),
        "three".into(),
        "four".into(),
    ]))];
    let mut view = TranscriptView::default();
    render(&mut view, &cells);
    view.scroll(&cells, /*rows*/ -1);
    cells.push(Arc::new(PlainHistoryCell::new(vec!["new output".into()])));
    render(&mut view, &cells);
    let before = view
        .footer(/*width*/ 80, MotionMode::Reduced)
        .expect("unseen activity footer")
        .text;
    view.history = TranscriptHistoryState::LoadingOlder;
    let loading = view
        .footer(/*width*/ 80, MotionMode::Reduced)
        .expect("loading footer");
    assert_eq!(loading.text.lines[0].spans[0], "↑ ".fg(accent_color()));
    insta::assert_snapshot!(loading.text.to_string(), @"↑ Loading earlier messages… · New activity · esc latest");
    view.history = TranscriptHistoryState::Failed;
    assert!(!view.is_loading_history());
    let failed = view
        .footer(/*width*/ 80, MotionMode::Reduced)
        .expect("failed history footer");
    insta::assert_snapshot!(failed.text.to_string(), @"New activity · Retry history: ⌥+</ctrl+home.  esc latest");
    view.history = TranscriptHistoryState::Complete;
    assert!(!view.is_loading_history());
    assert_eq!(
        view.footer(/*width*/ 80, MotionMode::Reduced)
            .expect("restored unseen activity footer")
            .text,
        before,
    );
    view.jump_to_latest();
    assert!(view.footer(/*width*/ 80, MotionMode::Reduced).is_none());
}

#[test]
fn passive_footer_keeps_jump_actions_in_one_row() {
    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(PlainHistoryCell::new(vec![
        "retained history".into(),
    ]))];
    let mut snapshots = Vec::new();
    for width in [26, 38, 78] {
        for history in [
            TranscriptHistoryState::LoadingOlder,
            TranscriptHistoryState::LoadingBeginning,
            TranscriptHistoryState::Failed,
            TranscriptHistoryState::Complete,
        ] {
            let mut view = TranscriptView::default();
            view.jump_to_entry(&cells, /*index*/ 0);
            view.history = history;
            view.unseen_activity = true;
            let footer = view
                .footer(width, MotionMode::Reduced)
                .expect("activity footer");
            assert_eq!(footer.text.height(), 1);
            assert!(footer.text.width() <= usize::from(width));
            snapshots.push(format!("{width} columns, {history:?}: {}", footer.text));
        }
    }
    insta::assert_snapshot!("compact_navigation_hints", snapshots.join("\n"));
}

#[derive(Debug)]
struct ActivityCell;

impl HistoryCell for ActivityCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        vec!["Activity summary".into()]
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        self.transcript_lines(/*width*/ 80)
    }

    fn transcript_lines(&self, _width: u16) -> Vec<Line<'static>> {
        vec!["Activity summary".into(), "Hidden details".into()]
    }

    fn activity_ids(&self) -> Vec<String> {
        vec!["sample-activity".into()]
    }
}

impl TranscriptView {
    pub(crate) fn footer(&self, width: u16, motion: MotionMode) -> Option<TranscriptFooter> {
        self.footer_with_navigation(width, motion, "esc latest")
    }
}
