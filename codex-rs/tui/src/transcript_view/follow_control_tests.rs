//! Follow gestures and the visible return control share one reading-state contract.

use super::*;
use crate::history_cell::PlainHistoryCell;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;

fn cell(text: &str) -> Arc<dyn HistoryCell> {
    Arc::new(PlainHistoryCell::new(
        text.lines().map(|line| line.to_owned().into()).collect(),
    ))
}

fn paint(view: &mut TranscriptView, cells: &[Arc<dyn HistoryCell>], width: u16) -> Buffer {
    let area = Rect::new(/*x*/ 0, /*y*/ 0, width, /*height*/ 4);
    let mut buf = Buffer::empty(area);
    view.render(
        Rect::new(/*x*/ 0, /*y*/ 0, width, /*height*/ 3),
        &mut buf,
        cells,
    );
    view.render_follow_control(
        Some(Rect::new(
            /*x*/ 0, /*y*/ 3, width, /*height*/ 1,
        )),
        &mut buf,
    );
    buf
}

fn mouse(
    view: &mut TranscriptView,
    cells: &[Arc<dyn HistoryCell>],
    kind: MouseEventKind,
    column: u16,
    row: u16,
) {
    view.handle_mouse(
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        },
        cells,
    );
}

#[test]
fn empty_click_restores_following_but_not_an_existing_pause() {
    for following in [true, false] {
        let mut cells = vec![cell("one\ntwo\nthree\nfour")];
        let mut view = TranscriptView::default();
        paint(&mut view, &cells, /*width*/ 40);
        if !following {
            view.scroll(&cells, /*rows*/ -1);
            paint(&mut view, &cells, /*width*/ 40);
        }
        mouse(
            &mut view,
            &cells,
            MouseEventKind::Down(MouseButton::Left),
            /*column*/ 0,
            /*row*/ 0,
        );
        cells.push(cell("arrived during click"));
        paint(&mut view, &cells, /*width*/ 40);
        mouse(
            &mut view,
            &cells,
            MouseEventKind::Up(MouseButton::Left),
            /*column*/ 0,
            /*row*/ 0,
        );
        assert_eq!(
            (view.is_following(), view.selected_text(&cells)),
            (following, None)
        );
    }
}

#[test]
fn no_op_scroll_keeps_following_and_real_scroll_pauses() {
    let cells = vec![cell("one\ntwo")];
    let mut view = TranscriptView::default();
    paint(&mut view, &cells, /*width*/ 40);
    view.scroll(&cells, /*rows*/ -3);
    assert!(view.is_following());
    // Scrolling toward an unloaded page is meaningful even when the loaded rows fit.
    view.history = TranscriptHistoryState::Partial;
    view.scroll(&cells, /*rows*/ -3);
    assert!(!view.is_following());
    assert!(view.needs_history(&cells));
    view.history = TranscriptHistoryState::Complete;
    view.jump_to_latest();
    let cells = vec![cell("one\ntwo\nthree\nfour")];
    paint(&mut view, &cells, /*width*/ 40);
    view.scroll(&cells, /*rows*/ -3);
    assert!(view.can_return_to_latest());
    view.scroll(&cells, /*rows*/ 3);
    assert!(view.is_following());
}

#[test]
fn copying_live_selection_retains_the_displayed_revision() {
    let mut view = TranscriptView::default();
    let cells = Vec::new();
    view.sync_live_tail(/*width*/ 40, /*key*/ None, |_| {
        Some(vec![HyperlinkLine::from("original live text")])
    });
    paint(&mut view, &cells, /*width*/ 40);
    view.begin_selection(&cells, /*column*/ 0, /*row*/ 0, /*clicks*/ 3);
    let selected = view.selected_text(&cells).unwrap();
    view.sync_live_tail(/*width*/ 40, /*key*/ None, |_| {
        Some(vec![HyperlinkLine::from("changed live text")])
    });
    view.copy_selected_text_with(&cells, &selected, |_| {
        Ok(crate::clipboard_copy::CopyStatus::Confirmed)
    })
    .unwrap();
    let buffer = paint(&mut view, &cells, /*width*/ 40);
    assert!(!view.is_following());
    assert!(format!("{buffer:?}").contains("original live text"));
    view.jump_to_latest();
    assert!(format!("{:?}", paint(&mut view, &cells, /*width*/ 40)).contains("changed live text"));
}

#[test]
fn control_tracks_pause_activity_hover_resize_and_click() {
    let mut cells = vec![cell("one\ntwo\nthree\nfour\nfive")];
    let mut view = TranscriptView::default();
    paint(&mut view, &cells, /*width*/ 80);
    assert!(view.follow_control.area.is_none());
    view.scroll(&cells, /*rows*/ -2);
    let mut snapshots = vec![format!(
        "paused\n{:?}",
        paint(&mut view, &cells, /*width*/ 80)
    )];
    cells.push(cell("new output"));
    for width in [80, 20] {
        snapshots.push(format!(
            "activity at {width}\n{:?}",
            paint(&mut view, &cells, width)
        ));
        if width == 80 {
            let area = view.follow_control.area.unwrap();
            mouse(&mut view, &cells, MouseEventKind::Moved, area.x, area.y);
            snapshots.push(format!("hover\n{:?}", paint(&mut view, &cells, width)));
        }
    }
    let area = view.follow_control.area.unwrap();
    mouse(
        &mut view,
        &cells,
        MouseEventKind::Down(MouseButton::Left),
        area.x,
        area.y,
    );
    mouse(
        &mut view,
        &cells,
        MouseEventKind::Up(MouseButton::Left),
        area.x,
        area.y,
    );
    assert!(view.is_following());
    paint(&mut view, &cells, /*width*/ 20);
    assert!(view.follow_control.area.is_none());
    insta::assert_snapshot!("follow_control", snapshots.join("\n\n"));
}

#[test]
fn hidden_control_does_not_consume_clicks_and_drag_out_cancels() {
    let cells = vec![cell("one\ntwo\nthree\nfour")];
    let mut view = TranscriptView::default();
    paint(&mut view, &cells, /*width*/ 40);
    view.scroll(&cells, /*rows*/ -1);
    paint(&mut view, &cells, /*width*/ 40);
    let area = view.follow_control.area.unwrap();
    mouse(
        &mut view,
        &cells,
        MouseEventKind::Down(MouseButton::Left),
        area.x,
        area.y,
    );
    mouse(
        &mut view,
        &cells,
        MouseEventKind::Up(MouseButton::Left),
        /*column*/ 0,
        /*row*/ 0,
    );
    assert!(!view.is_following());
    let mut buf = Buffer::empty(Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 4,
    ));
    view.render_follow_control(/*area*/ None, &mut buf);
    assert!(
        view.handle_follow_control_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: area.x,
            row: area.y,
            modifiers: KeyModifiers::NONE,
        })
        .is_none()
    );
}

#[test]
fn height_changes_backfill_after_empty_click_but_preserve_deliberate_pause() {
    let rows = (0..60)
        .map(|row| format!("row {row:02}"))
        .collect::<Vec<_>>()
        .join("\n");
    let cells = vec![cell(&rows)];
    for pause in ["none", "scroll", "selection"] {
        let mut view = TranscriptView::default();
        let short = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 120, /*height*/ 24,
        );
        view.render(short, &mut Buffer::empty(short), &cells);
        if pause == "scroll" {
            view.scroll(&cells, /*rows*/ -6);
            view.render(short, &mut Buffer::empty(short), &cells);
        }
        if pause == "selection" {
            view.begin_selection(&cells, /*column*/ 0, /*row*/ 0, /*clicks*/ 3);
        } else {
            mouse(
                &mut view,
                &cells,
                MouseEventKind::Down(MouseButton::Left),
                /*column*/ 0,
                /*row*/ 0,
            );
            mouse(
                &mut view,
                &cells,
                MouseEventKind::Up(MouseButton::Left),
                /*column*/ 0,
                /*row*/ 0,
            );
        }
        let before = (view.visible[0].key, view.visible[0].row);
        let selected = view.selected_text(&cells);
        for height in [50, 24, 50] {
            let area = Rect::new(/*x*/ 0, /*y*/ 0, /*width*/ 120, height);
            view.render(area, &mut Buffer::empty(area), &cells);
            assert_eq!(view.is_following(), pause == "none");
            assert_eq!(view.selected_text(&cells), selected);
            if pause == "none" {
                assert_eq!(view.visible.len(), usize::from(height));
                assert_eq!(view.visible.last().unwrap().row, 59);
            } else {
                assert_eq!((view.visible[0].key, view.visible[0].row), before);
            }
        }
    }
}
