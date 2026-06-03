use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_core::config::Constrained;
use codex_core::sandboxing::SandboxPermissions;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::items::TurnItem;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookOutputEntry;
use codex_protocol::protocol::HookOutputEntryKind;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::user_input::UserInput;
use core_test_support::hooks::trust_discovered_hooks;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_message_item_added;
use core_test_support::responses::ev_output_text_delta;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_sandbox;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tokio::sync::oneshot;
use tokio::time::sleep;
use tokio::time::timeout;

fn write_interrupt_hook(home: &Path, system_message: Option<&str>) -> Result<()> {
    let script_path = home.join("interrupt_hook.py");
    let log_path = home.join("interrupt_hook_log.jsonl");
    let system_message_json =
        serde_json::to_string(&system_message).context("serialize interrupt hook message")?;
    let script = format!(
        r#"import json
from pathlib import Path
import sys

log_path = Path(r"{log_path}")
system_message = json.loads({system_message_json:?})

payload = json.load(sys.stdin)
with log_path.open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")

if system_message is None:
    print("{{}}")
else:
    print(json.dumps({{"systemMessage": system_message}}))
"#,
        log_path = log_path.display(),
        system_message_json = system_message_json,
    );
    let hooks = serde_json::json!({
        "hooks": {
            "Interrupt": [{
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", script_path.display()),
                    "statusMessage": "running interrupt hook",
                }]
            }]
        }
    });

    fs::write(&script_path, script).context("write interrupt hook script")?;
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write hooks.json")?;
    Ok(())
}

fn read_hook_inputs_from_log(log_path: &Path) -> Result<Vec<Value>> {
    fs::read_to_string(log_path)
        .with_context(|| format!("read hook log {}", log_path.display()))?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).context("parse hook log line"))
        .collect()
}

fn read_interrupt_hook_inputs(home: &Path) -> Result<Vec<Value>> {
    read_hook_inputs_from_log(home.join("interrupt_hook_log.jsonl").as_path())
}

fn ev_message_item_done(id: &str, text: &str) -> Value {
    serde_json::json!({
        "type": "response.output_item.done",
        "item": {
            "type": "message",
            "role": "assistant",
            "id": id,
            "content": [{"type": "output_text", "text": text}]
        }
    })
}

fn sse_event(event: Value) -> String {
    sse(vec![event])
}

async fn submit_text_turn(test: &TestCodex, text: &str) -> Result<()> {
    test.codex
        .submit(Op::UserInput {
            environments: None,
            items: vec![UserInput::Text {
                text: text.to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    Ok(())
}

fn guardian_deny_assessment(rationale: &str) -> String {
    json!({
        "risk_level": "high",
        "user_authorization": "low",
        "outcome": "deny",
        "rationale": rationale,
    })
    .to_string()
}

#[tokio::test]
async fn interrupt_hook_runs_before_turn_aborted_and_records_payload() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let (gate_completed_tx, gate_completed_rx) = oneshot::channel();
    let chunks = vec![vec![
        StreamingSseChunk {
            gate: None,
            body: sse_event(ev_response_created("resp-1")),
        },
        StreamingSseChunk {
            gate: None,
            body: sse_event(ev_message_item_added("msg-1", "")),
        },
        StreamingSseChunk {
            gate: None,
            body: sse_event(ev_output_text_delta("first ")),
        },
        StreamingSseChunk {
            gate: None,
            body: sse_event(ev_message_item_done("msg-1", "first response")),
        },
        StreamingSseChunk {
            gate: Some(gate_completed_rx),
            body: sse_event(ev_completed("resp-1")),
        },
    ]];
    let (server, _completions) = start_streaming_sse_server(chunks).await;

    let mut builder = test_codex()
        .with_model("gpt-5.4")
        .with_pre_build_hook(|home| {
            if let Err(error) = write_interrupt_hook(home, Some("watch the tide")) {
                panic!("failed to write interrupt hook fixture: {error}");
            }
        })
        .with_config(trust_discovered_hooks);
    let test = builder.build_with_streaming_server(&server).await?;

    submit_text_turn(&test, "interrupt me").await?;

    let _: TurnItem = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::ItemCompleted(ItemCompletedEvent {
            item: TurnItem::AgentMessage(item),
            ..
        }) => Some(TurnItem::AgentMessage(item.clone())),
        _ => None,
    })
    .await;

    test.codex.submit(Op::Interrupt).await?;

    let started = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::HookStarted(started) if started.run.event_name == HookEventName::Interrupt => {
            Some(started.clone())
        }
        _ => None,
    })
    .await;
    let completed = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::HookCompleted(completed)
            if completed.run.event_name == HookEventName::Interrupt =>
        {
            Some(completed.clone())
        }
        _ => None,
    })
    .await;
    let aborted = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TurnAborted(aborted) => Some(aborted.clone()),
        _ => None,
    })
    .await;

    let _ = gate_completed_tx.send(());

    assert_eq!(started.run.event_name, HookEventName::Interrupt);
    assert_eq!(completed.run.event_name, HookEventName::Interrupt);
    assert_eq!(completed.run.status, HookRunStatus::Completed);
    assert_eq!(
        completed.run.entries,
        vec![HookOutputEntry {
            kind: HookOutputEntryKind::Warning,
            text: "watch the tide".to_string(),
        }]
    );
    assert_eq!(aborted.reason, TurnAbortReason::Interrupted);

    let hook_inputs = read_interrupt_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 1);
    let payload = &hook_inputs[0];
    assert_eq!(
        payload.get("hook_event_name"),
        Some(&Value::String("Interrupt".to_string()))
    );
    assert!(
        payload
            .get("turn_id")
            .and_then(Value::as_str)
            .is_some_and(|turn_id| !turn_id.is_empty())
    );
    assert!(
        payload
            .get("transcript_path")
            .and_then(Value::as_str)
            .is_some_and(|path| !path.is_empty())
    );
    assert!(payload.get("stop_hook_active").is_none());
    assert!(payload.get("last_assistant_message").is_none());

    Ok(())
}

#[tokio::test]
async fn startup_interrupt_without_active_turn_does_not_run_interrupt_hook() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            if let Err(error) = write_interrupt_hook(home, Some("should not run")) {
                panic!("failed to write interrupt hook fixture: {error}");
            }
        })
        .with_config(trust_discovered_hooks);
    let test = builder.build(&server).await?;

    test.codex.submit(Op::Interrupt).await?;
    sleep(Duration::from_millis(200)).await;

    assert!(
        !test
            .codex_home_path()
            .join("interrupt_hook_log.jsonl")
            .exists(),
        "startup interrupt should not invoke Interrupt hooks without an active turn",
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn replaced_abort_does_not_run_interrupt_hook() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let tool_args = json!({
        "command": "sleep 60",
        "timeout_ms": 60_000,
    })
    .to_string();
    let _response = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-replaced-1"),
            ev_function_call("call-replaced-1", "shell_command", &tool_args),
            ev_completed("resp-replaced-1"),
        ]),
    )
    .await;

    let mut builder = test_codex()
        .with_model("gpt-5.4")
        .with_pre_build_hook(|home| {
            if let Err(error) = write_interrupt_hook(home, Some("should not run")) {
                panic!("failed to write interrupt hook fixture: {error}");
            }
        })
        .with_config(trust_discovered_hooks);
    let test = builder.build(&server).await?;

    submit_text_turn(&test, "start a long-running turn").await?;

    let _ = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::ExecCommandBegin(begin) => Some(begin.clone()),
        _ => None,
    })
    .await;

    test.codex.submit(Op::Compact).await?;

    let mut saw_replaced_abort = false;
    for _ in 0..200 {
        let event = timeout(Duration::from_secs(20), test.codex.next_event())
            .await
            .context("timed out waiting for replaced-turn events")?
            .context("event stream ended unexpectedly")?;
        match event.msg {
            EventMsg::HookStarted(started)
                if started.run.event_name == HookEventName::Interrupt =>
            {
                panic!("replaced abort should not emit interrupt hook started event: {started:?}");
            }
            EventMsg::HookCompleted(completed)
                if completed.run.event_name == HookEventName::Interrupt =>
            {
                panic!(
                    "replaced abort should not emit interrupt hook completed event: {completed:?}"
                );
            }
            EventMsg::TurnAborted(aborted) if aborted.reason == TurnAbortReason::Replaced => {
                saw_replaced_abort = true;
                break;
            }
            _ => {}
        }
    }

    assert!(
        saw_replaced_abort,
        "expected a replaced abort for the first turn"
    );
    sleep(Duration::from_millis(200)).await;
    assert!(
        !test
            .codex_home_path()
            .join("interrupt_hook_log.jsonl")
            .exists(),
        "replaced abort should not invoke Interrupt hooks",
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_interrupt_runs_interrupt_hook() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let approval_policy_for_config = approval_policy;
    let sandbox_policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };
    let sandbox_policy_for_config = sandbox_policy.clone();

    let mut builder = test_codex()
        .with_model("gpt-5.4")
        .with_pre_build_hook(|home| {
            if let Err(error) = write_interrupt_hook(home, Some("guardian wave")) {
                panic!("failed to write interrupt hook fixture: {error}");
            }
        })
        .with_config(move |config| {
            trust_discovered_hooks(config);
            config.permissions.approval_policy = Constrained::allow_any(approval_policy_for_config);
            config
                .set_legacy_sandbox_policy(sandbox_policy_for_config)
                .expect("set sandbox policy");
        });
    let test = builder.build(&server).await?;

    let denied_output_path = test.cwd.path().join("guardian-denied.txt");
    let tool_args = json!({
        "cmd": format!("printf guardian-denied > {}", denied_output_path.display()),
        "yield_time_ms": 1_000_u64,
        "sandbox_permissions": SandboxPermissions::RequireEscalated,
        "justification": "Trigger Guardian denial.",
    });
    let serialized_args = serde_json::to_string(&tool_args)?;

    let _responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-guardian-parent-1"),
                ev_function_call("exec-call-1", "exec_command", &serialized_args),
                ev_completed("resp-guardian-parent-1"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian-review-1"),
                ev_assistant_message(
                    "msg-guardian-review-1",
                    &guardian_deny_assessment("first deny"),
                ),
                ev_completed("resp-guardian-review-1"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian-parent-2"),
                ev_function_call("exec-call-2", "exec_command", &serialized_args),
                ev_completed("resp-guardian-parent-2"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian-review-2"),
                ev_assistant_message(
                    "msg-guardian-review-2",
                    &guardian_deny_assessment("second deny"),
                ),
                ev_completed("resp-guardian-review-2"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian-parent-3"),
                ev_function_call("exec-call-3", "exec_command", &serialized_args),
                ev_completed("resp-guardian-parent-3"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian-review-3"),
                ev_assistant_message(
                    "msg-guardian-review-3",
                    &guardian_deny_assessment("third deny"),
                ),
                ev_completed("resp-guardian-review-3"),
            ]),
        ],
    )
    .await;

    test.codex
        .submit(Op::UserInput {
            environments: None,
            items: vec![UserInput::Text {
                text: "trigger guardian denial loop".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: ThreadSettingsOverrides {
                cwd: Some(test.cwd.path().to_path_buf()),
                approval_policy: Some(approval_policy),
                approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
                sandbox_policy: Some(sandbox_policy),
                ..Default::default()
            },
        })
        .await?;

    let started = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::HookStarted(started) if started.run.event_name == HookEventName::Interrupt => {
            Some(started.clone())
        }
        _ => None,
    })
    .await;
    let completed = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::HookCompleted(completed)
            if completed.run.event_name == HookEventName::Interrupt =>
        {
            Some(completed.clone())
        }
        _ => None,
    })
    .await;
    let aborted = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TurnAborted(aborted) => Some(aborted.clone()),
        _ => None,
    })
    .await;

    assert_eq!(started.run.event_name, HookEventName::Interrupt);
    assert_eq!(completed.run.event_name, HookEventName::Interrupt);
    assert_eq!(completed.run.status, HookRunStatus::Completed);
    assert_eq!(
        completed.run.entries,
        vec![HookOutputEntry {
            kind: HookOutputEntryKind::Warning,
            text: "guardian wave".to_string(),
        }]
    );
    assert_eq!(aborted.reason, TurnAbortReason::Interrupted);
    assert!(
        !denied_output_path.exists(),
        "denied guardian command should not run"
    );

    let hook_inputs = read_interrupt_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 1);
    let payload = &hook_inputs[0];
    assert_eq!(
        payload.get("hook_event_name"),
        Some(&Value::String("Interrupt".to_string()))
    );
    assert!(
        payload
            .get("turn_id")
            .and_then(Value::as_str)
            .is_some_and(|turn_id| !turn_id.is_empty())
    );
    assert!(
        payload
            .get("transcript_path")
            .and_then(Value::as_str)
            .is_some_and(|path| !path.is_empty())
    );
    assert!(payload.get("last_assistant_message").is_none());

    Ok(())
}
