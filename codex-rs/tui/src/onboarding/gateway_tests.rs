#![cfg(test)]

use super::*;
use crate::app::provider::rpc::list_providers;
use crate::app::provider::rpc::write_provider;
use codex_app_server_protocol::CustomGatewaySettingsParams;
use codex_app_server_protocol::ProviderSettingsPatch;
use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use serde_json::json;

fn screen(widget: &AuthModeWidget, height: u16) -> String {
    let area = Rect::new(0, 0, 72, height);
    let mut terminal = crate::custom_terminal::Terminal::with_options(
        crate::test_backend::VT100Backend::new(area.width, area.height),
    )
    .unwrap();
    terminal.set_viewport_area(area);
    terminal
        .draw(|frame| widget.render_ref(area, frame.buffer_mut()))
        .unwrap();
    terminal
        .backend()
        .to_string()
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn select_a_saved_gateway_during_sign_in() -> color_eyre::Result<()> {
    let (mut widget, home) = super::tests::widget_forced_chatgpt().await;
    widget.auth_config.forced_login_method = None;
    widget.bedrock_setup_enabled = true;
    widget.gcp_setup_enabled = true;
    widget.gateway_setup_enabled = true;

    widget.handle_key_event(KeyCode::Char('3').into());
    insta::assert_snapshot!("gateway_sign_in_menu", screen(&widget, /*height*/ 8));
    widget.handle_key_event(KeyCode::Char('4').into());
    widget.advance_gateway_request().await;
    insta::assert_snapshot!("gateway_empty_picker", screen(&widget, /*height*/ 8));
    widget.handle_key_event(KeyCode::Esc.into());

    let managed = home.path().join("managed_config.toml");
    std::fs::write(&managed, "model_provider = 'openai'")?;
    widget.handle_key_event(KeyCode::Char('4').into());
    widget.advance_gateway_request().await;
    let error = screen(&widget, /*height*/ 8);
    assert!(error.contains("configuration policy"));
    insta::assert_snapshot!("gateway_load_error", error);
    widget.handle_key_event(KeyCode::Esc.into());
    std::fs::remove_file(managed)?;
    std::fs::write(
        home.path().join("config.toml"),
        r#"
[model_providers.office]
name = "Office"
base_url = "https://office.example.test/v1"
wire_api = "responses"
"#,
    )?;
    widget.handle_key_event(KeyCode::Char('4').into());
    widget.advance_gateway_request().await;
    insta::assert_snapshot!("gateway_saved_picker", screen(&widget, /*height*/ 8));
    widget.handle_key_event(KeyCode::Enter.into());
    widget.advance_gateway_request().await;
    assert!(matches!(widget.get_step_state(), StepState::Complete));
    insta::assert_snapshot!(screen(&widget, /*height*/ 4), @"✓ Custom provider configured");
    let saved: toml::Value =
        toml::from_str(&std::fs::read_to_string(home.path().join("config.toml"))?)?;
    assert_eq!(saved["model_provider"].as_str(), Some("office"));
    Ok(())
}

#[tokio::test]
async fn create_highlights_the_saved_gateway_until_explicitly_selected() -> color_eyre::Result<()> {
    let (mut widget, home) = super::tests::widget_forced_chatgpt().await;
    widget.auth_config.forced_login_method = None;
    widget.bedrock_setup_enabled = true;
    widget.gcp_setup_enabled = true;
    widget.gateway_setup_enabled = true;
    write_provider(
        &widget.app_server_request_handle,
        codex_app_server_protocol::ModelProviderConfigWriteParams {
            settings: ProviderSettingsPatch::CustomGateway(CustomGatewaySettingsParams {
                name: Some("Office".into()),
                base_url: Some(Some("https://office.example.test/v1".into())),
                ..Default::default()
            }),
        },
    )
    .await
    .map_err(color_eyre::Report::msg)?;
    widget.handle_key_event(KeyCode::Char('3').into());
    widget.handle_key_event(KeyCode::Char('4').into());
    widget.advance_gateway_request().await;
    widget.handle_key_event(KeyCode::Down.into());
    widget.handle_key_event(KeyCode::Enter.into());
    insta::assert_snapshot!(screen(&widget, /*height*/ 12), @r"
    Create a custom provider or gateway

    › Name*  Team gateway
      Base URL*  https://gateway.example/v1
      API key variable  $TEAM_API_KEY
      Save

    * Required
    Enter the environment variable name on the app server.
    tab move · enter next/save · esc back
    ");
    widget.handle_paste("Team gateway".into());
    widget.handle_key_event(KeyCode::Down.into());
    widget.handle_paste("https://team.example.test/v1?".into());
    widget.handle_key_event(KeyCode::Down.into());
    widget.handle_paste("TEAM_KEY-".into());
    widget.handle_key_event(KeyCode::Down.into());
    insta::assert_snapshot!("gateway_save_focused_short", screen(&widget, /*height*/ 4));
    widget.handle_key_event(KeyCode::Enter.into());
    insta::assert_snapshot!("gateway_save_error_short", screen(&widget, /*height*/ 4));
    widget.handle_key_event(KeyCode::Up.into());
    widget.handle_key_event(KeyCode::Backspace.into());
    widget.handle_key_event(KeyCode::Down.into());
    widget.handle_key_event(KeyCode::Enter.into());
    widget.advance_gateway_request().await;
    insta::assert_snapshot!("gateway_validation_retry", screen(&widget, /*height*/ 8));

    // Correct the invalid URL and keep the other fields.
    widget.handle_key_event(KeyCode::Up.into());
    widget.handle_key_event(KeyCode::Up.into());
    widget.handle_key_event(KeyCode::Backspace.into());
    widget.handle_key_event(KeyCode::Down.into());
    widget.handle_key_event(KeyCode::Down.into());
    widget.handle_key_event(KeyCode::Enter.into());
    widget.advance_gateway_request().await;
    assert_eq!(widget.error_message(), None);
    widget.handle_key_event(KeyEvent::new_with_kind(
        KeyCode::Enter,
        KeyModifiers::NONE,
        KeyEventKind::Repeat,
    ));
    assert!(matches!(widget.get_step_state(), StepState::InProgress));
    insta::assert_snapshot!("gateway_created_picker", screen(&widget, /*height*/ 8));
    let page = list_providers(&widget.app_server_request_handle)
        .await
        .map_err(color_eyre::Report::msg)?;
    assert_eq!(page.default_provider_id, "openai");

    widget.handle_key_event(KeyEvent::new_with_kind(
        KeyCode::Enter,
        KeyModifiers::NONE,
        KeyEventKind::Release,
    ));
    widget.handle_key_event(KeyCode::Enter.into());
    widget.advance_gateway_request().await;
    assert!(matches!(widget.get_step_state(), StepState::Complete));
    let saved: serde_json::Value =
        toml::from_str(&std::fs::read_to_string(home.path().join("config.toml"))?)?;
    assert_eq!(
        saved,
        json!({
            "model_provider": "team-gateway",
            "model_providers": {
                "office": {
                    "name": "Office",
                    "base_url": "https://office.example.test/v1",
                    "wire_api": "responses",
                },
                "team-gateway": {
                    "name": "Team gateway",
                    "base_url": "https://team.example.test/v1",
                    "env_key": "TEAM_KEY",
                    "wire_api": "responses",
                },
            },
        })
    );
    Ok(())
}

#[tokio::test]
async fn gateway_oauth_displays_the_browser_link_and_retry_action() {
    let (widget, _home) = super::tests::widget_forced_chatgpt().await;
    *widget.sign_in_state.write().unwrap() = SignInState::GatewayOAuth {
        provider_id: "office".into(),
        url: None,
    };
    insta::assert_snapshot!(screen(&widget, /*height*/ 1), @"Starting gateway sign-in…");
    *widget.sign_in_state.write().unwrap() = SignInState::GatewayOAuth {
        provider_id: "office".into(),
        url: Some("https://office.example.test/authorize".into()),
    };
    insta::assert_snapshot!("gateway_oauth_browser", screen(&widget, /*height*/ 4));
    *widget.sign_in_state.write().unwrap() = SignInState::GatewayOAuthFailed {
        provider_id: "office".into(),
        message: "Gateway sign-in expired.".into(),
    };
    insta::assert_snapshot!("gateway_oauth_failed", screen(&widget, /*height*/ 4));
}
