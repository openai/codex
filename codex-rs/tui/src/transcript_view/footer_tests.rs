//! Loading feedback preserves the transcript's selection, search editor, and passive context.

use super::*;
use crate::history_cell::PlainHistoryCell;
use crate::style::accent_color;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use crossterm::event::MouseButton;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use pretty_assertions::assert_eq;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

fn render(view: &mut TranscriptView, cells: &[Arc<dyn HistoryCell>]) -> Buffer {
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 32, /*height*/ 3,
    );
    let mut buffer = Buffer::empty(area);
    view.render(area, &mut buffer, cells);
    buffer
}

#[test]
fn selection_hints_follow_selected_text_instead_of_mouse_down() {
    let cells: Vec<Arc<dyn HistoryCell>> =
        vec![Arc::new(PlainHistoryCell::new(vec!["é selection".into()]))];
    let mut snapshots = Vec::new();
    for searching in [false, true] {
        let mut view = TranscriptView::default();
        render(&mut view, &cells);
        if searching {
            view.begin_search();
        }
        for (label, kind, column) in [
            ("mouse down", MouseEventKind::Down(MouseButton::Left), 0),
            ("one character", MouseEventKind::Drag(MouseButton::Left), 1),
            ("back to anchor", MouseEventKind::Drag(MouseButton::Left), 0),
            ("released empty", MouseEventKind::Up(MouseButton::Left), 0),
        ] {
            view.handle_mouse(
                MouseEvent {
                    kind,
                    column,
                    row: 0,
                    modifiers: KeyModifiers::NONE,
                },
                &cells,
            );
            render(&mut view, &cells);
            let footer = view.footer(/*width*/ 80, MotionMode::Reduced);
            if label == "one character" {
                assert_eq!(view.selected_text(&cells).as_deref(), Some("é"));
            }
            let text = footer.map_or_else(
                || "<composer hints>".to_string(),
                |footer| footer.text.to_string(),
            );
            snapshots.push(format!("searching={searching} · {label}\n{text}"));
        }
    }
    let joined = snapshots.join("\n\n");
    let snapshot = joined
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!("selection_hint_threshold", snapshot);
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
        assert_eq!((footer.cursor_column, footer.is_interactive), (None, true));
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
fn loading_and_failure_preserve_search_query_and_caret() {
    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(PlainHistoryCell::new(vec![
        "visible history".into(),
    ]))];
    let mut view = TranscriptView::default();
    render(&mut view, &cells);
    view.history = TranscriptHistoryState::Partial;
    view.begin_search();
    view.paste_search("missing needle");
    assert!(!view.advance_search(&cells));
    assert!(view.needs_history(&cells));

    let mut hints = Vec::new();
    for width in [32, 12] {
        let editor = view.search_footer(width).expect("search editor");
        for state in [
            TranscriptHistoryState::LoadingOlder,
            TranscriptHistoryState::LoadingBeginning,
            TranscriptHistoryState::Failed,
        ] {
            view.history = state;
            view.advance_search(&cells);
            let footer = view
                .footer(width, MotionMode::Reduced)
                .expect("search footer");
            assert_eq!(
                (footer.text.lines[0].clone(), footer.cursor_column),
                (editor.0.clone(), Some(editor.1)),
            );
            assert!(footer.is_interactive);
            let hint = &footer.text.lines[1];
            assert!(hint.width() <= usize::from(width));
            hints.push(format!("{width} columns, {state:?}: {hint}"));
        }
    }
    insta::assert_snapshot!(hints.join("\n"));
    view.history = TranscriptHistoryState::Complete;
    view.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &cells);
    assert!(!view.is_search_active());
    assert!(!view.is_detailed());
    assert!(view.footer(/*width*/ 32, MotionMode::Reduced).is_none());
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

#[test]
fn narrow_selection_and_search_keep_complete_exit_hints() {
    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(PlainHistoryCell::new(vec![
        "selected words".into(),
    ]))];
    let mut view = TranscriptView::default();
    render(&mut view, &cells);
    view.begin_selection(&cells, /*column*/ 0, /*row*/ 0, /*clicks*/ 3);
    view.end_drag();
    let mut snapshots = Vec::new();
    for searching in [false, true] {
        if searching {
            view.end_selection(&cells);
            view.begin_search();
            view.paste_search("selected");
            view.advance_search(&cells);
        }
        for width in [12, 20, 32, 80] {
            let footer = view.footer(width, MotionMode::Reduced).unwrap();
            let hint = footer.text.lines.last().unwrap();
            assert!(hint.width() <= usize::from(width));
            assert!(hint.to_string().contains("esc"));
            snapshots.push(format!("search={searching}, {width} columns: {hint}"));
        }
    }
    insta::assert_snapshot!(snapshots.join("\n"));
}

#[derive(Debug, Default)]
struct ActivityCell {
    expanded_renders: AtomicUsize,
}

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

    fn expanded_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        self.expanded_renders
            .fetch_add(/*val*/ 1, Ordering::Relaxed);
        self.transcript_hyperlink_lines(width)
    }

    fn activity_ids(&self) -> Vec<String> {
        vec!["sample-activity".into()]
    }
}

#[test]
fn activity_focus_keeps_controls_without_passive_hints() {
    let mut cells: Vec<Arc<dyn HistoryCell>> = Vec::new();
    let cell = Arc::new(ActivityCell::default());
    let mut view = TranscriptView::default();
    let render_live = |view: &mut TranscriptView| {
        let expanded = view.sync_live_activity(&cells, cell.activity_ids());
        view.sync_live_activity_tail(/*width*/ 32, /*key*/ None, expanded, |width| {
            Some(ActivityTranscriptLines {
                activity: if expanded {
                    cell.expanded_hyperlink_lines(width)
                } else {
                    cell.compact_hyperlink_lines(width)
                },
                auxiliary: Vec::new(),
                has_hidden_details: true,
            })
        });
        render(view, &[])
    };
    let collapsed = render_live(&mut view);
    let mut hints = vec![format!("collapsed\n{collapsed:?}")];
    for width in [20, 32, 80] {
        hints.push(format!(
            "{width} columns: {}",
            view.footer(width, MotionMode::Reduced).map_or_else(
                || "<composer hints>".to_owned(),
                |footer| footer.text.to_string()
            ),
        ));
    }
    view.handle_key(KeyEvent::new(KeyCode::F(4), KeyModifiers::NONE), &cells);
    assert!(view.is_activity_focused());
    hints.push(format!("focused\n{:?}", render_live(&mut view)));
    for width in [20, 32, 80] {
        let footer = view.footer(width, MotionMode::Reduced).unwrap();
        assert!(footer.is_interactive);
        hints.push(format!("focused, {width} columns: {}", footer.text));
    }
    view.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &cells);
    assert!(view.disclosure.is_expanded(&["sample-activity".to_owned()]));
    let expanded = render_live(&mut view);
    hints.push(format!("expanded\n{expanded:?}"));
    let hidden = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 32, /*height*/ 0,
    );
    view.render(hidden, &mut Buffer::empty(hidden), &cells);
    cells.push(cell.clone());
    view.sync_live_activity(&cells, Vec::new());
    view.sync_live_tail(/*width*/ 32, /*key*/ None, |_| None);
    assert_eq!(render(&mut view, &cells), expanded);
    assert!(cell.expanded_renders.swap(/*val*/ 0, Ordering::Relaxed) > 0);
    view.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &cells);
    render(&mut view, &cells);
    assert_eq!(cell.expanded_renders.load(Ordering::Relaxed), 0);
    view.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &cells);
    assert!(!view.is_activity_focused());

    for configured in [
        serde_json::json!("ctrl-x t"),
        serde_json::json!(["ctrl-x t", "f4"]),
        serde_json::json!([]),
    ] {
        let config = serde_json::from_value(serde_json::json!({
            "global": {"focus_activity": configured}
        }))
        .unwrap();
        let keymap = crate::keymap::RuntimeKeymap::from_config(&config).unwrap();
        view.set_keymap_bindings(&keymap);
        view.jump_to_latest();
        render(&mut view, &cells);
        let hint = view.footer(/*width*/ 80, MotionMode::Reduced).map_or_else(
            || "<composer hints>".to_owned(),
            |footer| footer.text.to_string(),
        );
        hints.push(format!("{configured}: {hint}"));
    }
    insta::assert_snapshot!(hints.join("\n"));
}

#[test]
fn transcript_footer_keys_remain_distinct_from_prose_and_query_text() {
    use ratatui::widgets::Paragraph;

    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(ActivityCell::default())];
    let mut view = TranscriptView::default();
    render(&mut view, &cells);
    view.begin_search();
    view.paste_search("Activity");
    view.advance_search(&cells);
    let footer = view.footer(/*width*/ 80, MotionMode::Reduced).unwrap();
    let area = Rect::new(
        /*x*/ 0,
        /*y*/ 0,
        /*width*/ 80,
        footer.text.height() as u16,
    );
    let mut buffer = Buffer::empty(area);
    Paragraph::new(footer.text).render(area, &mut buffer);
    insta::assert_snapshot!(format!("{buffer:?}"));
}

impl TranscriptView {
    pub(crate) fn footer(&self, width: u16, motion: MotionMode) -> Option<TranscriptFooter> {
        self.footer_with_navigation(width, motion, "esc latest")
    }
}
