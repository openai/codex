//! Exercises protocol-gated stdio sharing through chat startup, tool dispatch, and shutdown.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ExperimentalFeatureEnablementSetParams;
use codex_app_server_protocol::ExperimentalFeatureEnablementSetResponse;
use codex_app_server_protocol::McpServerToolCallParams;
use codex_app_server_protocol::McpServerToolCallResponse;
use codex_app_server_protocol::ThreadClosedNotification;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadUnsubscribeParams;
use codex_app_server_protocol::ThreadUnsubscribeResponse;
use codex_app_server_protocol::ThreadUnsubscribeStatus;
use core_test_support::skip_if_remote;
use core_test_support::stdio_server_bin;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

#[tokio::test]
async fn stdio_sharing_follows_protocol_and_enablement_for_loaded_chats() -> Result<()> {
    skip_if_remote!(Ok(()), "the local stdio executable is a host-local fixture");
    for (modern, modern_without_sharing) in [(false, false), (true, false), (true, true)] {
        let responses = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
        let codex_home = TempDir::new()?;
        let command = toml::Value::String(stdio_server_bin()?);
        // Exercise both pool-driven protocol selection and modern connections without pooling.
        let mut config = format!("[features]\nmcp_2026_07_28 = {modern_without_sharing}\n");
        for name in ["alpha", "beta"] {
            config.push_str(&format!(
                "[mcp_servers.{name}]\ncommand = {command}\nenabled_tools = [\"echo\"]\n\
                 [mcp_servers.{name}.env]\nMCP_TEST_DYNAMIC_SERVER_METADATA = \"1\"\n"
            ));
            if name == "beta" && modern {
                config.push_str("CODEX_MCP_PROTOCOL_VERSION = \"2026-07-28\"\n");
            } else if name == "beta" {
                config.push_str("CODEX_MCP_PROTOCOL_VERSION = \"2025-06-18\"\n");
            }
        }
        MockResponsesConfig::new(&responses.uri())
            .with_root_config("thread_unload_delay_secs = 0")
            .with_extra_config(&config)
            .write(codex_home.path())?;
        let mut app = TestAppServer::builder()
            .with_codex_home(codex_home.path())
            .build_initialized()
            .await?;
        set_sharing(&mut app, /*enabled*/ true).await?;
        let first = app.start_thread(ThreadStartParams::default()).await?.thread;
        let mut first_processes = call_servers(&mut app, &first.id).await?;
        assert_ne!(first_processes["alpha"], first_processes["beta"]);
        let second = app.start_thread(ThreadStartParams::default()).await?.thread;
        let mut second_processes = call_servers(&mut app, &second.id).await?;
        for name in ["alpha", "beta"] {
            assert_eq!(
                first_processes[name] == second_processes[name],
                name == "alpha" || modern
            );
        }
        if modern_without_sharing {
            for sharing in [false, true] {
                set_sharing(&mut app, sharing).await?;
                first_processes = call_servers(&mut app, &first.id).await?;
                second_processes = call_servers(&mut app, &second.id).await?;
                for name in ["alpha", "beta"] {
                    assert_eq!(first_processes[name] == second_processes[name], sharing);
                }
            }
        }
        unload_thread(&mut app, &first.id).await?;
        assert_eq!(call_servers(&mut app, &second.id).await?, second_processes);
        unload_thread(&mut app, &second.id).await?;
        #[cfg(unix)]
        for process in first_processes.values().chain(second_processes.values()) {
            core_test_support::process::wait_for_process_exit(
                process
                    .strip_prefix("rmcp-test-process-")
                    .expect("the fixture identifies its PID"),
            )
            .await?;
        }
        #[cfg(not(unix))]
        {
            let third = app.start_thread(ThreadStartParams::default()).await?.thread;
            let third_processes = call_servers(&mut app, &third.id).await?;
            for name in ["alpha", "beta"] {
                assert_ne!(third_processes[name], first_processes[name]);
            }
        }
    }
    Ok(())
}

async fn set_sharing(app: &mut TestAppServer, enabled: bool) -> Result<()> {
    let enablement = BTreeMap::from([("mcp_connection_sharing".to_string(), enabled)]);
    let response: ExperimentalFeatureEnablementSetResponse = app
        .request(
            |request_id| ClientRequest::ExperimentalFeatureEnablementSet {
                request_id,
                params: ExperimentalFeatureEnablementSetParams {
                    enablement: enablement.clone(),
                },
            },
        )
        .await?;
    assert_eq!(response.enablement, enablement);
    Ok(())
}

async fn call_servers(
    app: &mut TestAppServer,
    thread_id: &str,
) -> Result<HashMap<&'static str, String>> {
    let mut processes = HashMap::new();
    for name in ["alpha", "beta"] {
        let response: McpServerToolCallResponse = app
            .request(|request_id| ClientRequest::McpServerToolCall {
                request_id,
                params: McpServerToolCallParams {
                    thread_id: thread_id.to_string(),
                    server: name.to_string(),
                    tool: "echo".to_string(),
                    arguments: Some(json!({ "message": "ping" })),
                    meta: None,
                },
            })
            .await?;
        let result = response.structured_content.expect("echo result");
        let process = result["echo"]
            .as_str()
            .expect("the fixture identifies its process");
        processes.insert(name, process.to_owned());
    }
    Ok(processes)
}

async fn unload_thread(app: &mut TestAppServer, thread_id: &str) -> Result<()> {
    let response: ThreadUnsubscribeResponse = app
        .request(|request_id| ClientRequest::ThreadUnsubscribe {
            request_id,
            params: ThreadUnsubscribeParams {
                thread_id: thread_id.to_string(),
            },
        })
        .await?;
    assert_eq!(
        response,
        ThreadUnsubscribeResponse {
            status: ThreadUnsubscribeStatus::Unsubscribed,
        }
    );
    let closed: ThreadClosedNotification = timeout(
        Duration::from_secs(/*secs*/ 10),
        app.read_notification("thread/closed"),
    )
    .await??;
    assert_eq!(
        closed,
        ThreadClosedNotification {
            thread_id: thread_id.to_string(),
        }
    );
    Ok(())
}
