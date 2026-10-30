#![cfg(test)]

//! Regression coverage for a key held across the sign-in menus.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn held_enter_requires_a_fresh_confirmation_to_save_a_prefilled_api_key() {
    let (mut widget, _home) = tests::widget_forced_chatgpt().await;
    widget.auth_config.forced_login_method = None;
    let enter = |kind| KeyEvent::new_with_kind(KeyCode::Enter, KeyModifiers::NONE, kind);

    widget.handle_key_event(KeyCode::Up.into());
    widget.handle_key_event(enter(KeyEventKind::Press));
    widget.handle_key_event(enter(KeyEventKind::Repeat));
    widget.handle_key_event(enter(KeyEventKind::Press));
    widget.handle_key_event(KeyCode::Down.into());
    widget.handle_key_event(enter(KeyEventKind::Repeat));
    assert_eq!(widget.highlighted_mode, SignInOption::ApiKey);
    assert!(matches!(
        *widget.sign_in_state.read().unwrap(),
        SignInState::PickMode
    ));

    widget.handle_key_event(enter(KeyEventKind::Release));
    widget.handle_key_event(enter(KeyEventKind::Press));
    {
        let mut state = widget.sign_in_state.write().unwrap();
        let SignInState::ApiKeyEntry(input) = &mut *state else {
            panic!("expected API key entry");
        };
        input.value = "test-api-key".into();
        input.prepopulated_from_env = true;
    }
    let (request_frame, mut frames) = FrameRequester::test_channel();
    widget.request_frame = request_frame;
    widget.handle_key_event(enter(KeyEventKind::Press));
    widget.handle_key_event(enter(KeyEventKind::Repeat));
    widget.handle_key_event(KeyCode::Left.into());
    widget.handle_key_event(enter(KeyEventKind::Repeat));
    assert_eq!(
        frames.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    );

    widget.handle_key_event(enter(KeyEventKind::Release));
    widget.handle_key_event(enter(KeyEventKind::Press));
    frames.try_recv().expect("fresh Enter should start saving");
    frames.recv().await.expect("API key save should finish");
    assert!(matches!(
        *widget.sign_in_state.read().unwrap(),
        SignInState::ApiKeyConfigured
    ));
}
