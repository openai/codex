//! Sticky context follows the visible turn while leaving source text and hit rows intact.

use super::*;
use crate::history_cell::new_user_prompt;
use crate::transcript_view::tests::cell;
use crate::transcript_view::tests::render;
use crate::transcript_view::tests::text;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use crossterm::event::MouseButton;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use pretty_assertions::assert_eq;
use ratatui::style::Modifier;

fn user(message: &str) -> Arc<dyn HistoryCell> {
    Arc::new(new_user_prompt(
        message.into(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    ))
}

#[test]
fn prompt_header_tracks_reading_and_yields_to_visible_prompts_and_short_screens() {
    let mut cells = vec![
        user("First question"),
        cell("one\ntwo\nthree\nfour\nfive"),
        user("Second café\nquestion"),
        user("\u{0007}"),
        cell("six\nseven\neight\nnine\nten"),
    ];
    let mut view = TranscriptView::default();
    let mut snapshots = Vec::new();
    snapshots.push(format!(
        "Latest\n{}",
        text(&render(
            &mut view, &cells, /*width*/ 30, /*height*/ 5
        ))
    ));
    assert_eq!(view.area.y, 1);
    view.jump_to_entry(&cells, /*index*/ 1);
    snapshots.push(format!(
        "Earlier answer\n{}",
        text(&render(
            &mut view, &cells, /*width*/ 30, /*height*/ 5
        ))
    ));
    assert_eq!(view.visible[0].index, 1);
    cells.insert(/*index*/ 0, user("Prepend history"));
    view.history_loaded(&cells, 0..1);
    snapshots.push(format!(
        "After prepend\n{}",
        text(&render(
            &mut view, &cells, /*width*/ 30, /*height*/ 5
        ))
    ));
    view.jump_to_entry(&cells, /*index*/ 1);
    snapshots.push(format!(
        "Prompt visible\n{}",
        text(&render(
            &mut view, &cells, /*width*/ 30, /*height*/ 5
        ))
    ));
    assert_eq!(view.area.y, 0);
    view.jump_to_latest();
    snapshots.push(format!(
        "Short screen\n{}",
        text(&render(
            &mut view, &cells, /*width*/ 30, /*height*/ 3
        ))
    ));
    assert_eq!(view.area.y, 0);
    insta::assert_snapshot!(snapshots.join("\n\n"));
}

#[test]
fn header_sanitizes_controls_and_truncates_by_display_width() {
    let cells = vec![user("\x1b[31m界 café\r\nnext question"), cell("answer")];
    let line = line(&cells, /*first*/ 1, /*width*/ 18).unwrap();
    assert!(line.width() <= 18);
    insta::assert_snapshot!(line.to_string());
}

#[test]
fn mouse_selection_preserves_header_reservation_at_turn_boundaries() {
    let cells = vec![
        user("First question"),
        cell("first answer"),
        user("Second question"),
        cell("done 10:16 AM"),
    ];
    for width in [40, 15] {
        for height in [6, 7, 8] {
            for drag in [false, true] {
                let mut view = TranscriptView::default();
                let before = render(&mut view, &cells, width, height);
                let down = MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    column: 5,
                    row: height - 1,
                    modifiers: KeyModifiers::NONE,
                };
                view.handle_mouse(down, &cells);
                assert_eq!(
                    render(&mut view, &cells, width, height),
                    before,
                    "mouse down at {width}x{height}"
                );
                if !drag {
                    view.handle_mouse(
                        MouseEvent {
                            kind: MouseEventKind::Up(MouseButton::Left),
                            ..down
                        },
                        &cells,
                    );
                    assert_eq!(
                        (
                            render(&mut view, &cells, width, height),
                            view.selected_text(&cells),
                            view.is_following(),
                        ),
                        (before, None, true),
                        "empty release at {width}x{height}"
                    );
                    continue;
                }
                let end = MouseEvent {
                    kind: MouseEventKind::Drag(MouseButton::Left),
                    column: 13,
                    ..down
                };
                view.handle_mouse(end, &cells);
                let selected = render(&mut view, &cells, width, height);
                assert_eq!(text(&selected), text(&before));
                assert_eq!(view.selected_text(&cells).as_deref(), Some("10:16 AM"));
                view.handle_mouse(
                    MouseEvent {
                        kind: MouseEventKind::Up(MouseButton::Left),
                        ..end
                    },
                    &cells,
                );
                assert_eq!(render(&mut view, &cells, width, height), selected);
                let Some(ViewAction::Copy(copied)) = view.handle_key(
                    KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                    &cells,
                ) else {
                    panic!("copy selection at {width}x{height}");
                };
                assert_eq!(copied, "10:16 AM");
                assert_eq!(
                    selected
                        .content
                        .iter()
                        .filter(|cell| cell.modifier.contains(Modifier::REVERSED))
                        .map(ratatui::buffer::Cell::symbol)
                        .collect::<String>(),
                    copied
                );
            }
        }
    }
}

#[test]
fn paused_header_reservation_survives_copy_until_scroll_or_resize() {
    let cells = vec![
        user("First question"),
        cell("first answer"),
        user("Second question"),
        cell("done 10:16 AM"),
    ];
    for scroll in [false, true] {
        let mut view = TranscriptView::default();
        let before = render(&mut view, &cells, /*width*/ 40, /*height*/ 7);
        view.begin_selection(&cells, /*column*/ 0, /*row*/ 6, /*clicks*/ 2);
        assert_eq!(
            text(&render(
                &mut view, &cells, /*width*/ 40, /*height*/ 7
            )),
            text(&before)
        );
        let selected = view.selected_text(&cells).expect("selected word");
        assert_eq!(selected, "done");
        view.copy_selected_text_with(&cells, &selected, |_| {
            Ok(crate::clipboard_copy::CopyStatus::Confirmed)
        })
        .expect("confirmed copy");
        let bookmark = view.bookmark(&cells);
        view.set_presentation(/*detailed*/ true, HistoryRenderMode::Rich);
        view.jump_to_entry(&cells, /*index*/ 0);
        render(&mut view, &cells, /*width*/ 40, /*height*/ 7);
        view.restore_bookmark(bookmark);
        assert_eq!(
            (
                render(&mut view, &cells, /*width*/ 40, /*height*/ 7),
                view.is_following(),
            ),
            (before, false)
        );
        let height = if scroll {
            view.scroll(&cells, /*rows*/ -1);
            7
        } else {
            8
        };
        render(&mut view, &cells, /*width*/ 40, height);
        assert_eq!(view.area.y, 1);
    }
}
