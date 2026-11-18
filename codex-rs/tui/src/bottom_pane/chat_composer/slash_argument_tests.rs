#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

#[test]
fn slash_argument_completion_and_dismissal() {
    let (mut composer, _rx) = super::super::tests::new_test_composer();
    composer.set_disable_paste_burst(/*disabled*/ true);
    composer.draft.textarea.set_text_clearing_elements("/raw");
    composer.draft.textarea.set_cursor(/*pos*/ 4);
    composer.sync_popups();
    composer.handle_key_event(KeyCode::Tab.into());
    assert_eq!(composer.draft.textarea.text(), "/raw ");
    assert!(matches!(composer.popups.active, ActivePopup::Argument(_)));

    let mut terminal = Terminal::new(TestBackend::new(/*width*/ 8, /*height*/ 2)).unwrap();
    terminal
        .draw(|frame| {
            let ActivePopup::Argument(popup) = &composer.popups.active else {
                unreachable!()
            };
            popup.render_ref(frame.area(), frame.buffer_mut());
        })
        .unwrap();
    insta::assert_snapshot!("slash_argument_popup", terminal.backend());

    composer.handle_key_event(KeyCode::Down.into());
    composer.handle_key_event(KeyCode::Tab.into());
    assert_eq!(composer.draft.textarea.text(), "/raw off");
    assert!(!composer.popup_active());

    composer.draft.textarea.set_text_clearing_elements("/raw o");
    composer.draft.textarea.set_cursor(/*pos*/ 6);
    composer.sync_popups();
    composer.handle_key_event(KeyCode::Esc.into());
    composer.sync_popups();
    assert!(!composer.popup_active());
    composer.handle_key_event(KeyCode::Char('n').into());
    assert!(matches!(composer.popups.active, ActivePopup::Argument(_)));
}

#[test]
fn slash_argument_completion_preserves_tail_and_enter_submits_literal() {
    let (mut composer, _rx) = super::super::tests::new_test_composer();
    composer
        .draft
        .textarea
        .set_text_clearing_elements("/mcp loX server\nmore");
    composer.draft.textarea.set_cursor(/*pos*/ 7);
    composer.sync_popups();
    composer.handle_key_event(KeyCode::Tab.into());
    assert_eq!(composer.draft.textarea.text(), "/mcp login server\nmore");

    composer.draft.textarea.set_text_clearing_elements("/raw o");
    composer.draft.textarea.set_cursor(/*pos*/ 6);
    composer.sync_popups();
    assert_eq!(
        composer.handle_key_event(KeyCode::Enter.into()).0,
        InputResult::CommandWithArgs(SlashCommand::Raw, "o".to_owned(), Vec::new())
    );
}

#[test]
fn slash_argument_popup_gives_modified_down_to_submit_binding() {
    let (mut composer, _rx) = super::super::tests::new_test_composer();
    let submit = KeyEvent::new(KeyCode::Down, KeyModifiers::CONTROL | KeyModifiers::ALT);
    composer.submit_keys = vec![crate::key_hint::KeyBinding::from_event(submit)];
    composer
        .draft
        .textarea
        .set_text_clearing_elements("/raw off");
    composer.draft.textarea.set_cursor(/*pos*/ 8);
    composer.sync_popups();

    assert!(matches!(composer.popups.active, ActivePopup::Argument(_)));
    assert_eq!(
        composer.handle_key_event(submit).0,
        InputResult::CommandWithArgs(SlashCommand::Raw, "off".to_owned(), Vec::new())
    );
}

#[test]
fn slash_argument_respects_catalog_and_unavailable_thread() {
    let (mut composer, _rx) = super::super::tests::new_test_composer();
    composer
        .draft
        .textarea
        .set_text_clearing_elements("/usage ");
    composer.draft.textarea.set_cursor(/*pos*/ 7);
    composer.sync_popups();
    assert!(!composer.popup_active());

    composer.set_side_conversation_active(/*active*/ true);
    composer.draft.textarea.set_text_clearing_elements("/mcp ");
    composer.draft.textarea.set_cursor(/*pos*/ 5);
    composer.sync_popups();
    assert!(!composer.popup_active());

    composer.set_side_conversation_active(/*active*/ false);
    composer.draft.textarea.set_text_clearing_elements("/raw");
    composer.draft.textarea.set_cursor(/*pos*/ 4);
    composer.sync_popups();
    let mode = super::super::reconnect::RestrictedInputMode::UnavailableThread;
    composer.handle_restricted_key(KeyCode::Tab.into(), mode);
    assert!(matches!(composer.popups.active, ActivePopup::Argument(_)));
    composer.sync_popups();
    composer.handle_restricted_key(KeyCode::Tab.into(), mode);
    assert_eq!(composer.draft.textarea.text(), "/raw on");
}

#[test]
fn slash_argument_acceptance_revalidates_the_draft() {
    let (mut composer, _rx) = super::super::tests::new_test_composer();
    composer.set_disable_paste_burst(/*disabled*/ true);
    composer.draft.textarea.set_text_clearing_elements("/raw ");
    composer.draft.textarea.set_cursor(/*pos*/ 5);
    composer.sync_popups();

    // A different command can have the same still-valid replacement range.
    composer.draft.textarea.set_text_clearing_elements("/ide ");
    composer.draft.textarea.set_cursor(/*pos*/ 5);
    composer.handle_slash_argument_key(KeyCode::Tab.into());
    assert_eq!(composer.draft.textarea.text(), "/ide ");

    composer.draft.textarea.set_text_clearing_elements("/raw ");
    composer.draft.textarea.set_cursor(/*pos*/ 5);
    composer.sync_popups();
    let mode = super::super::reconnect::RestrictedInputMode::UnavailableThread;
    assert_eq!(
        composer.handle_restricted_key(KeyCode::Enter.into(), mode),
        InputResult::Command(SlashCommand::Raw)
    );
    composer.handle_restricted_key(KeyCode::Tab.into(), mode);
    assert_eq!(composer.draft.textarea.text(), "");
    assert!(!composer.popup_active());
}

#[test]
fn slash_argument_revalidates_restricted_mode_before_acceptance_and_redraw() {
    let (mut composer, _rx) = super::super::tests::new_test_composer();
    composer.draft.textarea.set_text_clearing_elements("/mcp ");
    composer.draft.textarea.set_cursor(/*pos*/ 5);
    composer.sync_popups();
    assert!(matches!(composer.popups.active, ActivePopup::Argument(_)));

    let mode = RestrictedInputMode::UnavailableThread;
    composer.handle_restricted_key(KeyCode::Tab.into(), mode);
    assert_eq!(composer.draft.textarea.text(), "/mcp ");
    composer.sync_popups();
    assert!(!composer.popup_active());
    composer.handle_restricted_key(KeyCode::Tab.into(), mode);
    assert_eq!(composer.draft.textarea.text(), "/mcp ");
}

#[test]
fn slash_argument_respects_parent_owned_policy_before_acceptance() {
    let (mut composer, _rx) = super::super::tests::new_test_composer();
    composer.draft.textarea.set_text_clearing_elements("/raw ");
    composer.draft.textarea.set_cursor(/*pos*/ 5);
    composer.sync_popups();
    assert!(matches!(composer.popups.active, ActivePopup::Argument(_)));

    composer.set_parent_owned_thread();
    composer.sync_popups();
    assert!(!composer.popup_active());
    // With no suggestion to accept, Tab retains ordinary bare-command submission.
    assert_eq!(
        composer.handle_key_event(KeyCode::Tab.into()).0,
        InputResult::Command(SlashCommand::Raw)
    );
    assert_eq!(composer.draft.textarea.text(), "");
}

#[test]
fn slash_argument_restricted_completion_retains_single_line_recovery_semantics() {
    let (mut composer, _rx) = super::super::tests::new_test_composer();
    composer
        .draft
        .textarea
        .set_text_clearing_elements("/raw o\nmore");
    composer.draft.textarea.set_cursor(/*pos*/ 6);
    composer.sync_popups();
    composer.handle_restricted_key(KeyCode::Tab.into(), RestrictedInputMode::UnavailableThread);
    composer.sync_popups();
    assert!(!composer.popup_active());
    assert_eq!(composer.draft.textarea.text(), "/raw o\nmore");
}

#[test]
fn parent_owned_unavailable_thread_retains_argument_recovery() {
    let (mut composer, _rx) = super::super::tests::new_test_composer();
    composer.set_parent_owned_thread();
    composer.draft.textarea.set_text_clearing_elements("/raw ");
    composer.draft.textarea.set_cursor(/*pos*/ 5);
    composer.sync_popups();
    assert!(!composer.popup_active());

    let mode = RestrictedInputMode::UnavailableThread;
    composer.handle_restricted_key(KeyCode::Null.into(), mode);
    composer.sync_popups();
    assert!(matches!(composer.popups.active, ActivePopup::Argument(_)));
    composer.handle_restricted_key(KeyCode::Tab.into(), mode);
    assert_eq!(composer.draft.textarea.text(), "/raw on");
    assert_eq!(
        composer.handle_restricted_key(KeyCode::Enter.into(), mode),
        InputResult::CommandWithArgs(SlashCommand::Raw, "on".to_owned(), Vec::new())
    );
}

#[test]
fn slash_tab_completion_moves_cursor_to_end() {
    let (mut composer, _rx) = super::super::tests::new_test_composer();
    super::super::tests::type_chars_humanlike(&mut composer, &['/', 'c']);

    composer.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(composer.draft.textarea.text(), "/compact ");
    assert_eq!(
        composer.draft.textarea.cursor(),
        composer.draft.textarea.text().len()
    );
}
