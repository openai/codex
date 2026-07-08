use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml_with_chatgpt_base_url;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);
const REMOTE_IMAGE_URL_ERROR: &str =
    "remote image URLs are not supported; use an inline data URL instead";

#[tokio::test]
async fn request_handlers_reject_remote_image_urls() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_mock_responses_config_toml_with_chatgpt_base_url(
        codex_home.path(),
        "http://localhost/unused",
        "http://localhost/unused",
    )?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_request_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams::default())
        .await?;
    let thread_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_request_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_response)?;
    let thread_id = thread.id;

    let remote_tool_output = serde_json::to_value(ResponseItem::FunctionCallOutput {
        id: None,
        call_id: "call-1".to_string(),
        output: FunctionCallOutputPayload::from_content_items(vec![
            FunctionCallOutputContentItem::InputImage {
                image_url: "https://example.com/tool.png".to_string(),
                detail: Some(ImageDetail::High),
            },
        ]),
        internal_chat_message_metadata_passthrough: None,
    })?;
    let requests = [
        (
            "turn/start",
            json!({
                "threadId": thread_id,
                "input": [{
                    "type": "image",
                    "url": "HTTP://example.com/start.png",
                    "detail": "high"
                }]
            }),
        ),
        (
            "turn/steer",
            json!({
                "threadId": thread_id,
                "expectedTurnId": "turn-id",
                "input": [{
                    "type": "image",
                    "url": "https://example.com/steer.png",
                    "detail": "high"
                }]
            }),
        ),
        (
            "thread/inject_items",
            json!({
                "threadId": thread_id,
                "items": [remote_tool_output]
            }),
        ),
    ];

    for (method, params) in requests {
        let request_id = mcp.send_raw_request(method, Some(params)).await?;
        let actual: JSONRPCError = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
        )
        .await??;
        let expected = JSONRPCError {
            id: RequestId::Integer(request_id),
            error: JSONRPCErrorError {
                code: -32600,
                data: None,
                message: REMOTE_IMAGE_URL_ERROR.to_string(),
            },
        };
        assert_eq!(actual, expected, "unexpected response for {method}");
    }

    Ok(())
}
