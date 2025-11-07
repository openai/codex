#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Result;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecCommandBeginEvent;
use codex_core::protocol::ExecCommandEndEvent;
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
use core_test_support::wait_for_event_with_timeout;
use serde_json::json;
use tokio::time::Duration;

fn is_exec_begin(ev: &EventMsg) -> Option<ExecCommandBeginEvent> {
    if let EventMsg::ExecCommandBegin(ev) = ev {
        Some(ev.clone())
    } else {
        None
    }
}

fn is_exec_end(ev: &EventMsg) -> Option<ExecCommandEndEvent> {
    if let EventMsg::ExecCommandEnd(ev) = ev {
        Some(ev.clone())
    } else {
        None
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "relies on streaming timing; kept for manual verification"]
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

    let mut begin: Option<ExecCommandBeginEvent> = None;
    let mut end: Option<ExecCommandEndEvent> = None;
    for _ in 0..40 {
        let ev = wait_for_event_with_timeout(&test.codex, |_| true, Duration::from_secs(20)).await;
        if begin.is_none() {
            begin = is_exec_begin(&ev);
        }
        if end.is_none() {
            end = is_exec_end(&ev);
        }
        if matches!(ev, EventMsg::TaskComplete(_)) && begin.is_some() && end.is_some() {
            break;
        }
    }

    let begin = begin.expect("exec begin");
    assert_eq!(
        begin.command.first().map(String::as_str),
        Some("subagent_list")
    );
    let end = end.expect("exec end");
    assert_eq!(end.call_id, begin.call_id);
    assert_eq!(end.exit_code, 0);

    Ok(())
}
