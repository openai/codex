//! Offline edits retain the same attachment bookkeeping as ordinary text edits.

use super::super::tests::new_test_composer;
use super::*;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use pretty_assertions::assert_eq;

#[test]
fn offline_delete_reveals_the_edited_row_without_moving_the_caret() {
    let (mut composer, _) = new_test_composer();
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 8,
    );
    let options = ComposerRenderOptions {
        max_height: Some(area.height),
        ..ComposerRenderOptions::default()
    };
    let text = (1..=20)
        .map(|n| format!("line {n:02} abc"))
        .collect::<Vec<_>>()
        .join("\n");
    composer.set_text_content(text.clone(), Vec::new(), Vec::new());
    let cursor = text.len() - 2;
    composer.draft.textarea.set_cursor(cursor);
    let mut buffer = Buffer::empty(area);
    composer.render_with_options(area, &mut buffer, /*mask_char*/ None, options);
    let (x, y) = composer.cursor_pos_with_options(area, options).unwrap();
    for _ in 0..3 {
        assert!(composer.handle_mouse(MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        }));
    }
    // Disconnect cleanup and blocked submits must not count as edits.
    for key in [KeyCode::Null, KeyCode::Enter, KeyCode::Tab] {
        composer.handle_restricted_key(key.into(), RestrictedInputMode::Disconnected);
        let mut buffer = Buffer::empty(area);
        composer.render_with_options(area, &mut buffer, /*mask_char*/ None, options);
        assert_eq!(composer.cursor_pos_with_options(area, options), None);
    }
    composer.handle_restricted_key(KeyCode::Delete.into(), RestrictedInputMode::Disconnected);
    let mut buffer = Buffer::empty(area);
    composer.render_with_options(area, &mut buffer, /*mask_char*/ None, options);
    assert_eq!(
        (composer.current_text(), composer.draft.textarea.cursor()),
        (text.replace("line 20 abc", "line 20 ac"), cursor)
    );
    assert!(composer.cursor_pos_with_options(area, options).is_some());
    let rows = buffer
        .content
        .chunks(usize::from(area.width))
        .map(|row| {
            row.iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!("offline_delete_after_wheel", rows);
}

#[test]
fn reconnect_expands_pastes_preserving_images_and_cursor() {
    let (mut composer, _) = new_test_composer();
    let paste = "x".repeat(LARGE_PASTE_CHAR_THRESHOLD + 1);
    composer.handle_paste(paste.clone());
    composer.attach_image(PathBuf::from("local.png"));
    composer.handle_paste(paste);
    let expanded = composer.current_text_with_pending();
    let images = composer.draft_snapshot().local_images;
    composer.handle_restricted_key(
        KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE),
        RestrictedInputMode::Disconnected,
    );
    let draft = composer.draft_snapshot();
    assert_eq!(
        (
            draft.text,
            draft.cursor,
            draft.local_images,
            draft.pending_pastes
        ),
        (
            format!("{expanded}!"),
            expanded.len() + 1,
            images,
            Vec::new()
        )
    );
    assert_eq!(draft.text_elements.len(), 1);
}

#[test]
fn reconnect_edit_removes_deleted_attachment_and_paste_metadata() {
    let (mut composer, _) = new_test_composer();
    composer.attach_image(PathBuf::from("local.png"));
    composer.handle_paste("x".repeat(LARGE_PASTE_CHAR_THRESHOLD + 1));
    composer.handle_restricted_key(
        KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
        RestrictedInputMode::Disconnected,
    );
    let draft = composer.draft_snapshot();
    assert_eq!(
        (draft.text, draft.local_images, draft.pending_pastes),
        (String::new(), Vec::new(), Vec::new())
    );
}

#[test]
fn reconnect_edit_cancels_history_preview_without_losing_original_draft() {
    let (mut composer, _) = new_test_composer();
    composer.set_text_content("original draft".into(), Vec::new(), Vec::new());
    composer.draft.textarea.set_cursor(/*pos*/ 14);
    composer.begin_history_search();
    composer.apply_history_search_result(HistorySearchResult::Found(HistoryEntry::new(
        "history preview".into(),
    )));
    composer.handle_restricted_key(
        KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE),
        RestrictedInputMode::Disconnected,
    );
    assert_eq!(composer.current_text(), "original draft!");
    assert!(!composer.history_search_active());
}

#[test]
fn unavailable_thread_dispatches_recovery_and_local_commands() {
    for command in [
        SlashCommand::New,
        SlashCommand::Clear,
        SlashCommand::Resume,
        SlashCommand::Agents,
        SlashCommand::MultiAgents,
        SlashCommand::Quit,
        SlashCommand::Exit,
        SlashCommand::Status,
        SlashCommand::DebugConfig,
        SlashCommand::Pwd,
        SlashCommand::Rollout,
        SlashCommand::Copy,
        SlashCommand::Raw,
    ] {
        let (mut composer, _) = new_test_composer();
        composer.set_text_content(format!("/{}", command.command()), Vec::new(), Vec::new());
        assert_eq!(
            composer.handle_restricted_key(
                KeyEvent::from(KeyCode::Enter),
                RestrictedInputMode::UnavailableThread,
            ),
            InputResult::Command(command),
        );
        assert_eq!(composer.current_text(), "");
    }
}

#[test]
fn unavailable_thread_dispatches_inline_commands_with_configured_submit_key() {
    for (text, command, args) in [
        ("/new recovery", SlashCommand::New, "recovery"),
        ("/clear recovery", SlashCommand::Clear, "recovery"),
        ("/resume saved", SlashCommand::Resume, "saved"),
        ("/raw on", SlashCommand::Raw, "on"),
    ] {
        let (mut composer, _) = new_test_composer();
        let key = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL);
        composer.submit_keys = vec![key_hint::ctrl(KeyCode::Char('s'))];
        composer.set_text_content(text.into(), Vec::new(), Vec::new());
        assert_eq!(
            composer.handle_restricted_key(
                KeyEvent::from(KeyCode::Enter),
                RestrictedInputMode::UnavailableThread
            ),
            InputResult::None,
        );
        assert_eq!(composer.current_text(), text);
        assert_eq!(
            composer.handle_restricted_key(key, RestrictedInputMode::UnavailableThread),
            InputResult::CommandWithArgs(command, args.into(), Vec::new()),
        );
    }
}

#[test]
fn restricted_input_preserves_blocked_drafts_and_attachments() {
    for mode in [
        RestrictedInputMode::Disconnected,
        RestrictedInputMode::UnavailableThread,
    ] {
        for text in [
            "keep my draft",
            "!echo hello",
            "/review",
            "/rename changed",
            "/compact",
            "/voice",
            "/unknown",
            "/status extra",
            "/status\nkeep my draft",
            "/clear recovery\nkeep my draft",
            "/new recovery\nkeep my draft",
            "/resume saved\nkeep my draft",
        ] {
            let (mut composer, _) = new_test_composer();
            composer.set_text_content(text.into(), Vec::new(), Vec::new());
            composer.attach_image(PathBuf::from("local.png"));
            let before = composer.draft_snapshot();
            for key in [KeyCode::Enter, KeyCode::Tab] {
                assert_eq!(
                    composer.handle_restricted_key(key.into(), mode),
                    InputResult::None
                );
                assert_eq!(composer.draft_snapshot(), before);
            }
        }
    }
    for text in ["/status", "/new", "/clear", "/raw on"] {
        let (mut composer, _) = new_test_composer();
        composer.set_text_content(text.into(), Vec::new(), Vec::new());
        assert_eq!(
            composer
                .handle_restricted_key(KeyCode::Enter.into(), RestrictedInputMode::Disconnected),
            InputResult::None
        );
        assert_eq!(composer.current_text(), text);
    }
}

#[test]
fn unavailable_thread_expands_pasted_commands_without_dispatching() {
    let (mut composer, _) = new_test_composer();
    let text = format!("/clear {}", "x".repeat(LARGE_PASTE_CHAR_THRESHOLD + 1));
    composer.handle_paste(text.clone());
    assert_ne!(composer.current_text(), text);
    assert_eq!(
        composer.handle_restricted_key(
            KeyCode::Enter.into(),
            RestrictedInputMode::UnavailableThread,
        ),
        InputResult::None
    );
    assert_eq!(composer.current_text(), text);
}
