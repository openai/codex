#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used)]

use std::time::Duration;
use std::time::Instant;

use codex_core::model_family::find_family_for_model;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use serde_json::json;

async fn run_turn(test: &TestCodex, prompt: &str) -> anyhow::Result<()> {
    let session_model = test.session_configured.model.clone();

    test.codex
        .submit(Op::UserTurn {
            items: vec![InputItem::Text {
                text: prompt.into(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    Ok(())
}

async fn run_turn_and_measure(test: &TestCodex, prompt: &str) -> anyhow::Result<Duration> {
    let start = Instant::now();
    run_turn(test, prompt).await?;
    Ok(start.elapsed())
}

#[allow(clippy::expect_used)]
async fn build_codex_with_test_tool(server: &wiremock::MockServer) -> anyhow::Result<TestCodex> {
    let mut builder = test_codex().with_config(|config| {
        config.model = "test-gpt-5-codex".to_string();
        config.model_family =
            find_family_for_model("test-gpt-5-codex").expect("test-gpt-5-codex model family");
    });
    builder.build(server).await
}

fn assert_parallel_duration(actual: Duration) {
    assert!(
        actual < Duration::from_millis(500),
        "expected parallel execution to finish quickly, got {actual:?}"
    );
}

fn assert_serial_duration(actual: Duration) {
    assert!(
        actual >= Duration::from_millis(500),
        "expected serial execution to take longer, got {actual:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn read_file_tools_run_in_parallel() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let test = build_codex_with_test_tool(&server).await?;

    let parallel_args = json!({
        "sleep_after_ms": 300,
        "barrier": {
            "id": "parallel-test-sync",
            "participants": 2,
            "timeout_ms": 1_000,
        }
    })
    .to_string();

    let first_response = sse(vec![
        json!({"type": "response.created", "response": {"id": "resp-1"}}),
        ev_function_call("call-1", "test_sync_tool", &parallel_args),
        ev_function_call("call-2", "test_sync_tool", &parallel_args),
        ev_completed("resp-1"),
    ]);
    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    mount_sse_sequence(&server, vec![first_response, second_response]).await;

    let duration = run_turn_and_measure(&test, "exercise sync tool").await?;
    assert_parallel_duration(duration);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn non_parallel_tools_run_serially() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let test = test_codex().build(&server).await?;

    let shell_args = json!({
        "command": ["/bin/sh", "-c", "sleep 0.3"],
        "timeout_ms": 1_000,
    });
    let args_one = serde_json::to_string(&shell_args)?;
    let args_two = serde_json::to_string(&shell_args)?;

    let first_response = sse(vec![
        json!({"type": "response.created", "response": {"id": "resp-1"}}),
        ev_function_call("call-1", "shell", &args_one),
        ev_function_call("call-2", "shell", &args_two),
        ev_completed("resp-1"),
    ]);
    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    mount_sse_sequence(&server, vec![first_response, second_response]).await;

    let duration = run_turn_and_measure(&test, "run shell twice").await?;
    assert_serial_duration(duration);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mixed_tools_fall_back_to_serial() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let test = build_codex_with_test_tool(&server).await?;

    let sync_args = json!({
        "sleep_after_ms": 300
    })
    .to_string();
    let shell_args = serde_json::to_string(&json!({
        "command": ["/bin/sh", "-c", "sleep 0.3"],
        "timeout_ms": 1_000,
    }))?;

    let first_response = sse(vec![
        json!({"type": "response.created", "response": {"id": "resp-1"}}),
        ev_function_call("call-1", "test_sync_tool", &sync_args),
        ev_function_call("call-2", "shell", &shell_args),
        ev_completed("resp-1"),
    ]);
    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    mount_sse_sequence(&server, vec![first_response, second_response]).await;

    let duration = run_turn_and_measure(&test, "mix tools").await?;
    assert_serial_duration(duration);

    Ok(())
}
