//! Fresh draft entry and composer-local Vim search remain separate input paths.

use super::super::InputResult;
use super::super::tests::snapshot_composer_state_with_width;
use crate::keymap::KeyChordMatch;
use crate::keymap::KeyChordMatcher;
use crate::keymap::RuntimeKeymap;
use codex_config::types::TuiKeymap;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use tokio::time::Instant;

#[test]
fn fresh_vim_draft_records_typing_after_submission() {
    snapshot_composer_state_with_width(
        "vim_fresh_draft",
        /*width*/ 40,
        /*enhanced_keys_supported*/ false,
        |composer| {
            composer.set_disable_paste_burst(/*disabled*/ true);
            for entry in [KeyCode::Char('i'), KeyCode::Esc] {
                composer.set_text_content("sent".into(), Vec::new(), Vec::new());
                composer.set_vim_enabled(/*enabled*/ true);
                composer.handle_key_event(entry.into());
                let (result, _) = composer.handle_key_event(KeyCode::Enter.into());
                assert!(matches!(result, InputResult::Submitted { .. }));
                assert_eq!(composer.draft.textarea.vim_mode_label(), Some("Insert"));
                for key in [KeyCode::Char('x'), KeyCode::Esc, KeyCode::Char('.')] {
                    composer.handle_key_event(key.into());
                }
                assert_eq!(composer.current_text(), "xx");
            }
            composer.handle_key_event(KeyCode::Enter.into());
        },
    );
}

#[test]
fn backspace_cancels_empty_vim_search() {
    snapshot_composer_state_with_width(
        "vim_search_cancelled",
        /*width*/ 40,
        /*enhanced_keys_supported*/ false,
        |composer| {
            for text in ["", "draft"] {
                composer.set_text_content(text.into(), Vec::new(), Vec::new());
                composer.set_vim_enabled(/*enabled*/ true);
                let cursor = composer.cursor();
                for prefix in ["/", "?", "d/", "d?", "c/", "c?", "y/", "y?"] {
                    for ch in prefix.chars() {
                        composer
                            .handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
                    }
                    assert!(composer.draft.textarea.vim_query().is_some());
                    composer.handle_key_event(KeyCode::Backspace.into());
                    assert!(!composer.draft.textarea.is_vim_operator_pending());
                    assert!(composer.draft.textarea.is_vim_normal_mode());
                    assert_eq!(
                        (composer.current_text(), composer.cursor()),
                        (text.into(), cursor)
                    );
                }
            }
        },
    );
}

#[test]
fn vim_search_query_edits_and_paste_preserve_the_draft() {
    snapshot_composer_state_with_width(
        "vim_composer_search",
        /*width*/ 40,
        /*enhanced_keys_supported*/ false,
        |composer| {
            composer.set_text_content("a g\u{301}g b हि gg".into(), Vec::new(), Vec::new());
            composer.set_vim_enabled(/*enabled*/ true);
            composer.draft.textarea.set_cursor(/*pos*/ 0);
            let config: TuiKeymap = toml::from_str("[vim_search]\nforward='g s'").unwrap();
            let keymap = RuntimeKeymap::from_config(&config).unwrap();
            composer.set_keymap_bindings(&keymap);
            let mut matcher = KeyChordMatcher::default();
            for ch in "dgse\u{301}\u{8}gg".chars() {
                let code = if ch == '\u{8}' {
                    KeyCode::Backspace
                } else {
                    KeyCode::Char(ch)
                };
                let event = KeyEvent::new(code, KeyModifiers::NONE);
                let event = match matcher.advance(
                    event,
                    &keymap.chords,
                    composer.keymap_contexts(),
                    Instant::now(),
                ) {
                    KeyChordMatch::PassThrough => event,
                    KeyChordMatch::Completed(event) => event,
                    KeyChordMatch::Pending(_) => continue,
                    other => panic!("unexpected chord result: {other:?}"),
                };
                composer.handle_key_event(event);
            }
            composer.handle_paste("photo.png".into());
            let input = composer.draft.textarea.vim_query().unwrap();
            assert_eq!(input.editor.text(), "ggphoto.png");
            assert_eq!(composer.draft.textarea.text(), "a g\u{301}g b हि gg");
            assert!(composer.attachments.local_image_paths().is_empty());
            composer.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
            composer.handle_key_event(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
            composer.handle_paste("ह".into());
        },
    );
}
