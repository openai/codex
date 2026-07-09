use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ReviewDelivery;
use codex_app_server_protocol::ReviewStartParams;
use codex_app_server_protocol::ReviewStartResponse;
use codex_app_server_protocol::ReviewTarget;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadStartedNotification;
use codex_app_server_protocol::ThreadStatusChangedNotification;
use codex_app_server_protocol::TurnItemsView;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput as V2UserInput;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const INVALID_REQUEST_ERROR_CODE: i64 = -32600;

#[tokio::test]
async fn review_start_runs_regular_turn_with_review_agent_skill() -> Result<()> {
    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_assistant_message("msg-1", "Done"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_id = start_default_thread(&mut mcp).await?;

    let review_req = mcp
        .send_review_start_request(ReviewStartParams {
            thread_id: thread_id.clone(),
            delivery: Some(ReviewDelivery::Inline),
            target: ReviewTarget::Commit {
                sha: "1234567deadbeef".to_string(),
                title: Some("Tidy UI colors".to_string()),
            },
        })
        .await?;
    let review_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(review_req)),
    )
    .await??;
    let ReviewStartResponse {
        turn,
        review_thread_id,
    } = to_response::<ReviewStartResponse>(review_resp)?;
    assert_eq!(review_thread_id, thread_id.clone());
    let turn_id = turn.id.clone();
    assert_eq!(turn.status, TurnStatus::InProgress);
    assert_eq!(turn.items_view, TurnItemsView::NotLoaded);
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let request = response_mock.single_request();
    let user_messages = request.message_input_texts("user");
    let prompt = user_messages
        .iter()
        .find(|text| text.contains("Spawn one sub-agent"))
        .expect("review delegation prompt should be model-visible");
    assert!(prompt.contains("$review-agent"));
    assert!(prompt.contains("1234567deadbeef"));
    assert!(prompt.contains("Tidy UI colors"));

    let skill_fragments = user_messages
        .iter()
        .filter(|text| text.starts_with("<skill>"))
        .collect::<Vec<_>>();
    assert_eq!(skill_fragments.len(), 1);
    assert!(skill_fragments[0].contains("<name>review-agent</name>"));
    assert!(skill_fragments[0].contains("Do not modify files"));

    assert_eq!(
        turn.items,
        vec![ThreadItem::UserMessage {
            id: turn_id,
            client_id: None,
            content: vec![V2UserInput::Text {
                text: prompt.to_string(),
                text_elements: Vec::new(),
            }],
        }]
    );

    Ok(())
}

#[tokio::test]
async fn review_start_rejects_empty_base_branch() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    let thread_id = start_default_thread(&mut mcp).await?;

    let request_id = mcp
        .send_review_start_request(ReviewStartParams {
            thread_id,
            delivery: Some(ReviewDelivery::Inline),
            target: ReviewTarget::BaseBranch {
                branch: "   ".to_string(),
            },
        })
        .await?;
    let error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert!(
        error.error.message.contains("branch must not be empty"),
        "unexpected message: {}",
        error.error.message
    );

    Ok(())
}

#[cfg_attr(target_os = "windows", ignore = "flaky on windows CI")]
#[tokio::test]
async fn review_start_with_detached_delivery_returns_new_thread_id() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_id = start_default_thread(&mut mcp).await?;
    materialize_thread_rollout(&mut mcp, &thread_id).await?;

    let review_req = mcp
        .send_review_start_request(ReviewStartParams {
            thread_id: thread_id.clone(),
            delivery: Some(ReviewDelivery::Detached),
            target: ReviewTarget::Custom {
                instructions: "detached review".to_string(),
            },
        })
        .await?;
    let review_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(review_req)),
    )
    .await??;
    let ReviewStartResponse {
        turn,
        review_thread_id,
    } = to_response::<ReviewStartResponse>(review_resp)?;

    assert_eq!(turn.status, TurnStatus::InProgress);
    assert_eq!(turn.items_view, TurnItemsView::NotLoaded);
    let [ThreadItem::UserMessage { content, .. }] = turn.items.as_slice() else {
        panic!("expected synthesized review user message");
    };
    let [
        V2UserInput::Text {
            text,
            text_elements,
        },
    ] = content.as_slice()
    else {
        panic!("expected synthesized review text");
    };
    assert!(text.contains("Spawn one sub-agent"));
    assert!(text.contains("$review-agent"));
    assert!(text.ends_with("Review target:\ndetached review"));
    assert!(text_elements.is_empty());
    assert_ne!(
        review_thread_id, thread_id,
        "detached review should run on a different thread"
    );

    let deadline = tokio::time::Instant::now() + DEFAULT_READ_TIMEOUT;
    let notification = loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let message = timeout(remaining, mcp.read_next_message()).await??;
        let JSONRPCMessage::Notification(notification) = message else {
            continue;
        };
        if notification.method == "thread/status/changed" {
            let status_changed: ThreadStatusChangedNotification =
                serde_json::from_value(notification.params.expect("params must be present"))?;
            if status_changed.thread_id == review_thread_id {
                anyhow::bail!(
                    "detached review threads should be introduced without a preceding thread/status/changed"
                );
            }
            continue;
        }
        if notification.method == "thread/started" {
            break notification;
        }
    };
    let started: ThreadStartedNotification =
        serde_json::from_value(notification.params.expect("params must be present"))?;
    assert_eq!(started.thread.id, review_thread_id);
    assert_eq!(started.thread.session_id, review_thread_id);

    Ok(())
}

#[tokio::test]
async fn review_start_rejects_empty_commit_sha() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    let thread_id = start_default_thread(&mut mcp).await?;

    let request_id = mcp
        .send_review_start_request(ReviewStartParams {
            thread_id,
            delivery: Some(ReviewDelivery::Inline),
            target: ReviewTarget::Commit {
                sha: "\t".to_string(),
                title: None,
            },
        })
        .await?;
    let error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert!(
        error.error.message.contains("sha must not be empty"),
        "unexpected message: {}",
        error.error.message
    );

    Ok(())
}

#[tokio::test]
async fn review_start_rejects_empty_custom_instructions() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    let thread_id = start_default_thread(&mut mcp).await?;

    let request_id = mcp
        .send_review_start_request(ReviewStartParams {
            thread_id,
            delivery: Some(ReviewDelivery::Inline),
            target: ReviewTarget::Custom {
                instructions: "\n\n".to_string(),
            },
        })
        .await?;
    let error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert!(
        error
            .error
            .message
            .contains("instructions must not be empty"),
        "unexpected message: {}",
        error.error.message
    );

    Ok(())
}

async fn start_default_thread(mcp: &mut TestAppServer) -> Result<String> {
    let thread_req = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
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
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/started"),
    )
    .await??;
    Ok(thread.id)
}

async fn materialize_thread_rollout(mcp: &mut TestAppServer, thread_id: &str) -> Result<()> {
    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread_id.to_string(),
            client_user_message_id: None,
            input: vec![V2UserInput::Text {
                text: "materialize rollout".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    Ok(())
}

fn create_config_toml(codex_home: &std::path::Path, server_uri: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"

model_provider = "mock_provider"

[features]
shell_snapshot = false

[model_providers.mock_provider]
name = "Mock provider"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}
