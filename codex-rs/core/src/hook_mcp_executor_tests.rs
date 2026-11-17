#![cfg(test)]
//! App hook dispatch follows refreshed connector policy without exposing app-only tools.

use super::*;
use crate::ConfigRefreshOutcome;
use crate::session::tests::make_session_and_context_with_auth_config_home_and_rx;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_mcp::McpServerKey;
use core_test_support::PathExt;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::recorded_apps_tool_calls;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Mutex;
use std::time::Duration;

#[tokio::test]
async fn app_hook_dispatch_follows_current_policy() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let apps = AppsTestServer::mount_with_tools(
        &server,
        Arc::new(Mutex::new(vec![json!({
            "name": "calendar.read_context",
            "description": "Read calendar context.",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": { "readOnlyHint": true },
            "_meta": {
                "connector_id": "calendar", "connector_name": "Calendar",
                "ui": { "visibility": ["app"] },
                "_codex_apps": {
                    "resource_uri": "connector://calendar/tools/calendar.read_context",
                    "contains_mcp_source": true, "connector_id": "calendar",
                },
            },
        })])),
    )
    .await?;
    let home = tempfile::tempdir()?;
    let (session, _turn, _events) = make_session_and_context_with_auth_config_home_and_rx(
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        Vec::new(),
        home.path(),
        |config| {
            config.chatgpt_base_url = apps.chatgpt_base_url;
            config.features.enable(Feature::Apps).unwrap();
        },
    )
    .await;
    let executor = CoreHookMcpExecutor {
        session: OnceLock::from(Arc::downgrade(&session)),
        runtime: Arc::clone(&session.services.mcp_runtime),
        thread_id: session.thread_id,
    };
    for (policy, connector, allowed, calls) in [
        ("", "calendar", true, 1),
        (
            "default_tools_approval_mode = 'prompt'",
            "calendar",
            false,
            1,
        ),
        ("enabled = false", "calendar", false, 1),
        ("", "different_connector", false, 1),
        ("", "calendar", true, 2),
    ] {
        let current = session.get_config().await;
        let mut next = (*current).clone();
        next.config_layer_stack = next.config_layer_stack.with_user_config(
            &next.codex_home.join("config.toml").abs(),
            toml::from_str(&format!(
                "[features]\napps = true\n[apps.calendar]\n{policy}"
            ))?,
        )?;
        assert_eq!(
            session.refresh_runtime_config(current, next).await,
            ConfigRefreshOutcome::Published
        );
        let result = executor
            .execute(HookMcpCall {
                expected_app_connector_id: Some(connector),
                skip_dormant_repl_cleanup: false,
                server: CODEX_APPS_MCP_SERVER_NAME.into(),
                tool: "calendar.read_context".into(),
                environment_id: None,
                metadata: None,
                input: Default::default(),
                timeout: Duration::from_secs(30),
            })
            .await;
        assert_eq!(
            (
                result.is_ok(),
                recorded_apps_tool_calls(&server).await.len()
            ),
            (allowed, calls),
            "{policy}: {:?}",
            result.err()
        );
    }
    let binding = executor
        .runtime
        .current_binding_for_call(&McpServerKey::unscoped(CODEX_APPS_MCP_SERVER_NAME))
        .await
        .unwrap();
    assert!(
        binding
            .tools()
            .iter()
            .all(|info| info.tool.name != "calendar.read_context")
    );
    Ok(())
}
