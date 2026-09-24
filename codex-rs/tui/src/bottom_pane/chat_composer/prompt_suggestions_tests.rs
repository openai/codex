//! Exercise suggestion parsing and composer input behavior.

use super::super::tests::new_test_composer;
use super::super::tests::snapshot_composer_state_with_width;
use super::*;
use codex_protocol::ThreadId;
use pretty_assertions::assert_eq;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

fn request() -> SuggestionRequest {
    SuggestionRequest {
        thread_id: ThreadId::new(),
        turn_id: "turn".into(),
        summary: codex_protocol::config_types::ReasoningSummary::None,
        id: Uuid::new_v4(),
        cancellation: CancellationToken::new(),
        generation_finished: CancellationToken::new(),
    }
}

#[test]
fn prompt_suggestion_tab_edits_and_enter_submits() {
    let (mut composer, _rx) = new_test_composer();
    composer.set_vim_enabled(/*enabled*/ true);
    composer.handle_key_event(KeyCode::Esc.into());
    let request = request();
    composer.set_prompt_suggestion(request.clone());
    composer.apply_prompt_suggestion(&request, Some("Add a regression test".into()));
    assert!(composer.is_empty());
    let (result, _) = composer.handle_key_event(KeyCode::Tab.into());
    assert!(matches!(result, InputResult::None));
    assert_eq!(composer.draft.textarea.text(), "Add a regression test");
    assert!(request.cancellation.is_cancelled());
    for kind in [KeyEventKind::Press, KeyEventKind::Repeat] {
        let (result, _) = composer.handle_key_event(KeyEvent::new_with_kind(
            KeyCode::Tab,
            KeyModifiers::NONE,
            kind,
        ));
        assert!(matches!(result, InputResult::None));
        assert_eq!(composer.draft.textarea.text(), "Add a regression test");
    }
    composer.handle_key_event(KeyCode::Char('u').into());
    assert!(composer.is_empty());
    composer.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
    assert_eq!(composer.draft.textarea.text(), "Add a regression test");
    let (result, _) = composer.handle_key_event(KeyCode::Enter.into());
    assert!(
        matches!(result, InputResult::Submitted { text, .. } if text == "Add a regression test")
    );
}

#[test]
fn prompt_suggestion_dismissal_and_stale_result() {
    let (mut composer, _rx) = new_test_composer();
    let old = request();
    composer.set_prompt_suggestion(old.clone());
    composer.handle_key_event(KeyCode::Esc.into());
    assert!(old.cancellation.is_cancelled());
    composer.apply_prompt_suggestion(&old, Some("Late".into()));
    assert_eq!(composer.visible_prompt_suggestion(), None);
    let next = request();
    composer.set_prompt_suggestion(next.clone());
    composer.apply_prompt_suggestion(&next, Some("Next".into()));
    composer.apply_prompt_suggestion(&old, Some("Stale".into()));
    assert_eq!(composer.visible_prompt_suggestion(), Some("Next"));
    composer.set_task_running(/*running*/ true);
    assert!(next.cancellation.is_cancelled());
    composer.set_prompt_suggestion(request());
    let empty = composer.prompt_suggestion.as_ref().unwrap().request.clone();
    composer.apply_prompt_suggestion(&empty, /*text*/ None);
    assert!(!composer.has_prompt_suggestion());
}

#[test]
fn prompt_suggestion_hides_for_typing_and_input_owners() {
    let (mut composer, _rx) = new_test_composer();
    let request = request();
    composer.set_prompt_suggestion(request.clone());
    composer.handle_key_event(KeyCode::Char('a').into());
    composer.apply_prompt_suggestion(&request, Some("Continue".into()));
    assert_eq!(composer.visible_prompt_suggestion(), None);
    composer.handle_key_event(KeyCode::Tab.into());
    assert!(!composer.draft.textarea.text().contains("Continue"));
    composer.draft.paste_burst.clear_after_explicit_paste();
    composer.draft.textarea.set_text_clearing_elements("");
    assert_eq!(composer.visible_prompt_suggestion(), Some("Continue"));
    composer
        .attachments
        .remote_image_urls
        .push("https://example.test/image.png".into());
    assert_eq!(composer.visible_prompt_suggestion(), None);
}

#[test]
fn prompt_suggestion_snapshots() {
    snapshot_composer_state_with_width(
        "prompt_suggestion_24",
        /*width*/ 24,
        /*enhanced_keys_supported*/ false,
        |composer| {
            let request = request();
            composer.set_prompt_suggestion(request.clone());
            composer.apply_prompt_suggestion(
                &request,
                Some("Add a regression test for the timeout".into()),
            );
        },
    );
}

#[test]
fn prompt_suggestion_yields_to_remapped_tab_and_popup() {
    let (mut composer, _rx) = new_test_composer();
    let request = request();
    composer.set_prompt_suggestion(request.clone());
    composer.apply_prompt_suggestion(&request, Some("Continue".into()));
    let mut config = codex_config::types::TuiKeymap::default();
    config.composer.submit = Some(codex_config::types::KeybindingsSpec::One(
        codex_config::types::KeybindingSpec("tab".into()),
    ));
    config.composer.queue = Some(codex_config::types::KeybindingsSpec::Many(vec![]));
    let keymap = crate::keymap::RuntimeKeymap::from_config(&config).expect("keymap");
    composer.set_keymap_bindings(&keymap);
    composer.handle_key_event(KeyCode::Tab.into());
    assert!(composer.is_empty());
    assert!(!request.cancellation.is_cancelled());
    composer.handle_paste("/".into());
    assert!(composer.popup_active());
    composer.handle_key_event(KeyCode::Esc.into());
    assert!(!request.cancellation.is_cancelled());
}

#[test]
fn prompt_suggestion_tab_only_yields_to_active_vim_bindings() {
    let (mut composer, _rx) = new_test_composer();
    composer.set_vim_enabled(/*enabled*/ true);
    composer.handle_key_event(KeyCode::Esc.into());
    let request = request();
    composer.set_prompt_suggestion(request.clone());
    composer.apply_prompt_suggestion(&request, Some("Continue".into()));
    let mut keymap = crate::keymap::RuntimeKeymap::defaults();
    keymap.vim_normal.undo = vec![crate::key_hint::plain(KeyCode::Tab)];
    composer.set_keymap_bindings(&keymap);
    composer.handle_key_event(KeyCode::Tab.into());
    assert!(composer.is_empty());
    composer.handle_key_event(KeyCode::Char('i').into());
    composer.handle_key_event(KeyCode::Tab.into());
    assert_eq!(composer.current_text(), "Continue");
}
