use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const REQUESTED_MODEL: &str = "gpt-5.1-codex-max";
const SERVER_MODEL: &str = "gpt-5.2-codex";

#[tokio::test]
async fn openai_model_header_mismatch_emits_warning_item_v2() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "Done"),
        responses::ev_completed("resp-1"),
    ]);
    let response = responses::sse_response(body).insert_header("OpenAI-Model", SERVER_MODEL);
    let _response_mock = responses::mount_response_once(&server, response).await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some(REQUESTED_MODEL.to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "trigger safeguard".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let _turn_start: TurnStartResponse = to_response(_turn_resp)?;

    let warning_started = timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let notification: JSONRPCNotification = mcp
                .read_stream_until_notification_message("item/started")
                .await?;
            let params = notification.params.expect("item/started params");
            let started: ItemStartedNotification =
                serde_json::from_value(params).expect("deserialize item/started");
            if warning_text_from_item(&started.item).is_some_and(is_cyber_model_warning_text) {
                return Ok::<ItemStartedNotification, anyhow::Error>(started);
            }
        }
    })
    .await??;

    let warning_text =
        warning_text_from_item(&warning_started.item).expect("expected warning user message item");
    assert!(warning_text.contains("Warning:"));
    assert!(warning_text.contains("gpt-5.2 as a fallback"));
    assert!(warning_text.contains("regain access to gpt-5.3-codex"));

    let warning_completed = timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let notification: JSONRPCNotification = mcp
                .read_stream_until_notification_message("item/completed")
                .await?;
            let params = notification.params.expect("item/completed params");
            let completed: ItemCompletedNotification =
                serde_json::from_value(params).expect("deserialize item/completed");
            if warning_text_from_item(&completed.item).is_some_and(is_cyber_model_warning_text) {
                return Ok::<ItemCompletedNotification, anyhow::Error>(completed);
            }
        }
    })
    .await??;
    assert_eq!(
        warning_text_from_item(&warning_completed.item),
        warning_text_from_item(&warning_started.item)
    );

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    Ok(())
}

#[tokio::test]
async fn response_model_field_mismatch_emits_warning_item_v2_when_header_matches_requested()
-> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        serde_json::json!({
            "type": "response.created",
            "response": {
                "id": "resp-1",
                "model": SERVER_MODEL,
            }
        }),
        responses::ev_assistant_message("msg-1", "Done"),
        responses::ev_completed("resp-1"),
    ]);
    let response = responses::sse_response(body).insert_header("OpenAI-Model", REQUESTED_MODEL);
    let _response_mock = responses::mount_response_once(&server, response).await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some(REQUESTED_MODEL.to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "trigger response model check".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let _turn_start: TurnStartResponse = to_response(turn_resp)?;

    let warning_started = timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let notification: JSONRPCNotification = mcp
                .read_stream_until_notification_message("item/started")
                .await?;
            let params = notification.params.expect("item/started params");
            let started: ItemStartedNotification =
                serde_json::from_value(params).expect("deserialize item/started");
            if warning_text_from_item(&started.item).is_some_and(is_cyber_model_warning_text) {
                return Ok::<ItemStartedNotification, anyhow::Error>(started);
            }
        }
    })
    .await??;
    let warning_text =
        warning_text_from_item(&warning_started.item).expect("expected warning user message item");
    assert!(warning_text.contains("gpt-5.2 as a fallback"));

    let warning_completed = timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let notification: JSONRPCNotification = mcp
                .read_stream_until_notification_message("item/completed")
                .await?;
            let params = notification.params.expect("item/completed params");
            let completed: ItemCompletedNotification =
                serde_json::from_value(params).expect("deserialize item/completed");
            if warning_text_from_item(&completed.item).is_some_and(is_cyber_model_warning_text) {
                return Ok::<ItemCompletedNotification, anyhow::Error>(completed);
            }
        }
    })
    .await??;
    assert_eq!(
        warning_text_from_item(&warning_completed.item),
        warning_text_from_item(&warning_started.item)
    );

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    Ok(())
}

fn warning_text_from_item(item: &ThreadItem) -> Option<&str> {
    let ThreadItem::UserMessage { content, .. } = item else {
        return None;
    };

    content.iter().find_map(|input| match input {
        UserInput::Text { text, .. } if text.starts_with("Warning: ") => Some(text.as_str()),
        _ => None,
    })
}

fn is_cyber_model_warning_text(text: &str) -> bool {
    text.contains("flagged for potentially high-risk cyber activity")
        && text.contains("apply for trusted access: https://chatgpt.com/cyber")
}

fn create_config_toml(codex_home: &std::path::Path, server_uri: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "{REQUESTED_MODEL}"
approval_policy = "never"
sandbox_mode = "read-only"

model_provider = "mock_provider"

[features]
remote_models = false
personality = true

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}
