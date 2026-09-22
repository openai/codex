//! Viewport behavior and cache lifetime across scrolling, selection, and redraws.

use super::*;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use crossterm::event::MouseButton;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use pretty_assertions::assert_eq;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

#[derive(Debug)]
struct TestCell {
    text: String,
    renders: Arc<AtomicUsize>,
}

impl HistoryCell for TestCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        self.renders.fetch_add(/*val*/ 1, Ordering::Relaxed);
        self.text
            .lines()
            .map(|line| line.to_owned().into())
            .collect()
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        self.display_lines(/*width*/ 80)
    }
}

pub(super) fn cell(text: impl Into<String>) -> Arc<dyn HistoryCell> {
    Arc::new(TestCell {
        text: text.into(),
        renders: Arc::default(),
    })
}

pub(super) fn render(
    view: &mut TranscriptView,
    cells: &[Arc<dyn HistoryCell>],
    width: u16,
    height: u16,
) -> Buffer {
    let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
    let mut buffer = Buffer::empty(area);
    view.render(area, &mut buffer, cells);
    buffer
}

pub(super) fn text(buffer: &Buffer) -> String {
    (buffer.area.top()..buffer.area.bottom())
        .map(|y| {
            (buffer.area.left()..buffer.area.right())
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn reading_survives_prepend_and_new_output_then_returns_to_latest() {
    let mut cells = vec![cell("older\nline two"), cell("current\nlast line")];
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 20, /*height*/ 3);
    view.scroll(&cells, /*rows*/ -2);
    let before = text(&render(
        &mut view, &cells, /*width*/ 20, /*height*/ 3,
    ));
    cells.insert(/*index*/ 0, cell("loaded from history"));
    cells.push(cell("new output"));
    assert_eq!(
        text(&render(
            &mut view, &cells, /*width*/ 20, /*height*/ 3
        )),
        before
    );
    view.jump_to_latest();
    insta::assert_snapshot!(text(&render(&mut view, &cells, /*width*/ 20, /*height*/ 3)), @"
    last line

    new output
    ");
}

#[test]
fn prepending_history_updates_pagination_position_before_the_next_frame() {
    let mut cells = vec![cell("current first"), cell("current last")];
    let mut view = TranscriptView::default();
    view.jump_to_entry(&cells, /*index*/ 0);
    assert!(view.near_start(&cells));
    cells.remove(/*index*/ 0);
    view.history_loaded(&cells, 0..0);
    let earlier = (0..100)
        .map(|index| cell(format!("earlier {index}")))
        .collect::<Vec<_>>();
    cells.splice(0..0, earlier);
    view.history_loaded(&cells, 0..100);
    assert!(!view.needs_history(&cells));
    let Position::Reading(anchor) = view.position else {
        panic!("reading anchor");
    };
    assert_eq!(
        (anchor.key, anchor.index),
        (EntryKey::cell(&cells[100]), 100)
    );
}

#[test]
fn beginning_wait_keeps_live_content_through_new_output_prepend_and_resize() {
    let mut cells = vec![cell("previous answer")];
    let live = cell("live zero\nlive one\nlive two\nlive three");
    let mut view = TranscriptView {
        history: TranscriptHistoryState::Partial,
        ..TranscriptView::default()
    };
    view.sync_live_tail(
        /*width*/ 20,
        /*key*/ None,
        |width| Some(live.display_hyperlink_lines(width)),
    );
    let before = render(&mut view, &cells, /*width*/ 20, /*height*/ 2);
    view.jump_to_beginning(&cells);
    cells.insert(/*index*/ 0, cell("older page"));
    view.history_loaded(&cells, 0..1);
    view.sync_live_tail(/*width*/ 20, /*key*/ None, |_| {
        Some(vec![HyperlinkLine::from("new streaming output")])
    });
    view.jump_to_beginning(&cells);
    assert_eq!(
        render(&mut view, &cells, /*width*/ 20, /*height*/ 2),
        before
    );
    let reading = view.position;
    render(&mut view, &cells, /*width*/ 12, /*height*/ 3);
    assert_eq!(
        (view.position, view.history),
        (reading, TranscriptHistoryState::LoadingBeginning)
    );
    assert_eq!(
        render(&mut view, &cells, /*width*/ 20, /*height*/ 2),
        before
    );
}

#[test]
fn transcript_interaction_cancels_pending_beginning_without_losing_received_history() {
    for key in [
        None,
        Some(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE)),
        Some(KeyEvent::new(KeyCode::F(3), KeyModifiers::NONE)),
        Some(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL)),
    ] {
        let mut cells = (0..20)
            .map(|index| cell(format!("line {index}")))
            .collect::<Vec<_>>();
        let mut view = TranscriptView {
            history: TranscriptHistoryState::Partial,
            ..TranscriptView::default()
        };
        render(&mut view, &cells, /*width*/ 24, /*height*/ 3);
        view.jump_to_beginning(&cells);
        match key {
            Some(key) if key.code == KeyCode::F(3) => view.begin_search(),
            Some(key) => {
                view.handle_key(key, &cells);
            }
            None => {
                view.handle_mouse(
                    MouseEvent {
                        kind: MouseEventKind::ScrollUp,
                        column: 0,
                        row: 0,
                        modifiers: KeyModifiers::NONE,
                    },
                    &cells,
                );
            }
        }
        assert_eq!(view.history, TranscriptHistoryState::LoadingOlder);
        let before = render(&mut view, &cells, /*width*/ 24, /*height*/ 3);
        cells.insert(/*index*/ 0, cell("received older history"));
        view.history_loaded(&cells, 0..1);
        view.history = TranscriptHistoryState::Complete;
        assert_eq!(
            render(&mut view, &cells, /*width*/ 24, /*height*/ 3),
            before
        );
    }
}

#[test]
fn returning_to_latest_cancels_beginning_intent_while_retaining_the_loaded_page() {
    for (beginning, latest) in [
        (
            KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL),
            KeyEvent::new(KeyCode::End, KeyModifiers::CONTROL),
        ),
        (
            KeyEvent::new(KeyCode::Char('<'), KeyModifiers::ALT),
            KeyEvent::new(KeyCode::Char('>'), KeyModifiers::ALT),
        ),
        (
            KeyEvent::new(KeyCode::Char('<'), KeyModifiers::ALT | KeyModifiers::SHIFT),
            KeyEvent::new(KeyCode::Char('>'), KeyModifiers::ALT | KeyModifiers::SHIFT),
        ),
        (
            KeyEvent::new(KeyCode::Char(','), KeyModifiers::ALT | KeyModifiers::SHIFT),
            KeyEvent::new(KeyCode::Char('.'), KeyModifiers::ALT | KeyModifiers::SHIFT),
        ),
    ] {
        let mut cells = vec![cell("newest answer\nlast line")];
        let mut view = TranscriptView {
            history: TranscriptHistoryState::Partial,
            ..TranscriptView::default()
        };
        render(&mut view, &cells, /*width*/ 20, /*height*/ 2);
        view.handle_key(beginning, &cells);
        assert_eq!(view.history, TranscriptHistoryState::LoadingBeginning);
        view.handle_key(latest, &cells);
        assert_eq!(
            (view.history, view.position),
            (TranscriptHistoryState::LoadingOlder, Position::Latest),
        );
        cells.insert(
            /*index*/ 0,
            cell("older page first line\nsecond line\nthird line"),
        );
        view.history = TranscriptHistoryState::Partial;
        view.history_loaded(&cells, 0..1);
        assert!(!view.needs_history(&cells));
        insta::allow_duplicates! {
            insta::assert_snapshot!(text(&render(&mut view, &cells, /*width*/ 20, /*height*/ 2)), @"
            newest answer
            last line
            ");
        }
        view.handle_key(beginning, &cells);
        assert_eq!(view.history, TranscriptHistoryState::LoadingBeginning);
    }
}

#[test]
fn transcript_jumps_leave_typing_and_effort_shortcuts_to_the_composer() {
    let cells = vec![cell("current answer")];
    let mut view = TranscriptView::default();
    for key in [
        KeyEvent::new(KeyCode::Char('<'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('>'), KeyModifiers::SHIFT),
        KeyEvent::new(KeyCode::Char(','), KeyModifiers::ALT),
        KeyEvent::new(KeyCode::Char('.'), KeyModifiers::ALT),
        KeyEvent::new(
            KeyCode::Char('<'),
            KeyModifiers::ALT | KeyModifiers::CONTROL,
        ),
        KeyEvent::new(KeyCode::Char('>'), KeyModifiers::ALT | KeyModifiers::SUPER),
        KeyEvent::new_with_kind(
            KeyCode::Char('<'),
            KeyModifiers::ALT,
            crossterm::event::KeyEventKind::Release,
        ),
    ] {
        assert!(view.handle_key(key, &cells).is_none(), "{key:?}");
        assert_eq!(view.position, Position::Latest);
    }
}

#[test]
fn scrolling_to_latest_cancels_beginning_and_keeps_selection_when_the_final_page_arrives() {
    for wheel in [false, true] {
        for select in [false, true] {
            let mut cells = vec![cell("older entry"), cell("latest answer")];
            let mut view = TranscriptView {
                history: TranscriptHistoryState::Partial,
                ..TranscriptView::default()
            };
            view.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL), &cells);
            render(&mut view, &cells, /*width*/ 20, /*height*/ 2);
            if select {
                view.begin_selection(&cells, /*column*/ 0, /*row*/ 0, /*clicks*/ 3);
            }
            let selected = view.selected_text(&cells);
            if wheel {
                view.handle_mouse(
                    MouseEvent {
                        kind: MouseEventKind::ScrollDown,
                        column: 0,
                        row: 0,
                        modifiers: KeyModifiers::NONE,
                    },
                    &cells,
                );
            } else {
                view.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE), &cells);
            }
            assert_eq!(
                (view.history, view.position, view.selected_text(&cells)),
                (
                    TranscriptHistoryState::LoadingOlder,
                    Position::Latest,
                    selected.clone()
                ),
            );
            cells.insert(/*index*/ 0, cell("final older page"));
            cells[1] = cell("revised older entry");
            view.history = TranscriptHistoryState::Complete;
            view.history_loaded(&cells, 0..1);
            assert_eq!(
                (view.position, view.selected_text(&cells)),
                (Position::Latest, selected),
            );
            assert!(
                text(&render(
                    &mut view, &cells, /*width*/ 20, /*height*/ 2
                ))
                .contains("latest answer")
            );
        }
    }
}

#[test]
fn single_row_scrolling_crosses_an_entry_separator_in_both_directions() {
    let cells = vec![cell("first"), cell("second")];
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 20, /*height*/ 1);
    let mut frames = Vec::new();
    for rows in [-1, -1, 1, 1] {
        view.scroll(&cells, rows);
        frames.push(text(&render(
            &mut view, &cells, /*width*/ 20, /*height*/ 1,
        )));
    }
    assert_eq!(frames, ["", "first", "", "second"]);
}

#[test]
fn selecting_within_an_edge_row_does_not_start_selection_autoscroll() {
    let cells = vec![cell(
        (0..40)
            .map(|row| format!("row {row:02}\n"))
            .collect::<String>(),
    )];
    for edge in [0, 5] {
        let mut view = TranscriptView::default();
        render(&mut view, &cells, /*width*/ 20, /*height*/ 6);
        view.scroll(&cells, /*rows*/ -12);
        let before = render(&mut view, &cells, /*width*/ 20, /*height*/ 6);
        view.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 0,
                row: edge,
                modifiers: KeyModifiers::NONE,
            },
            &cells,
        );
        for _ in 0..10 {
            assert!(!view.tick_selection(&cells));
            assert_eq!(
                render(&mut view, &cells, /*width*/ 20, /*height*/ 6),
                before
            );
        }
        view.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Drag(MouseButton::Left),
                column: 3,
                row: edge,
                modifiers: KeyModifiers::NONE,
            },
            &cells,
        );
        let selected = render(&mut view, &cells, /*width*/ 20, /*height*/ 6);
        assert_eq!(text(&selected), text(&before));
        assert_eq!(view.selected_text(&cells).as_deref(), Some("row"));
        for _ in 0..10 {
            assert!(!view.tick_selection(&cells));
            assert_eq!(
                render(&mut view, &cells, /*width*/ 20, /*height*/ 6),
                selected
            );
            assert_eq!(view.selected_text(&cells).as_deref(), Some("row"));
        }
        insta::assert_snapshot!(
            format!("horizontal_selection_at_edge_{edge}"),
            text(&selected)
        );
        let drag = MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: 3,
            row: edge,
            modifiers: KeyModifiers::NONE,
        };
        view.handle_mouse(
            MouseEvent {
                kind: if edge == 0 {
                    MouseEventKind::ScrollUp
                } else {
                    MouseEventKind::ScrollDown
                },
                ..drag
            },
            &cells,
        );
        render(&mut view, &cells, /*width*/ 20, /*height*/ 6);
        view.handle_mouse(drag, &cells);
        assert!(!view.tick_selection(&cells));
        // A deliberate vertical drag can return to its starting edge and still autoscroll.
        for row in [2, edge] {
            view.handle_mouse(
                MouseEvent {
                    kind: MouseEventKind::Drag(MouseButton::Left),
                    column: 3,
                    row,
                    modifiers: KeyModifiers::NONE,
                },
                &cells,
            );
            render(&mut view, &cells, /*width*/ 20, /*height*/ 6);
        }
        assert!(view.tick_selection(&cells));
    }
}

#[test]
fn wheel_scrolling_pauses_selection_autoscroll_until_the_next_drag() {
    let cells = vec![cell(
        (0..40)
            .map(|row| format!("row {row:02}\n"))
            .collect::<String>(),
    )];
    let mut frames = Vec::new();
    for (kind, edge) in [
        (MouseEventKind::ScrollUp, 0),
        (MouseEventKind::ScrollDown, 5),
    ] {
        let mut view = TranscriptView::default();
        render(&mut view, &cells, /*width*/ 20, /*height*/ 6);
        view.scroll(&cells, /*rows*/ -12);
        render(&mut view, &cells, /*width*/ 20, /*height*/ 6);
        view.begin_selection(&cells, /*column*/ 0, /*row*/ 2, /*clicks*/ 1);
        let drag = MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: 3,
            row: edge,
            modifiers: KeyModifiers::NONE,
        };
        // Wheel input before the first drag must not lose the original physical row.
        view.handle_mouse(MouseEvent { kind, ..drag }, &cells);
        assert!(!view.tick_selection(&cells));
        render(&mut view, &cells, /*width*/ 20, /*height*/ 6);
        view.handle_mouse(drag, &cells);
        assert!(view.tick_selection(&cells));
        render(&mut view, &cells, /*width*/ 20, /*height*/ 6);
        let selected = view.selected_text(&cells);
        view.handle_mouse(MouseEvent { kind, ..drag }, &cells);
        let after = render(&mut view, &cells, /*width*/ 20, /*height*/ 6);
        for _ in 0..10 {
            assert!(!view.tick_selection(&cells));
            assert_eq!(render(&mut view, &cells, /*width*/ 20, /*height*/ 6), after);
            assert_eq!(view.selected_text(&cells), selected);
        }
        frames.push(text(&after));
        view.handle_mouse(drag, &cells);
        assert!(view.tick_selection(&cells));
        render(&mut view, &cells, /*width*/ 20, /*height*/ 6);
        let selected = view.selected_text(&cells);
        view.handle_mouse(MouseEvent { kind, ..drag }, &cells);
        render(&mut view, &cells, /*width*/ 20, /*height*/ 6);
        view.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Up(MouseButton::Left),
                ..drag
            },
            &cells,
        );
        assert!(!view.tick_selection(&cells));
        assert_eq!(view.selected_text(&cells), selected);
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            row: 2,
            ..drag
        };
        view.handle_mouse(click, &cells);
        view.handle_mouse(MouseEvent { kind, ..click }, &cells);
        render(&mut view, &cells, /*width*/ 20, /*height*/ 6);
        view.handle_mouse(click, &cells);
        assert_eq!(view.selected_text(&cells), None);
    }
    insta::assert_snapshot!("wheel_stops_selection_autoscroll", frames.join("\n\n"));
}

#[test]
fn only_visible_history_is_laid_out_and_repeated_frames_reuse_it() {
    let renders = Arc::new(AtomicUsize::new(/*v*/ 0));
    let cells = (0..10_000)
        .map(|index| {
            Arc::new(TestCell {
                text: format!("message {index}"),
                renders: Arc::clone(&renders),
            }) as Arc<dyn HistoryCell>
        })
        .collect::<Vec<_>>();
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 40, /*height*/ 20);
    let initial = renders.load(Ordering::Relaxed);
    assert!(
        initial <= 12,
        "laid out {initial} history entries for a 20-row viewport"
    );
    render(&mut view, &cells, /*width*/ 40, /*height*/ 20);
    assert_eq!(renders.load(Ordering::Relaxed), initial);
    for highlight in [Some(9_999), Some(9_998), Some(9_998), None] {
        view.set_highlight(highlight);
        render(&mut view, &cells, /*width*/ 40, /*height*/ 20);
        assert_eq!(renders.load(Ordering::Relaxed), initial);
    }
}

#[test]
fn selection_copies_source_across_wraps_and_survives_prepend() {
    let mut cells = vec![cell("alpha beta gamma delta")];
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 10, /*height*/ 5);
    view.begin_selection(&cells, /*column*/ 0, /*row*/ 0, /*clicks*/ 1);
    view.extend_selection(/*column*/ 5, /*row*/ 2);
    assert_eq!(
        view.selected_text(&cells),
        Some("alpha beta gamma delta".to_string())
    );
    cells.insert(/*index*/ 0, cell("earlier page"));
    render(&mut view, &cells, /*width*/ 20, /*height*/ 5);
    assert_eq!(
        view.selected_text(&cells),
        Some("alpha beta gamma delta".to_string())
    );
}

#[test]
fn resize_keeps_the_logical_reading_anchor() {
    let mut cells = vec![cell(
        "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda",
    )];
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 10, /*height*/ 2);
    view.scroll(&cells, /*rows*/ -2);
    let Position::Reading(anchor) = view.position else {
        panic!("reading anchor");
    };
    render(&mut view, &cells, /*width*/ 20, /*height*/ 2);
    assert_eq!(view.position, Position::Reading(anchor));
    let replacement = cell("final answer");
    view.replace_range(&cells, 0..1, &replacement);
    cells.splice(0..1, [replacement]);
    assert_eq!(
        text(&render(
            &mut view, &cells, /*width*/ 20, /*height*/ 2
        )),
        "final answer\n"
    );
    assert!(!view.is_following());
    assert!(!view.unseen_activity);
    cells.push(cell("new output"));
    render(&mut view, &cells, /*width*/ 20, /*height*/ 2);
    assert!(view.unseen_activity);
}

#[test]
fn zero_sized_viewport_is_safe_and_empty_history_accepts_navigation() {
    let mut view = TranscriptView::default();
    render(&mut view, &[], /*width*/ 0, /*height*/ 0);
    view.scroll(&[], /*rows*/ -10);
    view.scroll(&[], /*rows*/ 10);
    assert_eq!(view.selected_text(&[]), None);
}

#[derive(Debug)]
struct MutableHistoryCell {
    version: Arc<AtomicUsize>,
    renders: Arc<AtomicUsize>,
}

impl HistoryCell for MutableHistoryCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        self.renders.fetch_add(/*val*/ 1, Ordering::Relaxed);
        let version = self.version.load(Ordering::Relaxed);
        (0..8)
            .map(|row| Line::from(format!("version {version} row {row}")))
            .collect()
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        self.display_lines(/*width*/ 80)
    }

    fn has_stable_transcript_height(&self) -> bool {
        false
    }
}

#[test]
fn mutable_history_formats_once_per_frame_and_refreshes_the_next_frame() {
    let version = Arc::new(AtomicUsize::new(/*v*/ 1));
    let renders = Arc::new(AtomicUsize::new(/*v*/ 0));
    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(MutableHistoryCell {
        version: Arc::clone(&version),
        renders: Arc::clone(&renders),
    })];
    let mut view = TranscriptView::default();
    let first = render(&mut view, &cells, /*width*/ 24, /*height*/ 5);
    assert_eq!(renders.load(Ordering::Relaxed), 1);
    version.store(/*val*/ 2, Ordering::Relaxed);
    let second = render(&mut view, &cells, /*width*/ 24, /*height*/ 5);
    assert_eq!(renders.load(Ordering::Relaxed), 2);
    assert!(text(&first).contains("version 1 row 3"));
    insta::assert_snapshot!(text(&second), @"
    version 2 row 3
    version 2 row 4
    version 2 row 5
    version 2 row 6
    version 2 row 7
    ");
    view.begin_search();
    view.paste_search("version 2");
    assert!(!view.advance_search(&cells));
    let matched = text(&render(
        &mut view, &cells, /*width*/ 24, /*height*/ 5,
    ));
    view.begin_selection(&cells, /*column*/ 0, /*row*/ 0, /*clicks*/ 3);
    version.store(/*val*/ 3, Ordering::Relaxed);
    view.end_selection(&cells);
    assert_eq!(
        text(&render(
            &mut view, &cells, /*width*/ 24, /*height*/ 5
        )),
        matched
    );
    view.handle_key(KeyCode::Enter.into(), &cells);
    assert!(!view.advance_search(&cells));
    assert!(
        text(&render(
            &mut view, &cells, /*width*/ 24, /*height*/ 5
        ))
        .contains("version 3")
    );
}

#[test]
fn selected_latest_content_keeps_new_activity_visible() {
    let mut cells = vec![cell("selected")];
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 24, /*height*/ 3);
    view.begin_selection(&cells, /*column*/ 0, /*row*/ 0, /*clicks*/ 3);
    view.scroll(&cells, /*rows*/ 10);
    cells.push(cell("new output"));
    render(&mut view, &cells, /*width*/ 24, /*height*/ 3);
    assert!(!view.is_following());
    insta::assert_snapshot!(view.status_line_with_navigation("Enter/Ctrl+C copy · Esc clear selection", crate::motion::MotionMode::Reduced).to_string(), @"New activity · Enter/Ctrl+C copy · Esc clear selection");
}

#[test]
fn live_reader_survives_commit_new_tail_resize_and_selection_then_rejoins_latest() {
    let mut cells = vec![cell("before")];
    let original = cell("live zero\nlive one\nlive two\nlive three\nlive four\nlive five");
    let mut view = TranscriptView::default();
    view.sync_live_tail(/*width*/ 20, /*key*/ None, |width| {
        Some(original.display_hyperlink_lines(width))
    });
    render(&mut view, &cells, /*width*/ 20, /*height*/ 2);
    view.scroll(&cells, /*rows*/ -2);
    let before = text(&render(
        &mut view, &cells, /*width*/ 20, /*height*/ 2,
    ));
    let reading = view.position;
    cells.push(Arc::clone(&original));
    view.sync_live_tail(/*width*/ 20, /*key*/ None, |_| None);
    assert_eq!(
        text(&render(
            &mut view, &cells, /*width*/ 20, /*height*/ 2
        )),
        before
    );
    view.sync_live_tail(
        /*width*/ 20,
        /*key*/ None,
        |_| Some(vec![HyperlinkLine::from("new tail")]),
    );
    assert_eq!(
        text(&render(
            &mut view, &cells, /*width*/ 20, /*height*/ 2
        )),
        before
    );
    render(&mut view, &cells, /*width*/ 9, /*height*/ 2);
    assert_eq!(view.position, reading);
    assert!(
        view.status_line_with_navigation("", crate::motion::MotionMode::Reduced)
            .to_string()
            .contains("New activity")
    );
    view.begin_selection(&cells, /*column*/ 0, /*row*/ 0, /*clicks*/ 3);
    assert_eq!(view.selected_text(&cells), Some("live two\n".to_string()));
    view.begin_search();
    assert!(!view.advance_search(&cells));
    view.handle_key(KeyCode::Esc.into(), &cells);
    assert_eq!(
        text(&render(
            &mut view, &cells, /*width*/ 20, /*height*/ 2
        )),
        before
    );
    view.jump_to_latest();
    assert!(view.snapshot().is_none());
    insta::assert_snapshot!(text(&render(&mut view, &cells, /*width*/ 20, /*height*/ 2)), @"

    new tail
    ");
}

#[test]
fn scrolling_out_of_held_live_content_releases_its_revision() {
    let mut cells = vec![cell("before")];
    let original = cell("zero\none\ntwo\nthree\nfour\nfive");
    let mut view = TranscriptView::default();
    view.sync_live_tail(/*width*/ 20, /*key*/ None, |width| {
        Some(original.display_hyperlink_lines(width))
    });
    render(&mut view, &cells, /*width*/ 20, /*height*/ 2);
    view.scroll(&cells, /*rows*/ -2);
    assert!(view.held_reading.is_some());
    cells.push(Arc::clone(&original));
    view.sync_live_tail(/*width*/ 20, /*key*/ None, |_| None);
    view.scroll(&cells, /*rows*/ -100);
    assert!(view.held_reading.is_none());
    view.scroll(&cells, /*rows*/ 100);
    assert!(view.is_following());
    assert!(
        text(&render(
            &mut view, &cells, /*width*/ 20, /*height*/ 2
        ))
        .contains("five")
    );
}

#[test]
fn repeated_resize_preserves_plain_and_markdown_passages_before_the_next_draw() {
    let passage = (0..40)
        .map(|index| format!("café{index:02}"))
        .collect::<Vec<_>>()
        .join(" ");
    let entries = [
        cell(passage.clone()),
        Arc::new(crate::history_cell::AgentMarkdownCell::new(
            passage,
            std::path::Path::new("/workspace"),
        )) as Arc<dyn HistoryCell>,
    ];
    for entry in entries {
        let cells = vec![cell("older entry"), entry];
        let mut view = TranscriptView::default();
        render(&mut view, &cells, /*width*/ 18, /*height*/ 3);
        view.scroll(&cells, /*rows*/ -5);
        // Resize can arrive before the first frame at this new reading position.
        let before_resize = view.position;
        let wide = text(&render(
            &mut view, &cells, /*width*/ 34, /*height*/ 3,
        ));
        let original = render(&mut view, &cells, /*width*/ 18, /*height*/ 3);
        assert_eq!(view.position, before_resize);
        let original_text = text(&original);
        let first_word = original_text
            .split_whitespace()
            .find(|word| word.starts_with("café"))
            .expect("visible Unicode passage");
        assert!(wide.contains(first_word));
        let narrow = render(&mut view, &cells, /*width*/ 12, /*height*/ 3);
        assert!(text(&narrow).contains(first_word));
        assert_eq!(
            render(&mut view, &cells, /*width*/ 18, /*height*/ 3),
            original
        );
        assert!(!view.is_following());
    }
}

#[test]
fn resizing_repeated_lines_keeps_the_same_occurrence() {
    let duplicate = "alpha beta gamma delta";
    let mut cells = vec![cell(format!(
        "{duplicate}\n{duplicate}\nsecond occurrence ends\ntrailing one\ntrailing two\ntrailing three"
    ))];
    let mut view = TranscriptView::default();
    view.jump_to_entry(&cells, /*index*/ 0);
    render(&mut view, &cells, /*width*/ 12, /*height*/ 2);
    view.scroll(&cells, /*rows*/ 3);
    let original = cells[0]
        .raw_lines()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    cells.insert(/*index*/ 0, cell("older report"));
    view.history_loaded(&cells, 0..1);
    let replacement = cell(format!("older report\n\n{original}"));
    view.replace_group(&cells, 0..2, &replacement);
    cells.splice(0..2, [replacement]);
    let wide = render(&mut view, &cells, /*width*/ 24, /*height*/ 2);
    insta::assert_snapshot!(text(&wide), @"
    alpha beta gamma delta
    second occurrence ends
    ");
    let narrow = render(&mut view, &cells, /*width*/ 12, /*height*/ 2);
    insta::assert_snapshot!(text(&narrow), @"
    gamma delta
    second
    ");
}

#[test]
fn resize_uses_the_passage_from_the_current_live_or_replaced_entry() {
    for entry_kind in ["live", "replaced"] {
        let initial = (0..40)
            .map(|index| format!("old{index:02}"))
            .collect::<Vec<_>>()
            .join(" ");
        let revised = (0..40)
            .map(|index| format!("café{index:02}"))
            .collect::<Vec<_>>()
            .join(" ");
        let mut view = TranscriptView::default();
        let mut cells = Vec::new();
        if entry_kind == "live" {
            view.sync_live_tail(/*width*/ 18, /*key*/ None, |_| {
                Some(crate::terminal_hyperlinks::plain_hyperlink_lines(vec![
                    initial.into(),
                ]))
            });
        } else {
            cells.push(cell(initial));
        }
        render(&mut view, &cells, /*width*/ 18, /*height*/ 3);
        view.scroll(&cells, /*rows*/ -5);
        if entry_kind == "live" {
            view.sync_live_tail(/*width*/ 18, /*key*/ None, |_| {
                Some(crate::terminal_hyperlinks::plain_hyperlink_lines(vec![
                    revised.clone().into(),
                ]))
            });
        } else {
            cells[0] = cell(revised.clone());
        }
        let current = render(&mut view, &cells, /*width*/ 18, /*height*/ 3);
        let current_text = text(&current);
        let first_word = current_text.split_whitespace().next().unwrap();
        if entry_kind == "live" {
            view.sync_live_tail(/*width*/ 34, /*key*/ None, |_| {
                Some(crate::terminal_hyperlinks::plain_hyperlink_lines(vec![
                    revised.clone().into(),
                ]))
            });
        }
        let wide = render(&mut view, &cells, /*width*/ 34, /*height*/ 3);
        assert!(text(&wide).contains(first_word), "{entry_kind}");
        if entry_kind == "live" {
            view.sync_live_tail(/*width*/ 18, /*key*/ None, |_| {
                Some(crate::terminal_hyperlinks::plain_hyperlink_lines(vec![
                    revised.into(),
                ]))
            });
        }
        assert_eq!(
            render(&mut view, &cells, /*width*/ 18, /*height*/ 3),
            current,
            "{entry_kind}"
        );
    }
}

#[test]
fn history_prefetch_counts_rows_in_preceding_entries() {
    for preceding_rows in [1, 100] {
        let cells = vec![
            cell(
                (0..preceding_rows)
                    .map(|row| format!("older {row}\n"))
                    .collect::<String>(),
            ),
            cell("current first\ncurrent last"),
        ];
        let mut view = TranscriptView::default();
        render(&mut view, &cells, /*width*/ 24, /*height*/ 4);
        assert_eq!(view.near_start(&cells), preceding_rows == 1);
        view.jump_to_entry(&cells, /*index*/ 1);
        assert_eq!(view.near_start(&cells), preceding_rows == 1);
        view.jump_to_entry(&cells, /*index*/ 0);
        assert!(view.near_start(&cells));
    }
}
