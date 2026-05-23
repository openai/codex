use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadInjectItemsParams;
use codex_app_server_protocol::ThreadInjectItemsResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadSuggestNextPromptResponse;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::sleep;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(/*secs*/ 10);

#[tokio::test]
async fn thread_suggest_next_prompt_samples_loaded_history_without_tools() -> Result<()> {
    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "run the tests"),
        responses::ev_completed("resp-1"),
    ]);
    let response_mock = responses::mount_sse_once(&server, body).await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;

    inject_suggestion_history(&mut mcp, &thread.id).await?;

    let request_id = mcp
        .send_raw_request(
            "thread/suggestNextPrompt",
            Some(json!({ "threadId": thread.id })),
        )
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response = to_response::<ThreadSuggestNextPromptResponse>(response)?;

    assert_eq!(response.suggestion.as_deref(), Some("run the tests"));
    let request = response_mock.single_request();
    assert_eq!(request.body_json()["tools"], json!([]));
    assert!(
        request
            .message_input_texts("user")
            .last()
            .is_some_and(|text| text.contains("[SUGGESTION MODE:")),
        "suggestion prompt should be appended as the final user message"
    );

    Ok(())
}

#[tokio::test]
async fn thread_suggest_next_prompt_cancel_stops_in_flight_sampling() -> Result<()> {
    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "run the tests"),
        responses::ev_completed("resp-1"),
    ]);
    let response_mock = responses::mount_response_once(
        &server,
        responses::sse_response(body).set_delay(std::time::Duration::from_secs(/*secs*/ 5)),
    )
    .await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;
    inject_suggestion_history(&mut mcp, &thread.id).await?;

    let suggestion_req = mcp
        .send_raw_request(
            "thread/suggestNextPrompt",
            Some(json!({
                "threadId": thread.id,
                "cancellationToken": "next-prompt-suggestion-test",
            })),
        )
        .await?;
    wait_for_request_count(&response_mock, /*expected*/ 1).await?;
    let cancel_req = mcp
        .send_raw_request(
            "thread/suggestNextPrompt",
            Some(json!({
                "threadId": thread.id,
                "cancellationToken": "next-prompt-suggestion-test",
                "cancel": true,
            })),
        )
        .await?;

    let cancel_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(cancel_req)),
    )
    .await??;
    let cancel_resp = to_response::<ThreadSuggestNextPromptResponse>(cancel_resp)?;
    assert_eq!(cancel_resp.suggestion, None);

    let suggestion_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(suggestion_req)),
    )
    .await??;
    let suggestion_resp = to_response::<ThreadSuggestNextPromptResponse>(suggestion_resp)?;
    assert_eq!(suggestion_resp.suggestion, None);
    assert_eq!(response_mock.requests().len(), /*expected*/ 1);

    Ok(())
}

#[tokio::test]
async fn thread_suggest_next_prompt_cancel_is_scoped_to_thread() -> Result<()> {
    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "run the tests"),
        responses::ev_completed("resp-1"),
    ]);
    let response_mock = responses::mount_response_once(
        &server,
        responses::sse_response(body)
            .set_delay(std::time::Duration::from_millis(/*millis*/ 100)),
    )
    .await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let first_thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let first_thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(first_thread_req)),
    )
    .await??;
    let ThreadStartResponse {
        thread: first_thread,
        ..
    } = to_response::<ThreadStartResponse>(first_thread_resp)?;
    inject_suggestion_history(&mut mcp, &first_thread.id).await?;

    let second_thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let second_thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(second_thread_req)),
    )
    .await??;
    let ThreadStartResponse {
        thread: second_thread,
        ..
    } = to_response::<ThreadStartResponse>(second_thread_resp)?;

    let suggestion_req = mcp
        .send_raw_request(
            "thread/suggestNextPrompt",
            Some(json!({
                "threadId": first_thread.id,
                "cancellationToken": "next-prompt-suggestion-shared-token",
            })),
        )
        .await?;
    wait_for_request_count(&response_mock, /*expected*/ 1).await?;

    let cancel_req = mcp
        .send_raw_request(
            "thread/suggestNextPrompt",
            Some(json!({
                "threadId": second_thread.id,
                "cancellationToken": "next-prompt-suggestion-shared-token",
                "cancel": true,
            })),
        )
        .await?;
    let cancel_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(cancel_req)),
    )
    .await??;
    let cancel_resp = to_response::<ThreadSuggestNextPromptResponse>(cancel_resp)?;
    assert_eq!(cancel_resp.suggestion, None);

    let suggestion_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(suggestion_req)),
    )
    .await??;
    let suggestion_resp = to_response::<ThreadSuggestNextPromptResponse>(suggestion_resp)?;
    assert_eq!(suggestion_resp.suggestion.as_deref(), Some("run the tests"));
    assert_eq!(response_mock.requests().len(), /*expected*/ 1);

    Ok(())
}

#[tokio::test]
async fn thread_suggest_next_prompt_cancel_before_start_suppresses_sampling() -> Result<()> {
    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "run the tests"),
        responses::ev_completed("resp-1"),
    ]);
    let response_mock = responses::mount_sse_once(&server, body).await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;
    inject_suggestion_history(&mut mcp, &thread.id).await?;

    let cancel_req = mcp
        .send_raw_request(
            "thread/suggestNextPrompt",
            Some(json!({
                "threadId": thread.id,
                "cancellationToken": "next-prompt-suggestion-immediate-cancel",
                "cancel": true,
            })),
        )
        .await?;
    let cancel_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(cancel_req)),
    )
    .await??;
    let cancel_resp = to_response::<ThreadSuggestNextPromptResponse>(cancel_resp)?;
    assert_eq!(cancel_resp.suggestion, None);

    let suggestion_req = mcp
        .send_raw_request(
            "thread/suggestNextPrompt",
            Some(json!({
                "threadId": thread.id,
                "cancellationToken": "next-prompt-suggestion-immediate-cancel",
            })),
        )
        .await?;
    let suggestion_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(suggestion_req)),
    )
    .await??;
    let suggestion_resp = to_response::<ThreadSuggestNextPromptResponse>(suggestion_resp)?;
    assert_eq!(suggestion_resp.suggestion, None);
    assert_eq!(response_mock.requests().len(), /*expected*/ 0);

    let late_duplicate_cancel_req = mcp
        .send_raw_request(
            "thread/suggestNextPrompt",
            Some(json!({
                "threadId": thread.id,
                "cancellationToken": "next-prompt-suggestion-immediate-cancel",
                "cancel": true,
            })),
        )
        .await?;
    let late_duplicate_cancel_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(late_duplicate_cancel_req)),
    )
    .await??;
    let late_duplicate_cancel_resp =
        to_response::<ThreadSuggestNextPromptResponse>(late_duplicate_cancel_resp)?;
    assert_eq!(late_duplicate_cancel_resp.suggestion, None);

    let reused_token_req = mcp
        .send_raw_request(
            "thread/suggestNextPrompt",
            Some(json!({
                "threadId": thread.id,
                "cancellationToken": "next-prompt-suggestion-immediate-cancel",
            })),
        )
        .await?;
    let reused_token_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(reused_token_req)),
    )
    .await??;
    let reused_token_resp = to_response::<ThreadSuggestNextPromptResponse>(reused_token_resp)?;
    assert_eq!(
        reused_token_resp.suggestion.as_deref(),
        Some("run the tests")
    );
    assert_eq!(response_mock.requests().len(), /*expected*/ 1);

    Ok(())
}

#[tokio::test]
async fn thread_suggest_next_prompt_late_cancel_does_not_suppress_reused_token() -> Result<()> {
    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_assistant_message("msg-1", "run the tests"),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-2"),
                responses::ev_assistant_message("msg-2", "commit changes"),
                responses::ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;
    inject_suggestion_history(&mut mcp, &thread.id).await?;

    let first_suggestion_req = mcp
        .send_raw_request(
            "thread/suggestNextPrompt",
            Some(json!({
                "threadId": thread.id,
                "cancellationToken": "next-prompt-suggestion-late-cancel",
            })),
        )
        .await?;
    let first_suggestion_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(first_suggestion_req)),
    )
    .await??;
    let first_suggestion_resp =
        to_response::<ThreadSuggestNextPromptResponse>(first_suggestion_resp)?;
    assert_eq!(
        first_suggestion_resp.suggestion.as_deref(),
        Some("run the tests")
    );

    let cancel_req = mcp
        .send_raw_request(
            "thread/suggestNextPrompt",
            Some(json!({
                "threadId": thread.id,
                "cancellationToken": "next-prompt-suggestion-late-cancel",
                "cancel": true,
            })),
        )
        .await?;
    let cancel_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(cancel_req)),
    )
    .await??;
    let cancel_resp = to_response::<ThreadSuggestNextPromptResponse>(cancel_resp)?;
    assert_eq!(cancel_resp.suggestion, None);

    let second_suggestion_req = mcp
        .send_raw_request(
            "thread/suggestNextPrompt",
            Some(json!({
                "threadId": thread.id,
                "cancellationToken": "next-prompt-suggestion-late-cancel",
            })),
        )
        .await?;
    let second_suggestion_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(second_suggestion_req)),
    )
    .await??;
    let second_suggestion_resp =
        to_response::<ThreadSuggestNextPromptResponse>(second_suggestion_resp)?;
    assert_eq!(
        second_suggestion_resp.suggestion.as_deref(),
        Some("commit changes")
    );
    assert_eq!(response_mock.requests().len(), /*expected*/ 2);

    Ok(())
}

async fn inject_suggestion_history(mcp: &mut McpProcess, thread_id: &str) -> Result<()> {
    let items = vec![
        message("user", "fix the bug"),
        message("assistant", "I fixed it"),
        message("user", "continue"),
        message("assistant", "The change is ready"),
    ];
    let inject_req = mcp
        .send_thread_inject_items_request(ThreadInjectItemsParams {
            thread_id: thread_id.to_string(),
            items: items
                .into_iter()
                .map(serde_json::to_value)
                .collect::<serde_json::Result<Vec<_>>>()?,
        })
        .await?;
    let inject_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(inject_req)),
    )
    .await??;
    let _response: ThreadInjectItemsResponse =
        to_response::<ThreadInjectItemsResponse>(inject_resp)?;
    Ok(())
}

async fn wait_for_request_count(
    response_mock: &core_test_support::responses::ResponseMock,
    expected: usize,
) -> Result<()> {
    timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            if response_mock.requests().len() >= expected {
                return;
            }
            sleep(std::time::Duration::from_millis(/*millis*/ 10)).await;
        }
    })
    .await?;
    Ok(())
}

fn message(role: &str, text: &str) -> ResponseItem {
    let content = match role {
        "assistant" => ContentItem::OutputText {
            text: text.to_string(),
        },
        _ => ContentItem::InputText {
            text: text.to_string(),
        },
    };
    ResponseItem::Message {
        id: None,
        role: role.to_string(),
        content: vec![content],
        phase: None,
    }
}

fn create_config_toml(codex_home: &Path, server_uri: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
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
"#
        ),
    )
}
