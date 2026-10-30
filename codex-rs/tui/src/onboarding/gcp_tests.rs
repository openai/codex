#![cfg(test)]

//! Exercises Google Cloud onboarding through the widget and embedded app-server.

use super::*;
use codex_app_server_protocol::ConfigBatchWriteParams;
use codex_app_server_protocol::ConfigEdit;
use codex_app_server_protocol::ConfigWriteResponse;
use codex_app_server_protocol::MergeStrategy;
use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test]
async fn gcp_menu_gating_and_setup_flow() {
    let (mut widget, home) = super::tests::widget_forced_chatgpt().await;
    widget.auth_config.forced_login_method = None;
    widget.handle_sign_in_option(SignInOption::Gcp);
    assert!(matches!(
        &*widget.sign_in_state.read().unwrap(),
        SignInState::PickMode
    ));

    let snapshot = |widget: &AuthModeWidget, name: &str, area: Rect| {
        let mut terminal = crate::custom_terminal::Terminal::with_options(
            crate::test_backend::VT100Backend::new(area.width, area.height),
        )
        .unwrap();
        terminal.set_viewport_area(area);
        terminal
            .draw(|frame| widget.render_ref(area, frame.buffer_mut()))
            .unwrap();
        insta::assert_snapshot!(name, terminal.backend().to_string());
    };
    snapshot(
        &widget,
        "sign_in_menu_short_without_cloud_wizards",
        Rect::new(0, 0, 76, 8),
    );

    let press = |key| KeyEvent::new(key, KeyModifiers::NONE);
    widget.gcp_setup_enabled = true;
    widget.handle_key_event(press(KeyCode::Char('3')));
    assert_eq!(
        widget.displayed_sign_in_options(),
        vec![SignInOption::ApiKey, SignInOption::Gcp]
    );
    widget.bedrock_setup_enabled = true;
    widget.handle_key_event(KeyEvent::new_with_kind(
        KeyCode::Char('3'),
        KeyModifiers::NONE,
        KeyEventKind::Repeat,
    ));
    assert_eq!(widget.highlighted_mode, SignInOption::ApiKey);
    assert!(matches!(
        *widget.sign_in_state.read().unwrap(),
        SignInState::PickMode
    ));
    snapshot(&widget, "gcp_sign_in_menu", Rect::new(0, 0, 76, 21));
    widget.handle_key_event(press(KeyCode::Up));
    snapshot(&widget, "gcp_sign_in_menu_short", Rect::new(0, 0, 76, 8));
    widget.handle_key_event(press(KeyCode::Esc));
    assert_eq!(widget.highlighted_mode, SignInOption::OtherMethods);
    let area = Rect::new(0, 0, 76, 12);

    widget.auth_config.forced_login_method = Some(ForcedLoginMethod::Chatgpt);
    assert_eq!(
        widget.displayed_sign_in_options(),
        vec![SignInOption::ChatGpt, SignInOption::DeviceCode]
    );
    widget.handle_sign_in_option(SignInOption::Gcp);
    assert!(matches!(
        &*widget.sign_in_state.read().unwrap(),
        SignInState::PickMode
    ));
    widget.auth_config.forced_login_method = None;

    *widget.sign_in_state.write().unwrap() =
        SignInState::ChatGptDeviceCode(ContinueWithDeviceCodeState::pending("login".into()));
    widget.handle_key_event(press(KeyCode::Char('3')));
    assert!(matches!(
        &*widget.sign_in_state.read().unwrap(),
        SignInState::ChatGptDeviceCode(_)
    ));
    widget.cancel_active_attempt();

    widget.handle_key_event(press(KeyCode::Char('3')));
    widget.handle_key_event(press(KeyCode::Char('1')));
    widget.handle_key_event(press(KeyCode::Esc));
    assert_eq!(widget.highlighted_mode, SignInOption::ApiKey);
    widget.handle_key_event(press(KeyCode::Char('3')));
    widget.handle_key_event(press(KeyCode::Esc));
    assert_eq!(widget.highlighted_mode, SignInOption::Gcp);
    widget.handle_key_event(press(KeyCode::Enter));
    assert!(widget.is_text_entry_active());
    assert!(widget.should_suppress_printable_quit());
    widget.handle_key_event(KeyEvent::new_with_kind(
        KeyCode::Enter,
        KeyModifiers::NONE,
        KeyEventKind::Release,
    ));
    widget.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(
        widget.error_message().as_deref(),
        Some("Enter a Google Cloud project ID.")
    );
    widget.handle_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
    widget.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    widget.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::SUPER));
    widget.handle_key_event(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::SUPER));
    widget.handle_paste("test-ject\n".to_string());
    for _ in 0..4 {
        widget.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    }
    widget.handle_paste("pro".to_string());
    widget.handle_key_event(KeyEvent::new(
        KeyCode::Char('u'),
        KeyModifiers::SUPER | KeyModifiers::CONTROL,
    ));
    snapshot(&widget, "gcp_project_entry", area);

    widget.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    snapshot(&widget, "gcp_location_entry", area);
    // Location accepts text normally, and Esc preserves it for the next visit.
    widget.handle_paste("us-central1".to_string());
    widget.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    widget.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    widget.handle_key_event(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
    widget.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    snapshot(&widget, "gcp_credentials_entry", area);

    // A relative credential path fails server validation without requiring real GCP credentials.
    widget.handle_paste("relative-adc.json".to_string());
    let (request_frame, mut frames) = FrameRequester::test_channel();
    widget.request_frame = request_frame;
    widget.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(!widget.is_text_entry_active());
    widget.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    widget.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(!widget.is_text_entry_active());
    snapshot(&widget, "gcp_configuring", area);
    while !widget.is_text_entry_active() {
        frames.recv().await.unwrap();
    }
    let error = widget.error_message().unwrap();
    assert!(error.contains("absolute"), "{error}");
    assert!(!home.path().join("config.toml").exists());
    snapshot(&widget, "gcp_setup_error_narrow", Rect::new(0, 0, 40, 18));

    // Configure the existing command-auth boundary to avoid invoking gcloud in the test.
    let (command, args) = if cfg!(windows) {
        ("cmd", vec!["/C", "echo setup-token"])
    } else {
        ("echo", vec!["setup-token"])
    };
    widget
        .app_server_request_handle
        .request_typed::<ConfigWriteResponse>(ClientRequest::ConfigBatchWrite {
            request_id: onboarding_request_id(),
            params: ConfigBatchWriteParams {
                edits: vec![ConfigEdit {
                    key_path: "model_providers.gcp.auth".to_string(),
                    value: json!({"command": command, "args": args, "cwd": home.path()}),
                    merge_strategy: MergeStrategy::Replace,
                }],
                file_path: None,
                expected_version: None,
                reload_user_config: false,
            },
        })
        .await
        .unwrap();
    widget.handle_key_event(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
    widget.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    while matches!(&*widget.sign_in_state.read().unwrap(), SignInState::Gcp(_))
        && widget.error_message().is_none()
    {
        frames.recv().await.unwrap();
    }
    assert_eq!(widget.error_message(), None);
    assert!(matches!(widget.get_step_state(), StepState::Complete));
    snapshot(&widget, "gcp_configured", area);
    let config: toml::Value =
        toml::from_str(&std::fs::read_to_string(home.path().join("config.toml")).unwrap()).unwrap();
    assert_eq!(
        config,
        toml::Value::try_from(json!({
            "model_provider": "gcp",
            "model_providers": {"gcp": {
                "project_id": "test-project", "location": "global",
                "auth": {"command": command, "args": args, "cwd": home.path()},
            }},
        }))
        .unwrap()
    );
}
