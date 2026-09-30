//! Enter must expire paste detection even when no UI flush tick has run.

use std::time::Duration;
use std::time::Instant;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;

use super::InputResult;
use super::PasteBurst;
use super::tests::new_test_composer;
use super::tests::snapshot_composer_state_with_width;

#[test]
fn enter_expires_paste_state_without_a_ui_tick() {
    for vim_enabled in [false, true] {
        for parent_owned in [false, true] {
            for text in ["x", "hi"] {
                let (mut composer, _rx) = new_test_composer();
                composer.set_vim_enabled(vim_enabled);
                composer.draft.textarea.enter_vim_insert_mode();
                if parent_owned {
                    composer.set_parent_owned_thread();
                }
                let now = Instant::now();
                for ch in text.chars() {
                    composer.handle_input_basic_with_time(
                        KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
                        now,
                    );
                }
                assert!(composer.is_in_paste_burst());
                assert_eq!(composer.current_text(), "");

                let result = composer.handle_submission_with_time(
                    /*should_queue*/ false,
                    now + PasteBurst::recommended_active_flush_delay(),
                );
                let expected = if parent_owned {
                    InputResult::ParentOwnedInputBlocked
                } else {
                    InputResult::Submitted {
                        text: text.to_owned(),
                        text_elements: Vec::new(),
                    }
                };
                assert_eq!(result, (expected, true));
                assert_eq!(
                    composer.current_text(),
                    if parent_owned { text } else { "" }
                );
                assert!(!composer.is_in_paste_burst());
            }
        }
    }
}

#[test]
fn plain_enter_submits_expired_vim_insert_input() {
    snapshot_composer_state_with_width(
        "vim_enter_after_expired_paste",
        /*width*/ 60,
        /*enhanced_keys_supported*/ true,
        |composer| {
            composer.set_vim_enabled(/*enabled*/ true);
            composer.draft.textarea.enter_vim_insert_mode();
            let typed_at = Instant::now() - Duration::from_secs(1);
            composer.handle_input_basic_with_time(
                KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
                typed_at,
            );
            assert_eq!(
                composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
                (
                    InputResult::Submitted {
                        text: "x".to_owned(),
                        text_elements: Vec::new(),
                    },
                    true,
                )
            );
            assert_eq!(composer.current_text(), "");
        },
    );
}
