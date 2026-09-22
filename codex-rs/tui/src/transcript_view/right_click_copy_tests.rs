//! Right-click copying uses existing delivery semantics without moving the reading position.

use super::*;
use crate::clipboard_copy::CopyStatus;
use crate::transcript_view::tests::cell;
use crate::transcript_view::tests::render;
use crate::transcript_view::tests::text;
use pretty_assertions::assert_eq;
use std::time::Instant;

#[test]
fn right_click_retains_selection_until_confirmed_and_preserves_reading_position() {
    let cells = vec![cell("selected text\nsecond line\nthird line\nlatest line")];
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 40, /*height*/ 3);
    view.scroll(&cells, /*rows*/ -1);
    render(&mut view, &cells, /*width*/ 40, /*height*/ 3);
    view.begin_selection(&cells, /*column*/ 0, /*row*/ 0, /*clicks*/ 3);
    view.end_drag();
    let position = view.position;
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: 4,
        row: 0,
        modifiers: KeyModifiers::NONE,
    };
    let mut frames = Vec::new();
    for result in [
        Err("clipboard unavailable".to_owned()),
        Ok(CopyStatus::Unconfirmed),
        Ok(CopyStatus::Confirmed),
    ] {
        let Some(ViewAction::Copy(selected)) = view.handle_mouse(click, &cells) else {
            panic!("right-click must request a copy without following new output");
        };
        assert_eq!(selected, "selected text\n");
        let copied = view.copy_selected_text_with(&cells, &selected, |copied| {
            assert_eq!(copied, selected);
            result.clone()
        });
        assert_eq!(copied, result);
        assert_eq!(
            (view.selected_text(&cells), view.position),
            (
                (result != Ok(CopyStatus::Confirmed)).then_some(selected.clone()),
                position,
            )
        );
        view.show_copy_feedback(&copied, selected.chars().count());
        let mut buffer = Buffer::empty(Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 4,
        ));
        view.render(
            Rect::new(
                /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 3,
            ),
            &mut buffer,
            &cells,
        );
        view.render_composer_gap(
            Some(Rect::new(
                /*x*/ 0, /*y*/ 3, /*width*/ 40, /*height*/ 1,
            )),
            /*hint*/ None,
            &mut buffer,
            Instant::now(),
        );
        frames.push(format!("{result:?}\n{}", text(&buffer)));
    }
    insta::assert_snapshot!(frames.join("\n\n"));
    assert!(view.handle_mouse(click, &cells).is_none());
}

#[test]
fn right_click_requires_a_nonempty_selection_and_a_press_inside_the_transcript() {
    let cells = vec![cell("selected text")];
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 40, /*height*/ 3);
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: 4,
        row: 0,
        modifiers: KeyModifiers::NONE,
    };
    assert!(view.handle_mouse(click, &cells).is_none());
    view.begin_selection(&cells, /*column*/ 0, /*row*/ 0, /*clicks*/ 1);
    assert!(view.handle_mouse(click, &cells).is_none());
    view.begin_selection(&cells, /*column*/ 0, /*row*/ 0, /*clicks*/ 3);
    // Outside presses must stay ignored even while an active drag owns mouse events.
    for event in [
        MouseEvent {
            column: 40,
            ..click
        },
        MouseEvent { row: 3, ..click },
        MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Right),
            ..click
        },
    ] {
        assert!(view.handle_mouse(event, &cells).is_none());
        assert_eq!(view.selected_text(&cells).as_deref(), Some("selected text"));
    }
}
