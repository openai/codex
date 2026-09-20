//! Mouse edits preserve buffered typing and keep placeholder payloads in sync.

use super::super::tests::new_test_composer;
use super::*;
use crossterm::event::MouseButton::Left;
use crossterm::event::MouseEventKind::Down;
use crossterm::event::MouseEventKind::Up;
use pretty_assertions::assert_eq;

fn mouse(composer: &mut ChatComposer, area: Rect, kind: MouseEventKind, column: u16, row: u16) {
    let event = MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    };
    assert!(composer.prepare_mouse(event));
    composer.render(area, &mut Buffer::empty(area));
    assert!(composer.handle_mouse(event));
}

#[test]
fn composer_mouse_edits_flush_typing_and_reconcile_placeholders() {
    let (mut composer, _rx) = new_test_composer();
    composer.insert_str("hello");
    composer
        .draft
        .paste_burst
        .begin_with_retro_grabbed("\nworld".into(), Instant::now());
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 4,
    );
    composer.render(area, &mut Buffer::empty(area));
    let (x, y) = composer.cursor_pos(area).unwrap();
    mouse(&mut composer, area, Down(Left), x - 5, y);
    composer.insert_str("X");
    assert_eq!(composer.current_text(), "hello\nXworld");
    composer.handle_paste("z".repeat(LARGE_PASTE_CHAR_THRESHOLD + 1));
    for path in [Some("first.png"), Some("second.png"), None] {
        composer.render(area, &mut Buffer::empty(area));
        let (x, y) = composer.cursor_pos(area).unwrap();
        // Double-clicking inside the label selects the entire atomic element.
        for kind in [Down(Left), Up(Left), Down(Left), Up(Left)] {
            mouse(&mut composer, area, kind, x - 1, y);
        }
        if let Some(path) = path {
            let path = PathBuf::from(path);
            composer.attach_image(path.clone());
            assert_eq!(composer.attachments.local_image_paths(), vec![path]);
        } else {
            composer.handle_key_event(KeyCode::Char('é').into());
            assert!(composer.attachments.is_empty());
        }
        assert!(composer.draft.pending_pastes.is_empty());
    }
}

#[test]
fn selected_text_navigation_does_not_recall_history_or_select_images() {
    for (key, remote) in [
        (KeyCode::Up, false),
        (KeyCode::Down, false),
        (KeyCode::Up, true),
    ] {
        let (mut composer, _rx) = new_test_composer();
        for text in ["first", "second"] {
            composer
                .history
                .record_local_submission(HistoryEntry::new(text.into()));
        }
        composer.handle_key_event(KeyCode::Up.into());
        if remote {
            composer.set_remote_image_urls(vec!["https://example.com/image.png".into()]);
        }
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 10,
        );
        composer.render(area, &mut Buffer::empty(area));
        let (x, y) = composer.cursor_pos(area).unwrap();
        if remote {
            composer.draft.textarea.set_cursor(/*pos*/ 0);
            composer.handle_key_event(KeyCode::Up.into());
        }
        let (start, end) = if key == KeyCode::Up {
            (x, x - 6)
        } else {
            (x - 6, x)
        };
        for (kind, column) in [
            (Down(Left), start),
            (MouseEventKind::Drag(Left), end),
            (Up(Left), end),
        ] {
            mouse(&mut composer, area, kind, column, y);
        }
        assert_eq!(composer.attachments.selected_remote_image_index, None);
        composer.handle_key_event(key.into());
        assert_eq!(
            (
                composer.current_text(),
                composer.draft.textarea.mouse_selection_range(),
                composer.attachments.selected_remote_image_index
            ),
            ("second".to_string(), None, None)
        );
    }
}
