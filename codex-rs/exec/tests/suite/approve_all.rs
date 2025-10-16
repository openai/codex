#![cfg(not(target_os = "windows"))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use anyhow::Result;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex_exec::test_codex_exec;
use serde_json::Value;
use serde_json::json;

async fn run_exec_with_args(args: &[&str]) -> Result<String> {
    let test = test_codex_exec();

    let call_id = "exec-approve";
    let exec_args = json!({
        "command": [
            if cfg!(windows) { "cmd.exe" } else { "/bin/sh" },
            if cfg!(windows) { "/C" } else { "-lc" },
            "echo approve-all-ok",
        ],
        "timeout_ms": 1500,
        "with_escalated_permissions": true
    });

    let response_streams = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell", &serde_json::to_string(&exec_args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];

    let server = responses::start_mock_server().await;
    let mock = mount_sse_sequence(&server, response_streams).await;

    test.cmd_with_server(&server).args(args).assert().success();

    let requests = mock.requests();
    assert!(requests.len() >= 2, "expected at least two responses POSTs");
    let item = requests[1].function_call_output(call_id);
    let output_str = item
        .get("output")
        .and_then(Value::as_str)
        .expect("function_call_output.output should be a string");

    Ok(output_str.to_string())
}

/// Setting `features.approve_all=true` should switch to auto-approvals.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn approve_all_auto_accepts_exec() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let output = run_exec_with_args(&[
        "--skip-git-repo-check",
        "-c",
        "features.approve_all=true",
        "train",
    ])
    .await?;
    assert!(
        output.contains("Exit code: 0"),
        "expected Exit code: 0 in output: {output}"
    );
    assert!(
        output.contains("approve-all-ok"),
        "expected command output in response: {output}"
    );

    Ok(())
}
