//! Automatic copies retain completed mouse selections.

use super::*;
use crate::clipboard_copy::CopyStatus;
use crate::history_cell::AgentMarkdownCell;
use crate::transcript_view::tests::cell;
use crate::transcript_view::tests::render;
use crate::transcript_view::tests::text;
use crossterm::event::KeyModifiers;
use crossterm::event::MouseButton;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use pretty_assertions::assert_eq;
use std::time::Instant;

fn mouse(kind: MouseEventKind, column: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row: 0,
        modifiers: KeyModifiers::NONE,
    }
}

#[test]
fn copy_on_select_waits_for_release_and_retains_selection() {
    let cells = vec![cell("selected text")];
    for enabled in [false, true] {
        let mut view = TranscriptView {
            copy_on_select: enabled,
            ..Default::default()
        };
        render(&mut view, &cells, /*width*/ 20, /*height*/ 1);
        for event in [
            mouse(MouseEventKind::Down(MouseButton::Left), /*column*/ 0),
            mouse(MouseEventKind::Drag(MouseButton::Left), /*column*/ 5),
        ] {
            assert!(matches!(
                view.handle_mouse(event, &cells),
                Some(ViewAction::Changed)
            ));
        }
        let release = mouse(MouseEventKind::Up(MouseButton::Left), /*column*/ 8);
        let copied = match view.handle_mouse(release, &cells) {
            Some(ViewAction::CopyOnSelect(text)) => Some(text),
            Some(ViewAction::Changed) => None,
            _ => panic!("release must finish the selection"),
        };
        assert_eq!(copied.as_deref(), enabled.then_some("selected"));
        assert_eq!(view.selected_text(&cells).as_deref(), Some("selected"));
        assert!(view.handle_mouse(release, &cells).is_none());
        assert!(!view.tick_selection(&cells));
        if let Some(copied) = copied {
            let buffer = render(&mut view, &cells, /*width*/ 20, /*height*/ 1);
            let highlight = (0..20)
                .map(|column| {
                    if buffer[(column, 0)]
                        .modifier
                        .contains(ratatui::style::Modifier::REVERSED)
                    {
                        '^'
                    } else {
                        '·'
                    }
                })
                .collect::<String>();
            insta::assert_snapshot!(
                format!("{}\n{highlight}", text(&buffer)),
                @"
                selected text
                ^^^^^^^^············
                "
            );
            for result in [
                Ok(CopyStatus::Confirmed),
                Ok(CopyStatus::Unconfirmed),
                Err("clipboard unavailable".to_owned()),
            ] {
                view.copy_selected_text_with(
                    &cells,
                    &copied,
                    /*clear_selection*/ false,
                    |_, _format| Ok(CopyStatus::Pending(1)),
                )
                .unwrap();
                assert_eq!(
                    view.finish_copy(&cells, &(1, result.clone()), /*current*/ true),
                    Some(false)
                );
                assert_eq!(view.selected_text(&cells).as_deref(), Some(copied.as_str()));
                assert_eq!(
                    view.copy_feedback
                        .as_ref()
                        .map(|feedback| (feedback.result, feedback.characters)),
                    Some((result.map_err(|_| ()), copied.chars().count()))
                );
            }
        }
    }
}

#[test]
fn copy_on_select_copies_word_and_line_on_release_without_dragging() {
    let cells = vec![cell("alpha beta gamma")];
    let mut view = TranscriptView {
        copy_on_select: true,
        ..Default::default()
    };
    render(&mut view, &cells, /*width*/ 24, /*height*/ 1);
    let down = mouse(MouseEventKind::Down(MouseButton::Left), /*column*/ 7);
    let up = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        ..down
    };
    for expected in [None, Some("beta"), Some("alpha beta gamma")] {
        if let Some((at, ..)) = &mut view.last_click {
            *at = Instant::now();
        }
        assert!(matches!(
            view.handle_mouse(down, &cells),
            Some(ViewAction::Changed)
        ));
        let copied = match view.handle_mouse(up, &cells) {
            Some(ViewAction::CopyOnSelect(text)) => Some(text),
            Some(ViewAction::Changed) => None,
            _ => panic!("release must finish the repeated click"),
        };
        assert_eq!(
            (copied.as_deref(), view.selected_text(&cells).as_deref()),
            (expected, expected)
        );
        assert!(view.handle_mouse(up, &cells).is_none());
    }
}

#[test]
fn copy_on_select_preserves_link_activation_and_ignores_empty_drags() {
    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(AgentMarkdownCell::new(
        "[example.com](https://example.com/docs)".into(),
        std::path::Path::new("/"),
    ))];
    for (drag, release) in [(None, 2), (Some(5), 5), (Some(5), 2)] {
        let mut view = TranscriptView {
            copy_on_select: true,
            ..Default::default()
        };
        render(&mut view, &cells, /*width*/ 30, /*height*/ 1);
        let down = mouse(MouseEventKind::Down(MouseButton::Left), /*column*/ 2);
        assert!(matches!(
            view.handle_mouse(down, &cells),
            Some(ViewAction::Changed)
        ));
        if let Some(column) = drag {
            assert!(matches!(
                view.handle_mouse(
                    mouse(MouseEventKind::Drag(MouseButton::Left), column),
                    &cells
                ),
                Some(ViewAction::Changed)
            ));
        }
        let action = view.handle_mouse(
            mouse(MouseEventKind::Up(MouseButton::Left), release),
            &cells,
        );
        match (drag, release, action) {
            (None, _, Some(ViewAction::OpenLink(url))) => {
                assert_eq!(url, "https://example.com/docs");
                assert_eq!(view.selected_text(&cells), None);
            }
            (Some(_), 5, Some(ViewAction::CopyOnSelect(copied))) => {
                assert_eq!(copied, "exa");
                assert_eq!(view.selected_text(&cells).as_deref(), Some("exa"));
            }
            (Some(_), 2, Some(ViewAction::Changed)) => {
                assert_eq!(view.selected_text(&cells), None);
            }
            _ => panic!("only a nonempty link drag should copy"),
        }
    }
}
