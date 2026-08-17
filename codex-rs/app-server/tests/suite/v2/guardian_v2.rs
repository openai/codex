use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_fake_rollout;
use app_test_support::rollout_path;
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::extract::ws::Message;
use axum::extract::ws::WebSocketUpgrade;
use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::get;
use codex_app_server_protocol::ApprovalsReviewer;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::ItemGuardianApprovalReviewStartedNotification;
use codex_app_server_protocol::ThreadForkParams;
use codex_app_server_protocol::ThreadForkResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use codex_features::Feature;
use codex_protocol::security_risk::SecurityRiskScore;
use codex_rollout::RolloutItem;
use codex_rollout::append_rollout_item_to_path;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::Notify;
use tokio::time::timeout;

use super::mcp_tool::TEST_SERVER_NAME;
use super::mcp_tool::TEST_TOOL_NAME;
use super::mcp_tool::start_mcp_server;

const TIMEOUT: Duration = Duration::from_secs(30);
const USER_CONTEXT: &str = "The user authorized reading the existing project files.";

#[derive(Default)]
struct MockResponsesState {
    parent_requests: AtomicUsize,
    guardian_reviews: AtomicUsize,
    luna_requests: Mutex<Vec<Value>>,
    allow_luna: Notify,
    allow_guardian_review: Notify,
    luna_score: f64,
}

#[derive(Clone, Copy)]
enum GuardianRisk {
    Low,
    High,
}

#[derive(Clone, Copy)]
enum ThreadLifecycle {
    New,
    Resume,
    Fork,
}

async fn parent_response(
    State(state): State<Arc<MockResponsesState>>,
    Json(request): Json<Value>,
) -> impl IntoResponse {
    let events = if request
        .pointer("/client_metadata/x-openai-subagent")
        .and_then(Value::as_str)
        == Some("guardian")
    {
        let review_number = state.guardian_reviews.fetch_add(1, Ordering::SeqCst);
        if review_number == 0 {
            state.allow_guardian_review.notified().await;
        }
        vec![
            responses::ev_response_created("guardian-review"),
            responses::ev_assistant_message(
                "guardian-assessment",
                &json!({
                    "risk_level": "low",
                    "user_authorization": "high",
                    "outcome": "allow",
                    "rationale": "The requested command is safe.",
                })
                .to_string(),
            ),
            responses::ev_completed("guardian-review"),
        ]
    } else {
        let request_number = state.parent_requests.fetch_add(1, Ordering::SeqCst);
        if request_number < 2 {
            let call_id = format!("guardian-action-{request_number}");
            let arguments = json!({ "message": format!("guardian-{request_number}") }).to_string();
            vec![
                responses::ev_response_created(&call_id),
                responses::ev_function_call_with_namespace(
                    &call_id,
                    &format!("mcp__{TEST_SERVER_NAME}"),
                    TEST_TOOL_NAME,
                    &arguments,
                ),
                responses::ev_completed(&call_id),
            ]
        } else {
            vec![
                responses::ev_response_created("guardian-complete"),
                responses::ev_assistant_message("guardian-message", "done"),
                responses::ev_completed("guardian-complete"),
            ]
        }
    };

    (
        [(header::CONTENT_TYPE, "text/event-stream")],
        responses::sse(events),
    )
}

async fn luna_websocket(
    State(state): State<Arc<MockResponsesState>>,
    websocket: WebSocketUpgrade,
) -> impl IntoResponse {
    websocket.on_upgrade(move |mut socket| async move {
        while let Some(Ok(message)) = socket.recv().await {
            let Message::Text(text) = message else {
                continue;
            };
            let request: Value = serde_json::from_str(&text).expect("valid Luna request");
            state
                .luna_requests
                .lock()
                .expect("Luna request lock should not be poisoned")
                .push(request);
            state.allow_luna.notified().await;
            let score = json!({ "scores": { "action_risk": state.luna_score } }).to_string();
            for event in [
                responses::ev_response_created("luna-score"),
                responses::ev_assistant_message("luna-score-message", &score),
                responses::ev_completed("luna-score"),
            ] {
                if socket
                    .send(Message::Text(event.to_string().into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }
    })
}

async fn guardian_v2_routes_tool_approvals(
    risk: GuardianRisk,
    lifecycle: ThreadLifecycle,
) -> Result<()> {
    let (luna_score, expected_guardian_reviews) = match (risk, lifecycle) {
        (GuardianRisk::Low, ThreadLifecycle::New) => (0.25, 1),
        (GuardianRisk::Low, ThreadLifecycle::Resume | ThreadLifecycle::Fork) => (0.25, 0),
        (GuardianRisk::High, _) => (0.95, 2),
    };
    let responses_state = Arc::new(MockResponsesState {
        luna_score,
        ..Default::default()
    });
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let responses_url = format!("http://{}", listener.local_addr()?);
    let router = Router::new()
        .route("/v1/responses", get(luna_websocket).post(parent_response))
        .with_state(Arc::clone(&responses_state));
    let responses_server = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    let (mcp_server_url, mcp_server_handle) = start_mcp_server().await?;

    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&responses_url)
        .with_provider_config("supports_websockets = false")
        .with_approval_policy("on-request")
        .with_root_config("approvals_reviewer = \"auto_review\"")
        .with_extra_config(&format!(
            "[mcp_servers.{TEST_SERVER_NAME}]\nurl = \"{mcp_server_url}/mcp\"\ndefault_tools_approval_mode = \"prompt\""
        ))
        .enable_feature(Feature::GuardianV2)
        .enable_feature(Feature::GuardianApproval)
        .write(codex_home.path())?;
    let original_thread_id = match lifecycle {
        ThreadLifecycle::New => None,
        ThreadLifecycle::Resume | ThreadLifecycle::Fork => {
            let thread_id = create_fake_rollout(
                codex_home.path(),
                "2025-01-05T12-00-00",
                "2025-01-05T12:00:00Z",
                USER_CONTEXT,
                Some("mock_provider"),
                /*git_info*/ None,
            )?;
            let original_rollout =
                rollout_path(codex_home.path(), "2025-01-05T12-00-00", &thread_id);
            for action_risk in [0.95, 0.1] {
                append_rollout_item_to_path(
                    &original_rollout,
                    &RolloutItem::SecurityRiskScore(SecurityRiskScore {
                        scores: BTreeMap::from([("action_risk".to_owned(), action_risk)]),
                        sampled_at: None,
                    }),
                )
                .await?;
            }
            Some(thread_id)
        }
    };
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(TIMEOUT)
        .await?;
    let thread = match lifecycle {
        ThreadLifecycle::New => {
            app_server
                .start_thread(ThreadStartParams {
                    approval_policy: Some(AskForApproval::OnRequest),
                    approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
                    ..Default::default()
                })
                .await?
                .thread
        }
        ThreadLifecycle::Resume => {
            let original_thread_id = original_thread_id.expect("resumed thread should exist");
            let request_id = app_server
                .send_thread_resume_request(ThreadResumeParams {
                    thread_id: original_thread_id.clone(),
                    approval_policy: Some(AskForApproval::OnRequest),
                    approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
                    ..Default::default()
                })
                .await?;
            let resumed: ThreadResumeResponse =
                timeout(TIMEOUT, app_server.read_response(request_id)).await??;
            assert_eq!(resumed.thread.id, original_thread_id);
            resumed.thread
        }
        ThreadLifecycle::Fork => {
            let original_thread_id = original_thread_id.expect("forked thread should exist");
            let request_id = app_server
                .send_thread_fork_request(ThreadForkParams {
                    thread_id: original_thread_id.clone(),
                    approval_policy: Some(AskForApproval::OnRequest),
                    approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
                    ..Default::default()
                })
                .await?;
            let forked: ThreadForkResponse =
                timeout(TIMEOUT, app_server.read_response(request_id)).await??;
            assert_ne!(forked.thread.id, original_thread_id);
            forked.thread
        }
    };
    let thread_id = thread.id;
    let rollout = thread.path.expect("thread should be persisted");
    let turn_request_id = app_server
        .send_turn_start_request(TurnStartParams {
            thread_id: thread_id.clone(),
            input: vec![UserInput::Text {
                text: USER_CONTEXT.to_owned(),
                text_elements: Vec::new(),
            }],
            approval_policy: Some(AskForApproval::OnRequest),
            approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse =
        timeout(TIMEOUT, app_server.read_response(turn_request_id)).await??;
    if matches!(lifecycle, ThreadLifecycle::New) {
        let review_started: ItemGuardianApprovalReviewStartedNotification = timeout(
            TIMEOUT,
            app_server.read_notification("item/autoApprovalReview/started"),
        )
        .await??;
        assert_eq!(review_started.thread_id, thread_id);
    }

    let luna_request = timeout(TIMEOUT, async {
        loop {
            if let Some(request) = responses_state
                .luna_requests
                .lock()
                .expect("Luna request lock should not be poisoned")
                .first()
                .cloned()
            {
                return request;
            }
            tokio::task::yield_now().await;
        }
    })
    .await?;
    assert_eq!(
        luna_request["prompt_cache_key"],
        format!("guardian-v2:{thread_id}")
    );
    assert!(
        luna_request["input"]
            .as_array()
            .expect("Luna input should be an array")
            .iter()
            .any(|item| {
                item["content"].as_array().is_some_and(|content| {
                    content.iter().any(|entry| {
                        entry["text"]
                            .as_str()
                            .is_some_and(|text| text.contains(USER_CONTEXT))
                    })
                })
            })
    );
    if matches!(lifecycle, ThreadLifecycle::Resume | ThreadLifecycle::Fork) {
        timeout(TIMEOUT, async {
            while responses_state.parent_requests.load(Ordering::SeqCst) < 3 {
                tokio::task::yield_now().await;
            }
        })
        .await?;
        assert_eq!(responses_state.guardian_reviews.load(Ordering::SeqCst), 0);
    }

    responses_state.allow_luna.notify_one();
    timeout(TIMEOUT, async {
        loop {
            if tokio::fs::read_to_string(&rollout)
                .await?
                .lines()
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .any(|line| {
                    line["type"] == "security_risk_score"
                        && line["payload"]["scores"]["action_risk"] == json!(luna_score)
                })
            {
                return Ok::<(), std::io::Error>(());
            }
            tokio::task::yield_now().await;
        }
    })
    .await??;
    responses_state.allow_guardian_review.notify_one();
    responses_state.allow_luna.notify_one();
    timeout(
        TIMEOUT,
        app_server.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    assert_eq!(
        responses_state.guardian_reviews.load(Ordering::SeqCst),
        expected_guardian_reviews
    );

    mcp_server_handle.abort();
    responses_server.abort();
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_v2_low_risk_actions_skip_subsequent_reviews() -> Result<()> {
    skip_if_no_network!(Ok(()));
    guardian_v2_routes_tool_approvals(GuardianRisk::Low, ThreadLifecycle::New).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_v2_high_risk_actions_require_full_reviews() -> Result<()> {
    skip_if_no_network!(Ok(()));
    guardian_v2_routes_tool_approvals(GuardianRisk::High, ThreadLifecycle::New).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resumed_thread_inherits_latest_guardian_score() -> Result<()> {
    skip_if_no_network!(Ok(()));
    guardian_v2_routes_tool_approvals(GuardianRisk::Low, ThreadLifecycle::Resume).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forked_thread_inherits_latest_guardian_score() -> Result<()> {
    skip_if_no_network!(Ok(()));
    guardian_v2_routes_tool_approvals(GuardianRisk::Low, ThreadLifecycle::Fork).await
}
