//! Link activation shares pointer gestures with selection without opening after a drag or scroll.

use super::*;
use crate::history_cell::AgentMarkdownCell;
use pretty_assertions::assert_eq;

fn transcript(markdown: &str, width: u16) -> (TranscriptView, Vec<Arc<dyn HistoryCell>>) {
    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(AgentMarkdownCell::new(
        markdown.into(),
        std::path::Path::new("/"),
    ))];
    let mut view = TranscriptView::default();
    let area = Rect::new(/*x*/ 2, /*y*/ 1, width, /*height*/ 8);
    view.render(area, &mut Buffer::empty(area), &cells);
    (view, cells)
}

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

#[test]
fn ordinary_click_opens_bare_and_markdown_links_on_release() {
    for markdown in [
        "https://example.com/docs",
        "[docs](https://example.com/docs)",
    ] {
        for width in [12, 60] {
            let (mut view, cells) = transcript(markdown, width);
            let down = mouse(
                MouseEventKind::Down(MouseButton::Left),
                /*column*/ 4,
                /*row*/ 1,
            );
            assert!(matches!(
                view.handle_mouse(down, &cells),
                Some(ViewAction::Changed)
            ));
            // Match the app's redraw before dispatching the next mouse event.
            let area = view.area;
            view.render(area, &mut Buffer::empty(area), &cells);
            let up = MouseEvent {
                kind: MouseEventKind::Up(MouseButton::Left),
                ..down
            };
            let Some(ViewAction::OpenLink(url)) = view.handle_mouse(up, &cells) else {
                panic!("click should open {markdown}");
            };
            assert_eq!(
                (
                    url.as_str(),
                    view.selected_text(&cells),
                    view.is_following()
                ),
                ("https://example.com/docs", None, true)
            );
            assert!(view.handle_mouse(up, &cells).is_none());
        }
    }
}

#[test]
fn dragging_a_link_selects_text_and_never_opens_it() {
    for return_to_origin in [false, true] {
        let (mut view, cells) = transcript(
            "[documentation](https://example.com/docs)",
            /*width*/ 60,
        );
        view.handle_mouse(
            mouse(
                MouseEventKind::Down(MouseButton::Left),
                /*column*/ 4,
                /*row*/ 1,
            ),
            &cells,
        );
        view.handle_mouse(
            mouse(
                MouseEventKind::Drag(MouseButton::Left),
                /*column*/ 7,
                /*row*/ 1,
            ),
            &cells,
        );
        assert_eq!(view.selected_text(&cells).as_deref(), Some("doc"));
        let column = if return_to_origin { 4 } else { 7 };
        let action = view.handle_mouse(
            mouse(
                MouseEventKind::Up(MouseButton::Left),
                column,
                /*row*/ 1,
            ),
            &cells,
        );
        assert!(matches!(action, Some(ViewAction::Changed)));
        assert_eq!(
            view.selected_text(&cells).as_deref(),
            (!return_to_origin).then_some("doc")
        );
    }
}

#[test]
fn scrolling_or_releasing_elsewhere_cancels_link_activation() {
    for interruption in [
        mouse(MouseEventKind::ScrollUp, /*column*/ 4, /*row*/ 1),
        mouse(
            MouseEventKind::ScrollDown,
            /*column*/ 4,
            /*row*/ 1,
        ),
        mouse(
            MouseEventKind::Up(MouseButton::Left),
            /*column*/ 5,
            /*row*/ 1,
        ),
        mouse(
            MouseEventKind::Up(MouseButton::Left),
            /*column*/ 0,
            /*row*/ 0,
        ),
    ] {
        let (mut view, cells) = transcript("[docs](https://example.com/docs)", /*width*/ 60);
        view.handle_mouse(
            mouse(
                MouseEventKind::Down(MouseButton::Left),
                /*column*/ 4,
                /*row*/ 1,
            ),
            &cells,
        );
        assert!(!matches!(
            view.handle_mouse(interruption, &cells),
            Some(ViewAction::OpenLink(_))
        ));
        assert!(!matches!(
            view.handle_mouse(
                mouse(
                    MouseEventKind::Up(MouseButton::Left),
                    /*column*/ 4,
                    /*row*/ 1
                ),
                &cells
            ),
            Some(ViewAction::OpenLink(_))
        ));
    }
}

#[test]
fn modified_clicks_still_open_immediately() {
    for modifiers in [KeyModifiers::CONTROL, KeyModifiers::SUPER] {
        let (mut view, cells) = transcript("[docs](https://example.com/docs)", /*width*/ 60);
        let event = MouseEvent {
            modifiers,
            ..mouse(
                MouseEventKind::Down(MouseButton::Left),
                /*column*/ 4,
                /*row*/ 1,
            )
        };
        let Some(ViewAction::OpenLink(url)) = view.handle_mouse(event, &cells) else {
            panic!("modified click should open link");
        };
        assert_eq!(url, "https://example.com/docs");
        assert!(view.selection.is_none());
    }
}
