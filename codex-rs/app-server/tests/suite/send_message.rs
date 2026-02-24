use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_fake_rollout;
use app_test_support::rollout_path;
use app_test_support::to_response;
use codex_app_server_protocol::AddConversationListenerParams;
use codex_app_server_protocol::AddConversationSubscriptionResponse;
use codex_app_server_protocol::InputItem;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::NewConversationParams;
use codex_app_server_protocol::NewConversationResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ResumeConversationParams;
use codex_app_server_protocol::ResumeConversationResponse;
use codex_app_server_protocol::SendUserMessageParams;
use codex_app_server_protocol::SendUserMessageResponse;
use codex_execpolicy::Policy;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::models::ContentItem;
use codex_protocol::models::DeveloperInstructions;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::RawResponseItemEvent;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::TurnContextItem;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn test_send_message_success() -> Result<()> {
    // Spin up a mock responses server that immediately ends the Codex turn.
    // Two Codex turns hit the mock model (session start + send-user-message). Provide two SSE responses.
    let server = responses::start_mock_server().await;
    let body1 = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "Done"),
        responses::ev_completed("resp-1"),
    ]);
    let body2 = responses::sse(vec![
        responses::ev_response_created("resp-2"),
        responses::ev_assistant_message("msg-2", "Done"),
        responses::ev_completed("resp-2"),
    ]);
    let _response_mock1 = responses::mount_sse_once(&server, body1).await;
    let _response_mock2 = responses::mount_sse_once(&server, body2).await;

    // Create a temporary Codex home with config pointing at the mock server.
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    // Start MCP server process and initialize.
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    // Start a conversation using the new wire API.
    let new_conv_id = mcp
        .send_new_conversation_request(NewConversationParams {
            ..Default::default()
        })
        .await?;
    let new_conv_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(new_conv_id)),
    )
    .await??;
    let NewConversationResponse {
        conversation_id, ..
    } = to_response::<_>(new_conv_resp)?;

    // 2) addConversationListener
    let add_listener_id = mcp
        .send_add_conversation_listener_request(AddConversationListenerParams {
            conversation_id,
            experimental_raw_events: false,
        })
        .await?;
    let add_listener_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(add_listener_id)),
    )
    .await??;
    let AddConversationSubscriptionResponse { subscription_id: _ } =
        to_response::<_>(add_listener_resp)?;

    // Now exercise sendUserMessage twice.
    send_message("Hello", conversation_id, &mut mcp).await?;
    send_message("Hello again", conversation_id, &mut mcp).await?;
    Ok(())
}

#[expect(clippy::expect_used)]
async fn send_message(
    message: &str,
    conversation_id: ThreadId,
    mcp: &mut McpProcess,
) -> Result<()> {
    // Now exercise sendUserMessage.
    let send_id = mcp
        .send_send_user_message_request(SendUserMessageParams {
            conversation_id,
            items: vec![InputItem::Text {
                text: message.to_string(),
                text_elements: Vec::new(),
            }],
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(send_id)),
    )
    .await??;

    let _ok: SendUserMessageResponse = to_response::<SendUserMessageResponse>(response)?;

    // Verify the task_finished notification is received.
    // Note this also ensures that the final request to the server was made.
    let task_finished_notification: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await??;
    let serde_json::Value::Object(map) = task_finished_notification
        .params
        .expect("notification should have params")
    else {
        panic!("task_finished_notification should have params");
    };
    assert_eq!(
        map.get("conversationId")
            .expect("should have conversationId"),
        &serde_json::Value::String(conversation_id.to_string())
    );

    let raw_attempt = tokio::time::timeout(
        std::time::Duration::from_millis(200),
        mcp.read_stream_until_notification_message("codex/event/raw_response_item"),
    )
    .await;
    assert!(
        raw_attempt.is_err(),
        "unexpected raw item notification when not opted in"
    );
    Ok(())
}

#[tokio::test]
async fn test_send_message_raw_notifications_opt_in() -> Result<()> {
    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "Done"),
        responses::ev_completed("resp-1"),
    ]);
    let _response_mock = responses::mount_sse_once(&server, body).await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let new_conv_id = mcp
        .send_new_conversation_request(NewConversationParams {
            developer_instructions: Some("Use the test harness tools.".to_string()),
            ..Default::default()
        })
        .await?;
    let new_conv_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(new_conv_id)),
    )
    .await??;
    let NewConversationResponse {
        conversation_id, ..
    } = to_response::<_>(new_conv_resp)?;

    let add_listener_id = mcp
        .send_add_conversation_listener_request(AddConversationListenerParams {
            conversation_id,
            experimental_raw_events: true,
        })
        .await?;
    let add_listener_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(add_listener_id)),
    )
    .await??;
    let AddConversationSubscriptionResponse { subscription_id: _ } =
        to_response::<_>(add_listener_resp)?;

    let send_id = mcp
        .send_send_user_message_request(SendUserMessageParams {
            conversation_id,
            items: vec![InputItem::Text {
                text: "Hello".to_string(),
                text_elements: Vec::new(),
            }],
        })
        .await?;

    let permissions = read_raw_response_item(&mut mcp, conversation_id).await;
    assert_permissions_message(&permissions);

    let developer = read_raw_response_item(&mut mcp, conversation_id).await;
    assert_developer_message(&developer, "Use the test harness tools.");

    let instructions = read_raw_response_item(&mut mcp, conversation_id).await;
    assert_instructions_message(&instructions);

    let environment = read_raw_response_item(&mut mcp, conversation_id).await;
    assert_environment_message(&environment);

    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(send_id)),
    )
    .await??;
    let _ok: SendUserMessageResponse = to_response::<SendUserMessageResponse>(response)?;

    let user_message = read_raw_response_item(&mut mcp, conversation_id).await;
    assert_user_message(&user_message, "Hello");

    let assistant_message = read_raw_response_item(&mut mcp, conversation_id).await;
    assert_assistant_message(&assistant_message, "Done");

    let _ = tokio::time::timeout(
        std::time::Duration::from_millis(250),
        mcp.read_stream_until_notification_message("codex/event/task_complete"),
    )
    .await;

    Ok(())
}

#[tokio::test]
async fn test_send_message_session_not_found() -> Result<()> {
    // Start MCP without creating a Codex session
    let codex_home = TempDir::new()?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let unknown = ThreadId::new();
    let req_id = mcp
        .send_send_user_message_request(SendUserMessageParams {
            conversation_id: unknown,
            items: vec![InputItem::Text {
                text: "ping".to_string(),
                text_elements: Vec::new(),
            }],
        })
        .await?;

    // Expect an error response for unknown conversation.
    let err = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(req_id)),
    )
    .await??;
    assert_eq!(err.id, RequestId::Integer(req_id));
    Ok(())
}

#[tokio::test]
async fn resume_with_model_mismatch_appends_model_switch_once() -> Result<()> {
    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_assistant_message("msg-1", "Done"),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-2"),
                responses::ev_assistant_message("msg-2", "Done again"),
                responses::ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let filename_ts = "2025-01-02T12-00-00";
    let meta_rfc3339 = "2025-01-02T12:00:00Z";
    let preview = "Resume me";
    let conversation_id = create_fake_rollout(
        codex_home.path(),
        filename_ts,
        meta_rfc3339,
        preview,
        Some("mock_provider"),
        None,
    )?;
    let rollout_path = rollout_path(codex_home.path(), filename_ts, &conversation_id);
    append_rollout_turn_context(&rollout_path, meta_rfc3339, "previous-model")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let resume_id = mcp
        .send_resume_conversation_request(ResumeConversationParams {
            path: Some(rollout_path.clone()),
            conversation_id: None,
            history: None,
            overrides: Some(NewConversationParams {
                model: Some("gpt-5.2-codex".to_string()),
                ..Default::default()
            }),
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("sessionConfigured"),
    )
    .await??;
    let resume_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(resume_id)),
    )
    .await??;
    let ResumeConversationResponse {
        conversation_id, ..
    } = to_response::<ResumeConversationResponse>(resume_resp)?;

    let add_listener_id = mcp
        .send_add_conversation_listener_request(AddConversationListenerParams {
            conversation_id,
            experimental_raw_events: false,
        })
        .await?;
    let add_listener_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(add_listener_id)),
    )
    .await??;
    let AddConversationSubscriptionResponse { subscription_id: _ } =
        to_response::<_>(add_listener_resp)?;

    send_message("hello after resume", conversation_id, &mut mcp).await?;
    send_message("second turn", conversation_id, &mut mcp).await?;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2, "expected two model requests");

    let first_developer_texts = requests[0].message_input_texts("developer");
    let first_model_switch_count = first_developer_texts
        .iter()
        .filter(|text| text.contains("<model_switch>"))
        .count();
    assert!(
        first_model_switch_count >= 1,
        "expected model switch message on first post-resume turn, got {first_developer_texts:?}"
    );

    let second_developer_texts = requests[1].message_input_texts("developer");
    let second_model_switch_count = second_developer_texts
        .iter()
        .filter(|text| text.contains("<model_switch>"))
        .count();
    assert_eq!(
        second_model_switch_count, 1,
        "did not expect duplicate model switch message on second post-resume turn, got {second_developer_texts:?}"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn create_config_toml(codex_home: &Path, server_uri: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "danger-full-access"

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

#[expect(clippy::expect_used)]
async fn read_raw_response_item(mcp: &mut McpProcess, conversation_id: ThreadId) -> ResponseItem {
    // TODO: Switch to rawResponseItem/completed once we migrate to app server v2 in codex web.
    loop {
        let raw_notification: JSONRPCNotification = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_notification_message("codex/event/raw_response_item"),
        )
        .await
        .expect("codex/event/raw_response_item notification timeout")
        .expect("codex/event/raw_response_item notification resp");

        let serde_json::Value::Object(params) = raw_notification
            .params
            .expect("codex/event/raw_response_item should have params")
        else {
            panic!("codex/event/raw_response_item should have params");
        };

        let conversation_id_value = params
            .get("conversationId")
            .and_then(|value| value.as_str())
            .expect("raw response item should include conversationId");

        assert_eq!(
            conversation_id_value,
            conversation_id.to_string(),
            "raw response item conversation mismatch"
        );

        let msg_value = params
            .get("msg")
            .cloned()
            .expect("raw response item should include msg payload");

        // Ghost snapshots are produced concurrently and may arrive before the model reply.
        let event: RawResponseItemEvent =
            serde_json::from_value(msg_value).expect("deserialize raw response item");
        if !matches!(event.item, ResponseItem::GhostSnapshot { .. }) {
            return event.item;
        }
    }
}

fn assert_instructions_message(item: &ResponseItem) {
    match item {
        ResponseItem::Message { role, content, .. } => {
            assert_eq!(role, "user");
            let texts = content_texts(content);
            let is_instructions = texts
                .iter()
                .any(|text| text.starts_with("# AGENTS.md instructions for "));
            assert!(
                is_instructions,
                "expected instructions message, got {texts:?}"
            );
        }
        other => panic!("expected instructions message, got {other:?}"),
    }
}

fn assert_permissions_message(item: &ResponseItem) {
    match item {
        ResponseItem::Message { role, content, .. } => {
            assert_eq!(role, "developer");
            let texts = content_texts(content);
            let expected = DeveloperInstructions::from_policy(
                &SandboxPolicy::DangerFullAccess,
                AskForApproval::Never,
                &Policy::empty(),
                &PathBuf::from("/tmp"),
                false,
            )
            .into_text();
            assert_eq!(
                texts,
                vec![expected.as_str()],
                "expected permissions developer message, got {texts:?}"
            );
        }
        other => panic!("expected permissions message, got {other:?}"),
    }
}

fn assert_developer_message(item: &ResponseItem, expected_text: &str) {
    match item {
        ResponseItem::Message { role, content, .. } => {
            assert_eq!(role, "developer");
            let texts = content_texts(content);
            assert_eq!(
                texts,
                vec![expected_text],
                "expected developer instructions message, got {texts:?}"
            );
        }
        other => panic!("expected developer instructions message, got {other:?}"),
    }
}

fn assert_environment_message(item: &ResponseItem) {
    match item {
        ResponseItem::Message { role, content, .. } => {
            assert_eq!(role, "user");
            let texts = content_texts(content);
            assert!(
                texts
                    .iter()
                    .any(|text| text.contains("<environment_context>")),
                "expected environment context message, got {texts:?}"
            );
        }
        other => panic!("expected environment message, got {other:?}"),
    }
}

fn assert_user_message(item: &ResponseItem, expected_text: &str) {
    match item {
        ResponseItem::Message { role, content, .. } => {
            assert_eq!(role, "user");
            let texts = content_texts(content);
            assert_eq!(texts, vec![expected_text]);
        }
        other => panic!("expected user message, got {other:?}"),
    }
}

fn assert_assistant_message(item: &ResponseItem, expected_text: &str) {
    match item {
        ResponseItem::Message { role, content, .. } => {
            assert_eq!(role, "assistant");
            let texts = content_texts(content);
            assert_eq!(texts, vec![expected_text]);
        }
        other => panic!("expected assistant message, got {other:?}"),
    }
}

fn content_texts(content: &[ContentItem]) -> Vec<&str> {
    content
        .iter()
        .filter_map(|item| match item {
            ContentItem::InputText { text, .. } | ContentItem::OutputText { text } => {
                Some(text.as_str())
            }
            _ => None,
        })
        .collect()
}

fn append_rollout_turn_context(path: &Path, timestamp: &str, model: &str) -> std::io::Result<()> {
    let line = RolloutLine {
        timestamp: timestamp.to_string(),
        item: RolloutItem::TurnContext(TurnContextItem {
            turn_id: None,
            cwd: PathBuf::from("/"),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            network: None,
            model: model.to_string(),
            personality: None,
            collaboration_mode: None,
            effort: None,
            summary: ReasoningSummary::Auto,
            user_instructions: None,
            developer_instructions: None,
            final_output_json_schema: None,
            truncation_policy: None,
        }),
    };
    let serialized = serde_json::to_string(&line).map_err(std::io::Error::other)?;
    std::fs::OpenOptions::new()
        .append(true)
        .open(path)?
        .write_all(format!("{serialized}\n").as_bytes())
}
