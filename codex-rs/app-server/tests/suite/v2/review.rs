use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::create_shell_command_sse_response;
use app_test_support::to_response;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ReviewDelivery;
use codex_app_server_protocol::ReviewStartParams;
use codex_app_server_protocol::ReviewStartResponse;
use codex_app_server_protocol::ReviewTarget;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput as V2UserInput;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const INVALID_REQUEST_ERROR_CODE: i64 = -32600;

#[tokio::test]
async fn review_start_runs_review_turn_and_emits_code_review_item() -> Result<()> {
    let review_payload = json!({
        "findings": [
            {
                "title": "Prefer Stylize helpers",
                "body": "Use .dim()/.bold() chaining instead of manual Style.",
                "confidence_score": 0.9,
                "priority": 1,
                "code_location": {
                    "absolute_file_path": "/tmp/file.rs",
                    "line_range": {"start": 10, "end": 20}
                }
            }
        ],
        "overall_correctness": "good",
        "overall_explanation": "Looks solid overall with minor polish suggested.",
        "overall_confidence_score": 0.75
    })
    .to_string();
    let server = create_mock_responses_server_repeating_assistant(&review_payload).await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
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

    // Confirm we see the EnteredReviewMode marker on the main thread.
    let mut saw_entered_review_mode = false;
    for _ in 0..10 {
        let item_started: JSONRPCNotification = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_notification_message("item/started"),
        )
        .await??;
        let started: ItemStartedNotification =
            serde_json::from_value(item_started.params.expect("params must be present"))?;
        match started.item {
            ThreadItem::EnteredReviewMode { id, review } => {
                assert_eq!(id, turn_id);
                assert_eq!(review, "commit 1234567: Tidy UI colors");
                saw_entered_review_mode = true;
                break;
            }
            _ => continue,
        }
    }
    assert!(
        saw_entered_review_mode,
        "did not observe enteredReviewMode item"
    );

    // Confirm we see the ExitedReviewMode marker (with review text)
    // on the same turn. Ignore any other items the stream surfaces.
    let mut review_body: Option<String> = None;
    for _ in 0..10 {
        let review_notif: JSONRPCNotification = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_notification_message("item/completed"),
        )
        .await??;
        let completed: ItemCompletedNotification =
            serde_json::from_value(review_notif.params.expect("params must be present"))?;
        match completed.item {
            ThreadItem::ExitedReviewMode { id, review } => {
                assert_eq!(id, turn_id);
                review_body = Some(review);
                break;
            }
            _ => continue,
        }
    }

    let review = review_body.expect("did not observe a code review item");
    assert!(review.contains("Prefer Stylize helpers"));
    assert!(review.contains("/tmp/file.rs:10-20"));

    Ok(())
}

#[tokio::test]
#[ignore = "TODO(owenlin0): flaky"]
async fn review_start_exec_approval_item_id_matches_command_execution_item() -> Result<()> {
    let responses = vec![
        create_shell_command_sse_response(
            vec![
                "git".to_string(),
                "rev-parse".to_string(),
                "HEAD".to_string(),
            ],
            None,
            Some(5000),
            "review-call-1",
        )?,
        create_final_assistant_message_sse_response("done")?,
    ];
    let server = create_mock_responses_server_sequence(responses).await;

    let codex_home = TempDir::new()?;
    create_config_toml_with_approval_policy(codex_home.path(), &server.uri(), "untrusted")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_id = start_default_thread(&mut mcp).await?;

    let review_req = mcp
        .send_review_start_request(ReviewStartParams {
            thread_id,
            delivery: Some(ReviewDelivery::Inline),
            target: ReviewTarget::Commit {
                sha: "1234567deadbeef".to_string(),
                title: Some("Check review approvals".to_string()),
            },
        })
        .await?;
    let review_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(review_req)),
    )
    .await??;
    let ReviewStartResponse { turn, .. } = to_response::<ReviewStartResponse>(review_resp)?;
    let turn_id = turn.id.clone();

    let server_req = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::CommandExecutionRequestApproval { request_id, params } = server_req else {
        panic!("expected CommandExecutionRequestApproval request");
    };
    assert_eq!(params.item_id, "review-call-1");
    assert_eq!(params.turn_id, turn_id);

    let mut command_item_id = None;
    for _ in 0..10 {
        let item_started: JSONRPCNotification = timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_notification_message("item/started"),
        )
        .await??;
        let started: ItemStartedNotification =
            serde_json::from_value(item_started.params.expect("params must be present"))?;
        if let ThreadItem::CommandExecution { id, .. } = started.item {
            command_item_id = Some(id);
            break;
        }
    }
    let command_item_id = command_item_id.expect("did not observe command execution item");
    assert_eq!(command_item_id, params.item_id);

    mcp.send_response(
        request_id,
        serde_json::json!({ "decision": codex_protocol::protocol::ReviewDecision::Approved }),
    )
    .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    Ok(())
}

#[tokio::test]
async fn review_start_rejects_empty_base_branch() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
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
    let review_payload = json!({
        "findings": [],
        "overall_correctness": "ok",
        "overall_explanation": "detached review",
        "overall_confidence_score": 0.5
    })
    .to_string();
    let server = create_mock_responses_server_repeating_assistant(&review_payload).await;

    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
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
    assert_ne!(
        review_thread_id, thread_id,
        "detached review should run on a different thread"
    );

    Ok(())
}

#[tokio::test]
async fn review_start_rejects_empty_commit_sha() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
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

    let mut mcp = McpProcess::new(codex_home.path()).await?;
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

async fn start_default_thread(mcp: &mut McpProcess) -> Result<String> {
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
    Ok(thread.id)
}

async fn materialize_thread_rollout(mcp: &mut McpProcess, thread_id: &str) -> Result<()> {
    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread_id.to_string(),
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
    create_config_toml_with_approval_policy(codex_home, server_uri, "never")
}

fn create_config_toml_with_approval_policy(
    codex_home: &std::path::Path,
    server_uri: &str,
    approval_policy: &str,
) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "{approval_policy}"
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
