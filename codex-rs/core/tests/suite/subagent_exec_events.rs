#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Result;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_exec_command_pair;
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn root_subagent_tool_emits_exec_events() -> Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex();
    let test = builder.build(&server).await?;

    let call_id = "subagent-call";
    let args = json!({});

    // First completion triggers the subagent tool call.
    core_test_support::responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "subagent_list", &args.to_string()),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    // Second completion finishes the turn.
    core_test_support::responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "spawn one please".to_string(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd.path().to_path_buf(),
            approval_policy: codex_core::protocol::AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: test.session_configured.model.clone(),
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    let (begin, end) = wait_for_exec_command_pair(&test.codex, call_id).await;

    assert_eq!(
        begin.command.first().map(String::as_str),
        Some("Listed subagents"),
    );
    assert_eq!(end.call_id, begin.call_id);
    assert_eq!(end.exit_code, 0);

    Ok(())
}
