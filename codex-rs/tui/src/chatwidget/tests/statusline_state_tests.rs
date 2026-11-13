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
            "account": {"rate_limits": []},
            "app": {"cli_version": CODEX_CLI_VERSION, "display_mode": "standard"},
            "config": {"statusline_items": ["run-state"]},
            "thread": {
                "workspace": {"cwd": expected_cwd},
                "usage": {"context": {}},
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

#[tokio::test]
async fn statusline_totals_use_the_reported_token_breakdown() {
    let (mut chat, _events, _ops) = make_chatwidget_manual(Some("gpt-5.2")).await;
    chat.handle_thread_session(configured_thread_session(ThreadId::new()));
    let mut info =
        helpers::make_token_info(/*total_tokens*/ 15, /*context_window*/ 100_000);
    info.total_token_usage = TokenUsage {
        input_tokens: 110,
        cached_input_tokens: 60,
        cache_write_input_tokens: 10,
        output_tokens: 20,
        total_tokens: 130,
        ..Default::default()
    };
    helpers::handle_token_count(&mut chat, Some(info.clone()));
    let state = serde_json::to_value(chat.statusline_state()).unwrap();
    assert_eq!(
        state["thread"]["usage"]["total"],
        json!({
            "total_input_tokens": 110, "cache_read_input_tokens": 60,
            "cache_write_input_tokens": 10, "other_input_tokens": 40, "output_tokens": 20
        })
    );

    info.total_token_usage.cache_write_input_tokens = 51;
    for (report, has_total) in [
        (info, false), // The cache counts exceed total input.
        (
            helpers::make_token_info(
                /*total_tokens*/ 100_000, /*context_window*/ 100_000,
            ),
            false,
        ), // Synthetic context fill.
        (
            helpers::make_token_info(/*total_tokens*/ 0, /*context_window*/ 100_000),
            true,
        ), // Measured zero.
    ] {
        helpers::handle_token_count(&mut chat, Some(report));
        let state = serde_json::to_value(chat.statusline_state()).unwrap();
        assert_eq!(state["thread"]["usage"].get("total").is_some(), has_total);
    }
}

#[tokio::test]
async fn statusline_context_uses_the_footer_window_and_percentage() {
    let (mut chat, _events, _ops) = make_chatwidget_manual(Some("gpt-5.2")).await;
    chat.handle_thread_session(configured_thread_session(ThreadId::new()));
    chat.config.model_context_window = Some(80_000);
    helpers::handle_token_count(&mut chat, /*info*/ None);
    let state = serde_json::to_value(chat.statusline_state()).unwrap();
    assert_eq!(
        state["thread"]["usage"]["context"],
        json!({"context_window_size": 80_000})
    );

    // The last response occupies half the usable window after the footer's fixed overhead.
    let mut info = helpers::make_token_info(
        /*total_tokens*/ 56_000, /*context_window*/ 100_000,
    );
    info.total_token_usage.total_tokens = 220_000;
    helpers::handle_token_count(&mut chat, Some(info));
    let state = serde_json::to_value(chat.statusline_state()).unwrap();
    assert_eq!(
        state["thread"]["usage"]["context"],
        json!({
            "context_window_size": 100_000, "used_tokens": 56_000, "used_percentage": 50
        })
    );
}
