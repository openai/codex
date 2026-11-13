#![cfg(test)]
//! Check the provider-facing values for the displayed conversation.

use super::app_server::configured_thread_session;
use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test]
async fn statusline_state_projects_the_displayed_settings() {
    let (mut chat, _events, _ops) = make_chatwidget_manual(Some("gpt-5.5")).await;
    let id = ThreadId::new();
    let mut session = configured_thread_session(id);
    session.model = "gpt-5.5".into();
    session.thread_name = Some("Saved carts".into());
    let expected_cwd = session.cwd.to_path_buf();
    chat.handle_thread_session(session);
    chat.local_settings.tui.status_line = Some(vec!["status".into()]);
    let mut model = get_available_model(&chat, "gpt-5.5");
    model.display_name = "Test model".into();
    model.service_tiers = vec![codex_protocol::openai_models::ModelServiceTier {
        id: "acme-priority".into(),
        name: "Turbo".into(),
        description: String::new(),
    }];
    helpers::set_test_model_catalog(&mut chat, vec![model]);
    chat.set_service_tier(Some("acme-priority".into()));
    chat.set_reasoning_effort(Some(ReasoningEffortConfig::None));

    assert_eq!(
        serde_json::to_value(chat.statusline_state()).unwrap(),
        json!({
            "app": {"cli_version": CODEX_CLI_VERSION, "display_mode": "standard"},
            "config": {"statusline_items": ["run-state"]},
            "thread": {
                "workspace": {"cwd": expected_cwd},
                "id": id, "name": "Saved carts", "agent": {"state": "ready"},
                "model": {"id": "gpt-5.5", "display_name": "Test model", "provider": "openai",
                    "reasoning_level": "none", "service_tier": "acme-priority", "service_tier_display_name": "Turbo"},
                "approval": {"policy": "never", "reviewer": "user"},
                "permissions": {"display_name": "Read Only", "type": "managed", "file_system": {"type":"restricted"}, "network": "restricted"}
            }
        })
    );

    chat.set_reasoning_effort(Some(ReasoningEffortConfig::Custom("default".into())));
    let state = serde_json::to_value(chat.statusline_state()).unwrap();
    assert_eq!(state["thread"]["model"]["reasoning_level"], "default");
}
