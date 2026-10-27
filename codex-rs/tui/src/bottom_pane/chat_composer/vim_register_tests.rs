#![cfg(test)]

//! Rich register coverage through real composer input and submission preparation.

use super::super::ChatComposer;
use super::super::LARGE_PASTE_CHAR_THRESHOLD;
use super::super::tests::new_test_composer;
use super::super::tests::snapshot_composer_state_with_width;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use std::path::PathBuf;

fn key(composer: &mut ChatComposer, code: KeyCode) {
    let _ = composer.handle_key_event(KeyEvent::new(code, KeyModifiers::NONE));
}

fn commands(composer: &mut ChatComposer, text: &str) {
    for ch in text.chars() {
        key(composer, KeyCode::Char(ch));
    }
}

fn rich_composer() -> ChatComposer {
    let (mut composer, _) = new_test_composer();
    composer.set_disable_paste_burst(/*disabled*/ true);
    composer.set_vim_enabled(/*enabled*/ true);
    composer.insert_selected_mention(0..0, "$skill", Some("/original/skill/SKILL.md"));
    composer.attach_image(PathBuf::from("/original/photo.png"));
    composer.handle_paste("z".repeat(LARGE_PASTE_CHAR_THRESHOLD + 1));
    composer.popups.active = super::super::ActivePopup::None;
    key(&mut composer, KeyCode::Esc);
    commands(&mut composer, "0");
    composer
}

#[test]
fn yank_and_repeated_paste_create_fresh_rich_objects() {
    let mut composer = rich_composer();
    let original = composer.current_text_with_pending();
    commands(&mut composer, "y$p.");
    assert_eq!(composer.draft.textarea.text_elements().len(), 9);
    assert_eq!(
        composer.local_image_paths(),
        vec![PathBuf::from("/original/photo.png"); 3]
    );
    assert_eq!(composer.draft.pending_pastes.len(), 3);
    assert_eq!(composer.mention_bindings().len(), 3);
    assert!(
        composer
            .mention_bindings()
            .iter()
            .all(|binding| binding.path == "/original/skill/SKILL.md")
    );
    assert_eq!(
        composer.current_text_with_pending().matches('z').count(),
        original.matches('z').count() * 3
    );
    let ids: std::collections::HashSet<_> = composer
        .draft
        .textarea
        .text_element_snapshots()
        .into_iter()
        .map(|element| element.id)
        .collect();
    assert_eq!(ids.len(), 9);
}

#[test]
fn deleted_rich_register_survives_clear_and_linewise_paste() {
    let mut composer = rich_composer();
    let original = composer.current_text_with_pending();
    commands(&mut composer, "dd");
    assert_eq!(composer.local_image_paths(), Vec::<PathBuf>::new());
    assert_eq!(
        composer.draft.pending_pastes,
        Vec::<(String, String)>::new()
    );
    composer.set_text_content("new".to_owned(), Vec::new(), Vec::new());
    commands(&mut composer, "p");
    assert_eq!(
        composer.current_text_with_pending(),
        format!("new\n{original}")
    );
    assert_eq!(
        composer.local_image_paths(),
        vec![PathBuf::from("/original/photo.png")]
    );
    assert_eq!(
        composer.mention_bindings()[0].path,
        "/original/skill/SKILL.md"
    );
}

#[test]
fn composer_handoff_preserves_only_the_live_rich_register() {
    let mut previous = rich_composer();
    commands(&mut previous, "y$A");
    let _ = previous.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL));
    key(&mut previous, KeyCode::Esc);
    commands(&mut previous, "yy");
    // The captured Ctrl-Y still owns the old generation, distinct from the current yank.
    assert_eq!(previous.vim_register.payloads.len(), 2);
    let original = previous.current_text_with_pending();
    let (mut composer, _) = new_test_composer();
    composer.set_disable_paste_burst(/*disabled*/ true);
    composer.set_vim_enabled(/*enabled*/ true);
    composer.set_text_content("new".to_owned(), Vec::new(), Vec::new());
    composer.restore_kill_buffer_snapshot(previous.take_kill_buffer_snapshot());
    drop(previous);
    assert_eq!(composer.vim_register.payloads.len(), 1);
    key(&mut composer, KeyCode::Esc);
    commands(&mut composer, "u.");
    assert_eq!(composer.current_text(), "new");
    commands(&mut composer, "p");
    assert_eq!(
        composer.current_text_with_pending(),
        format!("new\n{original}")
    );
    assert_eq!(composer.draft.textarea.text_elements().len(), 6);
    assert_eq!(
        composer.local_image_paths(),
        vec![PathBuf::from("/original/photo.png"); 2]
    );
    assert_eq!(composer.draft.pending_pastes.len(), 2);
    assert_eq!(composer.mention_bindings().len(), 2);
    assert!(
        composer
            .mention_bindings()
            .iter()
            .all(|binding| binding.path == "/original/skill/SKILL.md")
    );
}

#[test]
fn insert_yank_and_dot_preserve_rich_payloads() {
    for mode in ["i", "R"] {
        let mut composer = rich_composer();
        commands(&mut composer, "d$");
        commands(&mut composer, mode);
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL));
        commands(&mut composer, " foo");
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL));
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL));
        key(&mut composer, KeyCode::Esc);
        let edited = composer.current_text_with_pending();
        commands(&mut composer, "u");
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        assert_eq!(composer.current_text_with_pending(), edited);
        commands(&mut composer, "0.");
        assert_eq!(
            composer.local_image_paths(),
            vec![PathBuf::from("/original/photo.png"); 2]
        );
        assert_eq!(composer.draft.pending_pastes.len(), 2);
        assert_eq!(composer.mention_bindings().len(), 2);
        assert!(
            composer
                .mention_bindings()
                .iter()
                .all(|binding| binding.path == "/original/skill/SKILL.md")
        );
        assert_eq!(
            composer.current_text_with_pending().matches("foo").count(),
            if mode == "i" { 2 } else { 1 }
        );
    }
}

#[test]
fn insert_yank_repeat_keeps_original_text_after_word_deletion() {
    let (mut composer, _) = new_test_composer();
    composer.set_disable_paste_burst(/*disabled*/ true);
    composer.set_vim_enabled(/*enabled*/ true);
    composer.set_text_content("foo bar".to_owned(), Vec::new(), Vec::new());
    commands(&mut composer, "0y$");
    composer.set_text_content("_".to_owned(), Vec::new(), Vec::new());
    commands(&mut composer, "0i");
    for ch in ['y', 'w'] {
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::CONTROL));
    }
    key(&mut composer, KeyCode::Esc);
    assert_eq!(composer.current_text(), "foo _");
    commands(&mut composer, "0.");
    assert_eq!(composer.current_text(), "foo foo _");
    commands(&mut composer, "p");
    assert!(composer.current_text().contains("bar"));
}

#[test]
fn plain_delete_replaces_rich_register() {
    let mut composer = rich_composer();
    commands(&mut composer, "y$");
    composer.set_text_content("abc".to_owned(), Vec::new(), Vec::new());
    commands(&mut composer, "0xp");
    assert_eq!(composer.current_text(), "bac");
    assert_eq!(composer.draft.textarea.text_elements(), Vec::new());
    assert_eq!(composer.local_image_paths(), Vec::<PathBuf>::new());
    assert_eq!(composer.mention_bindings(), Vec::new());
}

#[test]
fn rich_paste_renders_unique_image_and_paste_labels() {
    snapshot_composer_state_with_width(
        "vim_rich_register",
        /*width*/ 100,
        /*enhanced_keys_supported*/ false,
        |composer| {
            let mut previous = rich_composer();
            commands(&mut previous, "y$");
            *composer = rich_composer();
            composer.restore_kill_buffer_snapshot(previous.take_kill_buffer_snapshot());
            commands(composer, "p");
        },
    );
}

#[test]
fn replace_yank_inserts_rich_objects_without_stale_character_recovery() {
    for existing_element in [false, true] {
        let mut composer = rich_composer();
        commands(&mut composer, "y$");
        composer.set_text_content("abcdefghij".to_owned(), Vec::new(), Vec::new());
        if existing_element {
            composer.draft.textarea.set_cursor(/*pos*/ 0);
            composer.attach_image(PathBuf::from("/original/other.png"));
        }
        commands(&mut composer, "0R");
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL));
        assert_eq!(
            composer.mention_bindings()[0].path,
            "/original/skill/SKILL.md"
        );
        assert_eq!(
            composer.local_image_paths().len(),
            1 + usize::from(existing_element)
        );
        key(&mut composer, KeyCode::Backspace);
        assert_eq!(
            composer.draft.pending_pastes,
            Vec::<(String, String)>::new()
        );
        assert!(composer.current_text().ends_with("abcdefghij"));
        assert_eq!(composer.mention_bindings().len(), 1);
    }
}

#[test]
fn equal_length_plain_replace_yank_records_dot() {
    let (mut composer, _) = new_test_composer();
    composer.set_disable_paste_burst(/*disabled*/ true);
    composer.set_vim_enabled(/*enabled*/ true);
    composer.set_text_content("xyz".to_owned(), Vec::new(), Vec::new());
    commands(&mut composer, "0y$");
    composer.set_text_content("abcdefghi".to_owned(), Vec::new(), Vec::new());
    commands(&mut composer, "0R");
    let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL));
    key(&mut composer, KeyCode::Esc);
    commands(&mut composer, "l.");
    assert_eq!(composer.current_text(), "xyzxyzghi");
}

#[test]
fn empty_insert_and_replace_yanks_do_not_replay_a_later_register() {
    for mode in ["i", "R"] {
        let (mut composer, _) = new_test_composer();
        composer.set_disable_paste_burst(/*disabled*/ true);
        composer.set_vim_enabled(/*enabled*/ true);
        commands(&mut composer, mode);
        let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL));
        commands(&mut composer, "x");
        key(&mut composer, KeyCode::Esc);
        commands(&mut composer, "0yl.");
        assert_eq!(
            composer.current_text(),
            if mode == "i" { "xx" } else { "x" }
        );
    }
}

#[test]
fn linewise_rich_paste_preserves_the_cursor_on_the_leading_element() {
    let mut states = Vec::new();
    for image in [false, true] {
        let (mut composer, _) = new_test_composer();
        composer.set_vim_enabled(/*enabled*/ true);
        if image {
            composer.attach_image(PathBuf::from("/original/photo.png"));
        } else {
            composer.handle_paste("z".repeat(LARGE_PASTE_CHAR_THRESHOLD + 1));
        }
        key(&mut composer, KeyCode::Esc);
        commands(&mut composer, "0yyp");
        let mut marked = composer.current_text().to_owned();
        marked.insert(composer.draft.textarea.cursor(), '|');
        states.push(marked);
    }
    insta::assert_snapshot!(states.join("\n\n"), @r###"
    [Pasted Content 1001 chars]
    |[Pasted Content 1001 chars] #2

    [Image #1]
    |[Image #2]
    "###);
}

fn select(composer: &mut ChatComposer, start: u16, end: u16) {
    use crossterm::event::MouseButton;
    use crossterm::event::MouseEvent;
    use crossterm::event::MouseEventKind;
    use ratatui::widgets::StatefulWidgetRef;
    let area = ratatui::layout::Rect::new(0, 0, 100, 4);
    let mut state = super::super::super::textarea::TextAreaState::default();
    let mut buffer = ratatui::buffer::Buffer::empty(area);
    (&composer.draft.textarea).render_ref(area, &mut buffer, &mut state);
    for (kind, column) in [
        (MouseEventKind::Down(MouseButton::Left), start),
        (MouseEventKind::Drag(MouseButton::Left), end),
        (MouseEventKind::Up(MouseButton::Left), end),
    ] {
        composer.draft.textarea.handle_mouse(
            MouseEvent {
                kind,
                column,
                row: 0,
                modifiers: KeyModifiers::NONE,
            },
            state,
        );
    }
    assert_eq!(
        composer.draft.textarea.mouse_selection_range(),
        Some(usize::from(start)..usize::from(end))
    );
}

#[test]
fn yank_replaces_selection_without_recording_pointer_edits() {
    for rich in [false, true] {
        for mode in ["nonvim", "i", "R"] {
            let mut composer = rich_composer();
            if !rich {
                composer.set_text_content("xyz".to_owned(), Vec::new(), Vec::new());
            }
            commands(&mut composer, "0y$");
            let copied = composer.current_text_with_pending();
            composer.set_text_content("abcdef".to_owned(), Vec::new(), Vec::new());
            if mode == "nonvim" {
                composer.set_vim_enabled(/*enabled*/ false);
            } else {
                commands(&mut composer, mode);
            }
            select(&mut composer, /*start*/ 1, /*end*/ 3);
            let _ =
                composer.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL));
            assert_eq!(
                composer.current_text_with_pending(),
                format!("a{copied}def")
            );
            assert_eq!(composer.mention_bindings().len(), usize::from(rich));
            assert_eq!(composer.local_image_paths().len(), usize::from(rich));
            if mode != "nonvim" {
                key(&mut composer, KeyCode::Esc);
                assert!(composer.draft.textarea.vim_repeat_actions().is_none());
            }
        }
    }
}

#[test]
fn normal_dot_clears_pointer_selection_before_each_replay() {
    let mut composer = rich_composer();
    composer.set_text_content("abcdef".to_owned(), Vec::new(), Vec::new());
    commands(&mut composer, "0iX");
    key(&mut composer, KeyCode::Esc);
    select(&mut composer, /*start*/ 1, /*end*/ 3);
    commands(&mut composer, ".");
    assert_eq!(composer.current_text(), "XabXcdef");
    commands(&mut composer, ".");
    assert_eq!(composer.current_text(), "XabXXcdef");
}

#[test]
fn plain_replace_yank_preserves_previous_backspace_recovery() {
    let mut composer = rich_composer();
    composer.set_text_content("xyz".to_owned(), Vec::new(), Vec::new());
    commands(&mut composer, "0y$");
    composer.set_text_content("abcdefghi".to_owned(), Vec::new(), Vec::new());
    commands(&mut composer, "0RQ");
    let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL));
    assert_eq!(composer.current_text(), "Qxyzefghi");
    for _ in 0..4 {
        key(&mut composer, KeyCode::Backspace);
    }
    assert_eq!(composer.current_text(), "abcdefghi");
}

#[test]
fn internal_snapshot_restores_exact_mention_identity() {
    let (mut composer, _) = new_test_composer();
    composer.set_vim_enabled(/*enabled*/ true);
    for path in [None, Some("plugin://first"), Some("plugin://second")] {
        let id = composer.draft.textarea.insert_element("@plugin");
        if let Some(path) = path {
            composer.draft.mention_bindings.insert(
                id,
                super::super::ComposerMentionBinding {
                    sigil: '@',
                    mention: "plugin".into(),
                    path: path.into(),
                },
            );
        }
    }
    key(&mut composer, KeyCode::Esc);
    commands(&mut composer, "0lyl");
    let original = composer.snapshot_draft();
    commands(&mut composer, "p");
    let pasted = composer.snapshot_draft();
    assert_eq!(
        pasted.mention_bindings,
        vec![
            original.mention_bindings[0].clone(),
            original.mention_bindings[0].clone(),
            original.mention_bindings[1].clone(),
        ]
    );
    commands(&mut composer, "u");
    let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
    assert_eq!(composer.snapshot_draft(), pasted);
    commands(&mut composer, "ui");
    key(&mut composer, KeyCode::Delete);
    key(&mut composer, KeyCode::Home);
    let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL));
    key(&mut composer, KeyCode::Esc);
    commands(&mut composer, "u");
    assert_eq!(composer.snapshot_draft(), original);
}
