use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sleep_emits_started_and_completed_items() -> Result<()> {
    const CALL_ID: &str = "sleep-1";
    const DURATION_MS: u64 = 1;

    let server = responses::start_mock_server().await;
    responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_function_call(
                    CALL_ID,
                    "sleep",
                    &serde_json::json!({ "duration_ms": DURATION_MS }).to_string(),
                ),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("msg-1", "Done"),
                responses::ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_start_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(thread_start_response)?;

    let turn_start_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![V2UserInput::Text {
                text: "Sleep briefly".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let turn_start_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_start_id)),
    )
    .await??;
    let TurnStartResponse { turn, .. } = to_response(turn_start_response)?;

    let (started, completed) = timeout(DEFAULT_READ_TIMEOUT, async {
        let mut started = None;
        let mut completed = None;
        while started.is_none() || completed.is_none() {
            let JSONRPCMessage::Notification(notification) = mcp.read_next_message().await? else {
                continue;
            };
            match notification.method.as_str() {
                "item/started" => {
                    let payload: ItemStartedNotification =
                        serde_json::from_value(notification.params.expect("item/started params"))?;
                    if matches!(&payload.item, ThreadItem::Sleep { .. }) {
                        started = Some(payload);
                    }
                }
                "item/completed" => {
                    let payload: ItemCompletedNotification = serde_json::from_value(
                        notification.params.expect("item/completed params"),
                    )?;
                    if matches!(&payload.item, ThreadItem::Sleep { .. }) {
                        completed = Some(payload);
                    }
                }
                _ => {}
            }
        }
        Ok::<_, anyhow::Error>((
            started.expect("sleep started"),
            completed.expect("sleep completed"),
        ))
    })
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let expected_item = ThreadItem::Sleep {
        id: CALL_ID.to_string(),
        duration_ms: DURATION_MS,
    };
    assert!(completed.completed_at_ms >= started.started_at_ms);
    assert_eq!(
        started,
        ItemStartedNotification {
            item: expected_item.clone(),
            thread_id: thread.id.clone(),
            turn_id: turn.id.clone(),
            started_at_ms: started.started_at_ms,
        }
    );
    assert_eq!(
        completed,
        ItemCompletedNotification {
            item: expected_item,
            thread_id: thread.id,
            turn_id: turn.id,
            completed_at_ms: completed.completed_at_ms,
        }
    );

    Ok(())
}

fn create_config_toml(codex_home: &Path, server_uri: &str) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"
model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0

[features]
sleep_tool = true
"#
        ),
    )
}
