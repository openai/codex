//! Board tools share namespace metadata; the detached SQLite server delivers native agent mail.

use super::connection_handling_websocket as websocket;
use anyhow::Context;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use codex_features::Feature;
use codex_rmcp_client::RmcpClient;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::io::AsyncBufReadExt;
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
    // Detached delivery is independent of the configured native tool namespace.
    if configured_namespace.is_some() {
        return Ok(());
    }

    // Reuse the real WebSocket server to exercise the detached executable's callback.
    // Its second agent is a native child, so ordinary direct-input injection is forbidden.
    let (mut app_process, address) = websocket::spawn_websocket_server(codex_home.path()).await?;
    let mut ws = websocket::connect_websocket(address).await?;
    websocket::send_initialize_request(&mut ws, /*id*/ 1, "board-notifications").await?;
    websocket::read_response_for_id(&mut ws, /*id*/ 1).await?;
    let program = codex_utils_cargo_bin::cargo_bin("codex-message-board-mcp")?;
    let mut board_process =
        tokio::process::Command::from(codex_utils_process::background_command(program))
            .arg("--sqlite-home")
            .arg(codex_home.path())
            .args([
                "--listen",
                "127.0.0.1:0",
                "--app-server",
                &format!("ws://{address}"),
            ])
            .env(
                "CODEX_MESSAGE_BOARD_TOKEN",
                "sqlite-notifications-test-token",
            )
            .stdout(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
    let ready = timeout(
        websocket::DEFAULT_READ_TIMEOUT,
        tokio::io::BufReader::new(board_process.stdout.take().context("board stdout")?)
            .lines()
            .next_line(),
    )
    .await??
    .context("board server readiness")?;
    let ready: Value = serde_json::from_str(&ready)?;
    // The board must reconnect before sending its first notice after a known disconnect.
    app_process.kill().await?;
    let (restarted, _) = websocket::spawn_websocket_server_with_args(
        codex_home.path(),
        &format!("ws://{address}"),
        &[],
    )
    .await?;
    app_process = restarted;
    ws = websocket::connect_websocket(address).await?;
    websocket::send_initialize_request(&mut ws, /*id*/ 1, "board-notifications").await?;
    websocket::read_response_for_id(&mut ws, /*id*/ 1).await?;
    let root = websocket::start_thread(&mut ws, /*id*/ 2).await?;
    // Parent and child requests can arrive in either order. Give both terminal answers
    // so late child results stay queued instead of extending the parent's turn.
    responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_function_call_with_namespace(
                    "spawn-notification-recipient",
                    namespace,
                    "spawn_agent",
                    r#"{"task_name":"worker","message":"Say ready.","fork_turns":"none"}"#,
                ),
                responses::ev_completed("spawn-recipient"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("recipient-ready-message", "Ready."),
                responses::ev_completed("recipient-ready"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("parent-ready-message", "Ready."),
                responses::ev_completed("parent-ready"),
            ]),
        ],
    )
    .await;
    websocket::send_request(
        &mut ws,
        "turn/start",
        /*id*/ 3,
        Some(json!({"threadId":root,"input":[{"type":"text","text":"Spawn a worker."}]})),
    )
    .await?;
    for _ in 0..2 {
        let completion = websocket::read_notification_for_method(&mut ws, "turn/completed").await?;
        assert_eq!(
            completion.params.context("completion params")?["turn"]["status"],
            "completed"
        );
    }

    let client = RmcpClient::new_streamable_http_client(
        "sqlite-notifications",
        ready["url"].as_str().context("board URL")?,
        Some("sqlite-notifications-test-token".into()),
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        codex_config::types::OAuthCredentialsStoreMode::File,
        Default::default(),
        codex_exec_server::Environment::default_for_tests().get_http_client(),
        /*auth_provider*/ None,
    )
    .await?;
    client
        .initialize(
            rmcp::model::InitializeRequestParams::default(),
            /*timeout*/ None,
            Box::new(|_, _| Box::pin(async { anyhow::bail!("unexpected elicitation") })),
        )
        .await?;
    let posted = client
        .call_tool(
            "post".into(),
            Some(json!({
                "new_channel_name":"delivery",
                "text":"A notice from the detached SQLite board.",
                "agents_to_notify":["/root/worker"]
            })),
            |_| Ok(Some(json!({"threadId":root,"sessionId":root}))),
            /*timeout*/ None,
        )
        .await?;
    assert_ne!(posted.is_error, Some(true), "{posted:?}");
    let post = posted.structured_content.context("posted metadata")?;
    let notification = format!(
        "Message Type: CHANNEL_POST\nSender: /root\nChannel: delivery\nMessage ID: {}\nThread ID: {}\nPayload:\nA notice from the detached SQLite board.",
        post["message_id"].as_str().context("message id")?,
        post["thread_id"].as_str().context("thread id")?,
    );

    // Queue-only mail must be available when the parent follows up with its idle child.
    let delivered = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_function_call_with_namespace(
                    "consume-notification",
                    namespace,
                    "followup_task",
                    r#"{"target":"/root/worker","message":"Read the notification."}"#,
                ),
                responses::ev_completed("resume-recipient"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("notification-consumed-message", "Done."),
                responses::ev_completed("notification-consumed"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("parent-complete-message", "Done."),
                responses::ev_completed("parent-complete"),
            ]),
        ],
    )
    .await;
    websocket::send_request(
        &mut ws,
        "turn/start",
        /*id*/ 4,
        Some(
            json!({"threadId":root,"input":[{"type":"text","text":"Follow up with the worker."}]}),
        ),
    )
    .await?;
    for _ in 0..2 {
        let completion = websocket::read_notification_for_method(&mut ws, "turn/completed").await?;
        assert_eq!(
            completion.params.context("completion params")?["turn"]["status"],
            "completed"
        );
    }
    let delivered_items = delivered
        .requests()
        .into_iter()
        .flat_map(|request| request.inputs_of_type("agent_message"))
        .map(responses::strip_metadata_from_json)
        .map(responses::strip_response_item_ids_from_json)
        .collect::<Vec<_>>();
    assert_eq!(
        delivered_items
            .iter()
            .filter(|item| item["content"] == json!([{"type":"input_text","text":notification}]))
            .collect::<Vec<_>>(),
        vec![&json!({
            "type":"agent_message","author":"/root","recipient":"/root/worker",
            "content":[{"type":"input_text","text":notification}]
        })],
    );
    client.shutdown().await;
    board_process.kill().await?;
    app_process.kill().await?;
    Ok(())
}
