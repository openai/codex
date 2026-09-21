//! Board tools share MAv2 namespace metadata through the production installation path.

use anyhow::Context;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use codex_features::Feature;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::timeout;

#[test_case::test_case(None, "collaboration"; "default_namespace")]
#[test_case::test_case(Some("agents"), "agents"; "configured_namespace")]
#[tokio::test]
async fn board_tools_share_multi_agent_namespace(
    configured_namespace: Option<&str>,
    namespace: &str,
) -> anyhow::Result<()> {
    let server = responses::start_mock_server().await;
    let mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_function_call_with_namespace(
                    "list-channels",
                    namespace,
                    "get_channels",
                    "{}",
                ),
                responses::ev_completed("response-1"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("done", "Done."),
                responses::ev_completed("response-2"),
            ]),
        ],
    )
    .await;
    let codex_home = TempDir::new()?;
    let namespace_config = configured_namespace
        .map(|name| format!("tool_namespace = {name:?}"))
        .unwrap_or_default();
    MockResponsesConfig::new(&server.uri())
        .enable_feature(Feature::AgentMessageBoard)
        .disable_feature(Feature::CodeMode)
        .disable_feature(Feature::CodeModeOnly)
        .with_extra_config(&format!(
            "[features.multi_agent_v2]\nenabled = true\n{namespace_config}\n\
             [features.tool_registry]\nerror_on_tool_collisions = true"
        ))
        .write(codex_home.path())?;
    let mut app = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let thread = app.start_thread(ThreadStartParams::default()).await?.thread;
    let request = app
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            input: vec![UserInput::Text {
                text: "List the shared board channels.".into(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse = app.read_response(request).await?;
    timeout(
        Duration::from_secs(60),
        app.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    // Strict collision checking rejects different descriptions before sampling.
    // Both tool families must also be advertised under the configured name.
    for name in ["spawn_agent", "get_channels", "post"] {
        assert!(requests[0].tool_by_name(namespace, name).is_some());
    }
    let result: Value = serde_json::from_str(
        &requests[1]
            .function_call_output_text("list-channels")
            .context("board tool must execute through the installed extension")?,
    )?;
    assert_eq!(
        result,
        json!({"results": [], "n_returned": 0, "has_more": false, "next_cursor": null})
    );
    Ok(())
}
