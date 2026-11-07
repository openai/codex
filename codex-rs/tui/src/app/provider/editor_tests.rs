#![cfg(test)]

use super::*;
use crate::app::test_support::make_test_app;
use crate::chatwidget::tests::render_bottom_popup;
use codex_app_server_protocol::GatewayAuth;
use codex_app_server_protocol::ModelProviderSummary;
use codex_app_server_protocol::ProviderSettings;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use serde_json::json;

fn wire(params: ModelProviderConfigWriteParams) -> serde_json::Value {
    serde_json::to_value(params.settings).unwrap()
}

fn gateway(auth: GatewayAuth) -> ModelProviderSummary {
    ModelProviderSummary {
        id: "team".into(),
        name: "Team".into(),
        can_edit: true,
        base_url: Some("https://old.example".into()),
        settings: ProviderSettings::CustomGateway { auth },
    }
}

#[tokio::test]
async fn clearing_configured_url_keeps_the_form_open_until_corrected() {
    let mut app = make_test_app().await;
    app.chat_widget.toggle_vim_mode_and_notify();
    let mut provider = gateway(GatewayAuth::EnvironmentVariable {
        env_key: "TEAM_KEY".into(),
    });
    provider.name = "Team\ngateway".into();
    let draft = Draft::new(Some(provider.clone()), &[provider]);
    let (tx, rx) = std::sync::mpsc::channel();
    let mut form = view(&app, draft, /*error*/ None, move |draft, params| {
        tx.send((draft, params)).unwrap()
    });
    assert!(form.prefer_esc_to_handle_key_event());
    assert_eq!(
        form.cursor_style(Rect::default()),
        SetCursorStyle::SteadyBar
    );
    form.handle_key_event(KeyCode::Esc.into());
    assert!(!form.prefer_esc_to_handle_key_event());
    form.handle_key_event(KeyCode::Char('i').into());
    assert!(form.prefer_esc_to_handle_key_event());
    form.handle_key_event(KeyCode::Tab.into());
    for _ in 0.."https://old.example".len() {
        form.handle_key_event(KeyCode::Backspace.into());
    }
    let area = Rect::new(2, 4, 12, 24);
    let mut buf = Buffer::empty(area);
    form.render(area, &mut buf);
    assert_eq!(buf[form.cursor_pos(area).unwrap()].symbol(), "h");
    let chat = &mut app.chat_widget;
    chat.show_bottom_pane_view(form);
    for key in [KeyCode::Tab, KeyCode::Tab, KeyCode::Enter] {
        chat.handle_key_event(key.into());
    }
    assert!(rx.try_recv().is_err());
    assert!(render_bottom_popup(chat, /*width*/ 72).contains("Enter a base URL."));
    for _ in 0..2 {
        let back = KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT);
        chat.handle_key_event(back);
        chat.handle_key_event(KeyEvent {
            kind: KeyEventKind::Release,
            ..back
        });
    }
    chat.handle_paste("https://new.example".into());
    chat.handle_key_event(KeyCode::Tab.into());
    chat.handle_key_event(KeyCode::Tab.into());
    chat.handle_key_event(KeyEvent {
        kind: KeyEventKind::Repeat,
        ..KeyCode::Enter.into()
    });
    assert!(rx.try_recv().is_err());
    chat.handle_key_event(KeyCode::Enter.into());
    let (draft, params) = rx.try_recv().unwrap();
    assert_eq!(
        wire(params),
        json!({"type":"customGateway", "providerId":"team", "baseUrl":"https://new.example"})
    );
    let error = "Server validation failed.";
    let retry = view(&app, draft, Some(error.into()), |_, _| {});
    app.chat_widget
        .replace_bottom_pane_view_if_present(VIEW_ID, retry);
    let rendered = render_bottom_popup(&app.chat_widget, /*width*/ 72);
    assert!(rendered.contains("https://new.example"));
    assert!(rendered.contains(error));
}

#[test]
fn hidden_gateway_settings_stay_out_of_rename_patch() {
    let mut provider = gateway(GatewayAuth::ExternallyConfigured {
        env_key: Some("EXTERNAL".into()),
    });
    provider.base_url = None;
    let mut draft = Draft::new(Some(provider), &[]);
    assert_eq!(draft.fields(), vec![(Field::Name, Some("Team"))]);
    draft.changes.insert(Field::Name, "Renamed".into());
    assert_eq!(
        wire(draft.patch().unwrap()),
        json!({"type":"customGateway", "providerId":"team", "name":"Renamed"})
    );
}

#[tokio::test]
async fn entering_a_key_for_a_legacy_gateway_prompts_for_its_address() {
    let mut app = make_test_app().await;
    let mut provider = gateway(GatewayAuth::None);
    provider.base_url = None;
    let form = view(
        &app,
        Draft::new(Some(provider), &[]),
        /*error*/ None,
        |_, _| {
            panic!("submitted a gateway key without an endpoint");
        },
    );
    let chat = &mut app.chat_widget;
    chat.show_bottom_pane_view(form);
    chat.handle_key_event(KeyCode::Tab.into());
    chat.handle_key_event(KeyCode::Tab.into());
    chat.handle_paste("TEAM_KEY".into());
    chat.handle_key_event(KeyCode::Tab.into());
    chat.handle_key_event(KeyCode::Enter.into());
    insta::assert_snapshot!(render_bottom_popup(chat, /*width*/ 72), @r"
    Edit Team

      Name*  Team
      Base URL*  https://gateway.example/v1
      API key variable  $TEAM_KEY
    › Save
    Enter a base URL to use this API key variable.

    * Required
    Enter the environment variable name on the app server.
    tab move · enter next/save · esc back
    ");
}

#[test]
fn cloud_provider_edits_preserve_credentials() {
    let mut provider = gateway(GatewayAuth::None);
    provider.id = "amazon-bedrock-runtime".into();
    provider.settings = ProviderSettings::Bedrock {
        profile: None,
        region: None,
    };
    let mut bedrock = Draft::new(Some(provider.clone()), &[]);
    assert_eq!(bedrock.fields(), vec![(Field::Region, None)]);
    bedrock.changes.insert(Field::Region, "us-west-2".into());
    assert_eq!(
        wire(bedrock.patch().unwrap()),
        json!({"type":"bedrock", "providerId":"amazon-bedrock-runtime", "region":"us-west-2"})
    );
    provider.id = "gcp".into();
    provider.settings = ProviderSettings::Gcp {
        project_id: Some("dev".into()),
        location: Some("us-east1".into()),
        credential_file: Some("/srv/gcp.json".into()),
    };
    let mut gcp = Draft::new(Some(provider), &[]);
    gcp.changes.insert(Field::Location, String::new());
    assert_eq!(
        wire(gcp.patch().unwrap()),
        json!({"type":"gcp", "providerId":"gcp", "location":null})
    );
}
