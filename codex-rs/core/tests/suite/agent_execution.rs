use anyhow::Result;
use codex_core::CodexThread;
use codex_core::StartThreadOptions;
use codex_features::Feature;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_response_once_match;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::sse;
use core_test_support::responses::sse_response;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;
use tokio::time::Instant;
use tokio::time::sleep;

const SPAWN_PROMPT: &str = "spawn the worker";
const BLOCKED_PROMPT: &str = "try the blocked followup";
const ALLOWED_PROMPT: &str = "try the allowed followup";
const INITIAL_TASK: &str = "initial worker task";
const BLOCKED_TASK: &str = "blocked worker task";
const ALLOWED_TASK: &str = "allowed worker task";

fn body_contains(request: &wiremock::Request, text: &str) -> bool {
    serde_json::from_slice::<serde_json::Value>(&request.body)
        .is_ok_and(|body| body.to_string().contains(text))
}

async fn wait_for_status(
    thread: &CodexThread,
    expected: impl Fn(&AgentStatus) -> bool,
) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if expected(&thread.agent_status().await) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for agent status");
        }
        sleep(Duration::from_millis(10)).await;
    }
}

fn tool_output(request: &ResponsesRequest, call_id: &str) -> (Option<String>, Option<bool>) {
    request
        .function_call_output_content_and_success(call_id)
        .expect("function call output")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn v2_followup_task_starts_only_when_idle_and_capacity_is_available() -> Result<()> {
    let server = start_mock_server().await;
    let spawn_args = serde_json::to_string(&json!({
        "message": INITIAL_TASK,
        "task_name": "worker",
    }))?;
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| body_contains(request, SPAWN_PROMPT),
        sse(vec![
            ev_response_created("spawn-response"),
            ev_function_call("spawn-call", "spawn_agent", &spawn_args),
            ev_completed("spawn-response"),
        ]),
    )
    .await;
    let initial_worker = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| body_contains(request, INITIAL_TASK),
        sse(vec![
            ev_response_created("initial-worker-response"),
            ev_completed("initial-worker-response"),
        ]),
    )
    .await;
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| body_contains(request, "spawn-call"),
        sse(vec![
            ev_response_created("spawn-followup-response"),
            ev_assistant_message("spawn-followup-message", "spawned"),
            ev_completed("spawn-followup-response"),
        ]),
    )
    .await;

    let mut builder = test_codex().with_model("koffing").with_config(|config| {
        config
            .features
            .enable(Feature::Collab)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::MultiAgentV2)
            .expect("test config should allow feature update");
        config.multi_agent_v2.max_concurrent_threads_per_session = 2;
    });
    let test = builder.build(&server).await?;
    test.submit_turn(SPAWN_PROMPT).await?;
    let _ = initial_worker.single_request();

    let worker_id = test
        .thread_manager
        .list_thread_ids()
        .await
        .into_iter()
        .find(|thread_id| *thread_id != test.session_configured.thread_id)
        .expect("spawned worker thread");
    let worker = test.thread_manager.get_thread(worker_id).await?;
    wait_for_status(worker.as_ref(), |status| {
        matches!(status, AgentStatus::Completed(_))
    })
    .await?;

    let permit_holder = test
        .thread_manager
        .start_thread_with_options(StartThreadOptions {
            config: test.config.clone(),
            initial_history: InitialHistory::New,
            session_source: Some(SessionSource::SubAgent(SubAgentSource::Other(
                "permit-holder".to_string(),
            ))),
            thread_source: None,
            dynamic_tools: Vec::new(),
            metrics_service_name: None,
            parent_trace: None,
            environments: Vec::new(),
        })
        .await?;
    mount_response_once_match(
        &server,
        |request: &wiremock::Request| body_contains(request, "hold the permit"),
        sse_response(sse(vec![
            ev_response_created("permit-holder-response"),
            ev_completed("permit-holder-response"),
        ]))
        .set_delay(Duration::from_secs(1)),
    )
    .await;
    permit_holder
        .thread
        .submit(
            vec![UserInput::Text {
                text: "hold the permit".to_string(),
                text_elements: Vec::new(),
            }]
            .into(),
        )
        .await?;
    wait_for_status(permit_holder.thread.as_ref(), |status| {
        matches!(status, AgentStatus::Running)
    })
    .await?;

    let blocked_args = serde_json::to_string(&json!({
        "target": "worker",
        "message": BLOCKED_TASK,
    }))?;
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| body_contains(request, BLOCKED_PROMPT),
        sse(vec![
            ev_response_created("blocked-response"),
            ev_function_call("blocked-call", "followup_task", &blocked_args),
            ev_completed("blocked-response"),
        ]),
    )
    .await;
    let blocked_followup = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| body_contains(request, "blocked-call"),
        sse(vec![
            ev_response_created("blocked-followup-response"),
            ev_assistant_message("blocked-followup-message", "blocked"),
            ev_completed("blocked-followup-response"),
        ]),
    )
    .await;
    test.submit_turn(BLOCKED_PROMPT).await?;
    assert_eq!(
        tool_output(&blocked_followup.single_request(), "blocked-call"),
        (
            Some("collab tool failed: agent thread limit reached".to_string()),
            Some(false),
        )
    );

    wait_for_status(permit_holder.thread.as_ref(), |status| {
        matches!(status, AgentStatus::Completed(_))
    })
    .await?;

    let allowed_args = serde_json::to_string(&json!({
        "target": "worker",
        "message": ALLOWED_TASK,
    }))?;
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| body_contains(request, ALLOWED_PROMPT),
        sse(vec![
            ev_response_created("allowed-response"),
            ev_function_call("allowed-call", "followup_task", &allowed_args),
            ev_completed("allowed-response"),
        ]),
    )
    .await;
    let allowed_worker = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| body_contains(request, ALLOWED_TASK),
        sse(vec![
            ev_response_created("allowed-worker-response"),
            ev_completed("allowed-worker-response"),
        ]),
    )
    .await;
    let allowed_followup = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| body_contains(request, "allowed-call"),
        sse(vec![
            ev_response_created("allowed-followup-response"),
            ev_assistant_message("allowed-followup-message", "started"),
            ev_completed("allowed-followup-response"),
        ]),
    )
    .await;
    test.submit_turn(ALLOWED_PROMPT).await?;

    assert_eq!(
        tool_output(&allowed_followup.single_request(), "allowed-call"),
        (Some(String::new()), Some(true))
    );
    let allowed_request = allowed_worker.single_request();
    assert!(allowed_request.body_contains_text(ALLOWED_TASK));
    assert!(!allowed_request.body_contains_text(BLOCKED_TASK));

    Ok(())
}
