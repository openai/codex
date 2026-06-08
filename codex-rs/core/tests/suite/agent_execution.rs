use anyhow::Result;
use codex_core::CodexThread;
use codex_features::Feature;
use codex_protocol::protocol::AgentStatus;
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

const FIRST_PROMPT: &str = "spawn the first worker";
const SECOND_PROMPT: &str = "spawn the second worker";
const FIRST_TASK: &str = "first worker task";
const SECOND_TASK: &str = "second worker task";

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
async fn v2_spawn_checks_shared_active_execution_capacity() -> Result<()> {
    let server = start_mock_server().await;
    let first_args = serde_json::to_string(&json!({
        "message": FIRST_TASK,
        "task_name": "first",
    }))?;
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| body_contains(request, FIRST_PROMPT),
        sse(vec![
            ev_response_created("first-response"),
            ev_function_call("first-call", "spawn_agent", &first_args),
            ev_completed("first-response"),
        ]),
    )
    .await;
    mount_response_once_match(
        &server,
        |request: &wiremock::Request| body_contains(request, FIRST_TASK),
        sse_response(sse(vec![
            ev_response_created("first-worker-response"),
            ev_completed("first-worker-response"),
        ]))
        .set_delay(Duration::from_secs(3)),
    )
    .await;
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| body_contains(request, "first-call"),
        sse(vec![
            ev_response_created("first-followup-response"),
            ev_assistant_message("first-followup-message", "spawned"),
            ev_completed("first-followup-response"),
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
    test.submit_turn(FIRST_PROMPT).await?;

    let first_worker_id = test
        .thread_manager
        .list_thread_ids()
        .await
        .into_iter()
        .find(|thread_id| *thread_id != test.session_configured.thread_id)
        .expect("spawned worker thread");
    let first_worker = test.thread_manager.get_thread(first_worker_id).await?;
    wait_for_status(first_worker.as_ref(), |status| {
        matches!(status, AgentStatus::Running)
    })
    .await?;

    let second_args = serde_json::to_string(&json!({
        "message": SECOND_TASK,
        "task_name": "second",
    }))?;
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| body_contains(request, SECOND_PROMPT),
        sse(vec![
            ev_response_created("second-response"),
            ev_function_call("second-call", "spawn_agent", &second_args),
            ev_completed("second-response"),
        ]),
    )
    .await;
    let second_followup = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| body_contains(request, "second-call"),
        sse(vec![
            ev_response_created("second-followup-response"),
            ev_assistant_message("second-followup-message", "blocked"),
            ev_completed("second-followup-response"),
        ]),
    )
    .await;
    test.submit_turn(SECOND_PROMPT).await?;

    assert_eq!(
        tool_output(&second_followup.single_request(), "second-call"),
        (
            Some("collab spawn failed: agent thread limit reached".to_string()),
            Some(false),
        )
    );
    assert_eq!(test.thread_manager.list_thread_ids().await.len(), 2);

    Ok(())
}
