#![cfg(test)]

use super::*;
use crossterm::event::KeyCode;
use pretty_assertions::assert_eq;

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
    insta::assert_snapshot!("gateway_load_error", screen(&widget, /*height*/ 8));
    widget.handle_key_event(KeyCode::Esc.into());

    widget.handle_key_event(KeyEvent {
        kind: KeyEventKind::Release,
        ..KeyCode::Char('4').into()
    });
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
    let saved: toml::Value =
        toml::from_str(&std::fs::read_to_string(home.path().join("config.toml"))?)?;
    assert_eq!(saved["model_provider"].as_str(), Some("office"));
    Ok(())
}

#[tokio::test]
async fn gateway_oauth_displays_the_browser_link_and_retry_action() {
    let (widget, _home) = super::tests::widget_forced_chatgpt().await;
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
