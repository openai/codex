//! Failed enterprise authentication leaves ordinary MCP calls and model history unchanged.

use anyhow::Result;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_login::CodexAuth;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::McpStartupStatus;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn enterprise_failure_preserves_ordinary_mcp_history() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let mut histories = Vec::new();
    for enabled in [false, true] {
        let server = responses::start_mock_server().await;
        let mcp = responses::start_mock_server().await;
        AppsTestServer::mount(&mcp).await?;
        let origin = mcp.uri();
        let policy = format!(
            r#"
[features]
use_xaa = {enabled}
apps = false
secret_auth_storage = false
[mcp_enterprise_managed_auth.idp]
issuer = "{origin}/idp"
client_id = "enterprise-client"
[mcp_servers.enterprise]
url = "{origin}/enterprise"
auth = "ema_auth"
[mcp_servers.ordinary]
url = "{origin}/api/codex/ps/mcp"
http_headers = {{ Authorization = "Bearer ordinary-access" }}
"#
        );
        let test = test_codex()
            .with_auth(CodexAuth::from_api_key("test-api-key"))
            .with_cloud_config_bundle(CloudConfigBundleFixture::loader_with_enterprise_config(
                policy,
            ))
            .with_config(super::configure_scenario_catalog)
            .with_model_info_override("gpt-6-astra", |model| {
                model.tool_mode = Some(ToolMode::Direct);
                model.use_responses_lite = false;
                model.supports_search_tool = false;
            })
            .build_with_auto_env(&server)
            .await?;
        if enabled {
            let error = wait_for_event_match(&test.codex, |event| match event {
                EventMsg::McpStartupUpdate(update) if update.server == "enterprise" => {
                    match &update.status {
                        McpStartupStatus::Failed { error, .. } => Some(error.clone()),
                        _ => None,
                    }
                }
                _ => None,
            })
            .await;
            insta::assert_snapshot!("enterprise_sign_in_required", error);
        }
        let startup = wait_for_event_match(&test.codex, |event| match event {
            EventMsg::McpStartupComplete(summary) => Some(summary.clone()),
            _ => None,
        })
        .await;
        assert_eq!(startup.ready, ["ordinary"]);
        assert_eq!(
            startup
                .failed
                .into_iter()
                .map(|failure| failure.server)
                .collect::<Vec<_>>(),
            if enabled {
                vec!["enterprise"]
            } else {
                Vec::new()
            }
        );
        let mock = responses::mount_sse_sequence(
            &server,
            vec![
                responses::sse(vec![
                    responses::ev_function_call_with_namespace(
                        "calendar-call",
                        "mcp__ordinary",
                        "calendar_list_events",
                        "{}",
                    ),
                    responses::ev_completed("call-response"),
                ]),
                responses::sse(vec![
                    responses::ev_assistant_message("answer", "Here are your calendar events."),
                    responses::ev_completed("answer-response"),
                ]),
            ],
        )
        .await;
        test.submit_text_turn("List my calendar events.").await?;
        let requests = mock.requests();
        assert_eq!(requests.len(), 2);
        histories.push(context_snapshot::format_request_history_snapshot(
            "Ordinary MCP keeps the same tool schemas, calls, and history when enterprise authentication fails.",
            &requests,
            &ContextSnapshotOptions::default().rewrite_known_segments().include_request_settings(),
        ));
        assert!(
            mcp.received_requests()
                .await
                .expect("requests")
                .iter()
                .all(|request| {
                    !request.url.path().contains("enterprise")
                        && !request.url.path().contains("idp")
                })
        );
        test.codex.shutdown_and_wait().await?;
    }
    assert_eq!(histories[0], histories[1]);
    insta::assert_snapshot!(
        "enterprise_failure_preserves_ordinary_mcp_history",
        histories[1]
    );
    Ok(())
}
