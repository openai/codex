//! Verifies that a hosted Apps protocol opt-in refreshes an existing app-server thread.

use std::collections::BTreeMap;

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ExperimentalFeatureEnablementSetParams;
use codex_app_server_protocol::ExperimentalFeatureEnablementSetResponse;
use codex_app_server_protocol::McpServerToolCallParams;
use codex_app_server_protocol::McpServerToolCallResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_config::types::AuthCredentialsStoreMode;
use codex_features::Feature;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn enabling_hosted_apps_protocol_refreshes_an_existing_thread() -> Result<()> {
    let server = responses::start_mock_server().await;
    let apps = AppsTestServer::mount(&server).await?;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_root_config(&format!("chatgpt_base_url = \"{}\"", apps.chatgpt_base_url))
        .enable_feature(Feature::Apps)
        .write(codex_home.path())?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let thread_id = app_server
        .start_thread(ThreadStartParams::default())
        .await?
        .thread
        .id;

    let call_tool = |thread_id: String| McpServerToolCallParams {
        environment_id: None,
        thread_id,
        server: "codex_apps".to_string(),
        tool: "calendar_list_events".to_string(),
        arguments: Some(json!({ "query": "hello" })),
        meta: None,
    };
    let _: McpServerToolCallResponse = app_server
        .request(|request_id| ClientRequest::McpServerToolCall {
            request_id,
            params: call_tool(thread_id.clone()),
        })
        .await?;
    let methods = server
        .received_requests()
        .await
        .expect("mock server should capture Apps MCP startup requests")
        .into_iter()
        .filter(|request| request.url.path() == "/api/codex/ps/mcp")
        .filter_map(|request| {
            let body: Value = serde_json::from_slice(&request.body).ok()?;
            body.get("method")?.as_str().map(str::to_string)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        methods,
        [
            "initialize",
            "notifications/initialized",
            "tools/list",
            "tools/call"
        ]
    );

    let enablement = BTreeMap::from([("codex_apps_mcp_2026_07_28".to_string(), true)]);
    let request_id = app_server
        .send_experimental_feature_enablement_set_request(ExperimentalFeatureEnablementSetParams {
            enablement: enablement.clone(),
        })
        .await?;
    let response: ExperimentalFeatureEnablementSetResponse =
        app_server.read_response(request_id).await?;
    assert_eq!(response.enablement, enablement);

    let _: McpServerToolCallResponse = app_server
        .request(|request_id| ClientRequest::McpServerToolCall {
            request_id,
            params: call_tool(thread_id),
        })
        .await?;
    let methods = server
        .received_requests()
        .await
        .expect("mock server should capture Apps MCP startup requests")
        .into_iter()
        .filter(|request| request.url.path() == "/api/codex/ps/mcp")
        .filter_map(|request| {
            let body: Value = serde_json::from_slice(&request.body).ok()?;
            body.get("method")?.as_str().map(str::to_string)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        methods,
        [
            "initialize",
            "notifications/initialized",
            "tools/list",
            "tools/call",
            "server/discover",
            "initialize",
            "notifications/initialized",
            "tools/list",
            "tools/call",
        ]
    );
    Ok(())
}
