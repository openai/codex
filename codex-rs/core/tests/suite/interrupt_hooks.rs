use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookOutputEntry;
use codex_protocol::protocol::HookOutputEntryKind;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::user_input::UserInput;
use core_test_support::fs_wait;
use core_test_support::hooks::trust_discovered_hooks;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tokio::time::timeout;
use wiremock::MockServer;

fn write_interrupt_hook(home: &Path, system_message: Option<&str>) -> Result<()> {
    let script_path = home.join("interrupt_hook.py");
    let log_path = home.join("interrupt_hook_log.jsonl");
    let transcript_snapshot_path = home.join("interrupt_transcript_snapshot.jsonl");
    let system_message_json =
        serde_json::to_string(&system_message).context("serialize interrupt hook message")?;
    let script = format!(
        r#"import json
from pathlib import Path
import sys

payload = json.load(sys.stdin)
with Path(r"{log_path}").open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")

transcript_path = payload.get("transcript_path")
if transcript_path is not None:
    snapshot = Path(transcript_path).read_text(encoding="utf-8")
    Path(r"{transcript_snapshot_path}").write_text(snapshot, encoding="utf-8")

message = json.loads({system_message_json:?})
if message is not None:
    print(json.dumps({{"systemMessage": message}}))
"#,
        log_path = log_path.display(),
        transcript_snapshot_path = transcript_snapshot_path.display(),
        system_message_json = system_message_json,
    );
    let hooks = json!({
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

fn read_interrupt_hook_inputs(home: &Path) -> Result<Vec<Value>> {
    fs::read_to_string(home.join("interrupt_hook_log.jsonl"))
        .context("read interrupt hook log")?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).context("parse interrupt hook log line"))
        .collect()
}

async fn build_test(server: &MockServer, system_message: Option<&str>) -> Result<TestCodex> {
    let system_message = system_message.map(str::to_string);
    test_codex()
        .with_model("gpt-5.4")
        .with_pre_build_hook(move |home| {
            write_interrupt_hook(home, system_message.as_deref())
                .unwrap_or_else(|error| panic!("failed to write interrupt hook fixture: {error}"));
        })
        .with_config(|config| {
            trust_discovered_hooks(config);
            config.agent_interrupt_message_enabled = false;
        })
        .build_with_auto_env(server)
        .await
}

async fn start_interruptible_turn(test: &TestCodex, server: &MockServer) -> Result<()> {
    let tool_args = json!({
        "cmd": "sleep 60",
        "yield_time_ms": 60_000,
    })
    .to_string();
    _ = mount_sse_once(
        server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call("call-1", "exec_command", &tool_args),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "interrupt me".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let _ = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::ExecCommandBegin(begin) => Some(begin.clone()),
        _ => None,
    })
    .await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupt_hook_runs_before_turn_aborted_and_records_payload() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "command hooks currently require a host-native working directory"
    );

    let server = start_mock_server().await;
    let test = build_test(&server, Some("watch the tide")).await?;
    start_interruptible_turn(&test, &server).await?;

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
    let _aborted = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TurnAborted(aborted) if aborted.reason == TurnAbortReason::Interrupted => {
            Some(aborted.clone())
        }
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
            text: "watch the tide".to_string(),
        }]
    );

    let hook_inputs = read_interrupt_hook_inputs(test.codex_home_path())?;
    assert_eq!(hook_inputs.len(), 1);
    let payload = &hook_inputs[0];
    assert_eq!(
        payload.get("hook_event_name"),
        Some(&Value::String("Interrupt".to_string()))
    );
    assert_eq!(
        payload.get("model"),
        Some(&Value::String("gpt-5.4".to_string()))
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

    let transcript_snapshot = fs::read_to_string(
        test.codex_home_path()
            .join("interrupt_transcript_snapshot.jsonl"),
    )?;
    assert!(
        transcript_snapshot.contains("interrupt me"),
        "the interrupted turn must be durable before the hook reads its transcript",
    );
    assert!(
        !transcript_snapshot.contains("<turn_aborted>"),
        "disabled interrupt markers should remain absent from the hook transcript",
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn timed_out_interrupt_hook_fails_before_turn_aborted() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "command hooks currently require a host-native working directory"
    );

    let server = start_mock_server().await;
    let test = build_test(&server, /*system_message*/ None).await?;
    fs::write(
        test.codex_home_path().join("interrupt_hook.py"),
        "import time\ntime.sleep(60)\n",
    )?;
    start_interruptible_turn(&test, &server).await?;

    test.codex.submit(Op::Interrupt).await?;
    let completed = timeout(
        Duration::from_secs(5),
        wait_for_event_match(&test.codex, |event| match event {
            EventMsg::HookCompleted(completed)
                if completed.run.event_name == HookEventName::Interrupt =>
            {
                Some(completed.clone())
            }
            _ => None,
        }),
    )
    .await
    .context("interrupt hook should time out promptly")?;

    assert_eq!(completed.run.status, HookRunStatus::Failed);
    assert_eq!(
        completed.run.entries,
        vec![HookOutputEntry {
            kind: HookOutputEntryKind::Error,
            text: "hook timed out after 1s".to_string(),
        }]
    );

    timeout(
        Duration::from_secs(5),
        wait_for_event_match(&test.codex, |event| match event {
            EventMsg::TurnAborted(aborted) if aborted.reason == TurnAbortReason::Interrupted => {
                Some(aborted.clone())
            }
            _ => None,
        }),
    )
    .await
    .context("a timed-out interrupt hook must not prevent TurnAborted")?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn async_interrupt_hook_runs_without_delaying_turn_aborted() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "command hooks currently require a host-native working directory"
    );

    let server = start_mock_server().await;
    let test = test_codex()
        .with_pre_build_hook(|home| {
            write_interrupt_hook(home, Some("async interrupt completed"))
                .expect("write interrupt hook fixture");
            let hooks_path = home.join("hooks.json");
            let mut hooks: Value = serde_json::from_str(
                &fs::read_to_string(&hooks_path).expect("read interrupt hook configuration"),
            )
            .expect("parse interrupt hook configuration");
            hooks["hooks"]["Interrupt"][0]["hooks"][0]["async"] = json!(true);
            hooks["hooks"]["Interrupt"][0]["hooks"][0]["timeout"] = json!(3);
            fs::write(hooks_path, hooks.to_string()).expect("write async interrupt configuration");

            let script_path = home.join("interrupt_hook.py");
            let original_script = fs::read_to_string(&script_path).expect("read interrupt script");
            let release_path = serde_json::to_string(&home.join("async_interrupt_release"))
                .expect("serialize async interrupt release path");
            let finished_path = serde_json::to_string(&home.join("async_interrupt_finished"))
                .expect("serialize async interrupt finished path");
            let gated_script = format!(
                "import time\nfrom pathlib import Path\nwhile not Path({release_path}).exists(): time.sleep(0.01)\n{original_script}\nPath({finished_path}).touch()\n"
            );
            fs::write(script_path, gated_script).expect("write gated async interrupt script");
        })
        .with_config(trust_discovered_hooks)
        .build_with_auto_env(&server)
        .await?;
    start_interruptible_turn(&test, &server).await?;

    test.codex.submit(Op::Interrupt).await?;
    timeout(
        Duration::from_secs(5),
        wait_for_event_match(&test.codex, |event| match event {
            EventMsg::TurnAborted(aborted) if aborted.reason == TurnAbortReason::Interrupted => {
                Some(aborted.clone())
            }
            _ => None,
        }),
    )
    .await
    .context("an async interrupt hook must not delay TurnAborted")?;

    assert!(
        !test
            .codex_home_path()
            .join("interrupt_hook_log.jsonl")
            .exists(),
        "the gated async hook must not finish before the turn abort is emitted"
    );

    fs::write(
        test.codex_home_path().join("async_interrupt_release"),
        "ready",
    )?;
    fs_wait::wait_for_path_exists(
        test.codex_home_path().join("async_interrupt_finished"),
        Duration::from_secs(5),
    )
    .await
    .context("async interrupt hook should finish after the turn abort")?;
    assert_eq!(read_interrupt_hook_inputs(test.codex_home_path())?.len(), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn self_aborted_turn_runs_interrupt_hook_before_turn_aborted() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "command hooks currently require a host-native working directory"
    );

    let server = start_mock_server().await;
    let test = test_codex()
        .with_model("gpt-5.4")
        .with_pre_build_hook(|home| {
            write_interrupt_hook(home, Some("compaction was interrupted"))
                .expect("write interrupt hook fixture");
            let hooks_path = home.join("hooks.json");
            let mut hooks: Value = serde_json::from_str(
                &fs::read_to_string(&hooks_path).expect("read interrupt hook configuration"),
            )
            .expect("parse interrupt hook configuration");
            hooks["hooks"]["PreCompact"] = json!([{
                "matcher": "manual",
                "hooks": [{
                    "type": "command",
                    "command": r#"python3 -c 'import json; print(json.dumps({"continue": False, "stopReason": "stop compaction"}))'"#,
                }]
            }]);
            fs::write(hooks_path, hooks.to_string()).expect("write pre-compact hook configuration");
        })
        .with_config(trust_discovered_hooks)
        .build_with_auto_env(&server)
        .await?;

    test.codex.submit(Op::Compact).await?;
    let pre_compact = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::HookCompleted(completed)
            if completed.run.event_name == HookEventName::PreCompact =>
        {
            Some(completed.clone())
        }
        _ => None,
    })
    .await;
    assert_eq!(pre_compact.run.status, HookRunStatus::Stopped);

    let interrupt = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::HookCompleted(completed)
            if completed.run.event_name == HookEventName::Interrupt =>
        {
            Some(completed.clone())
        }
        _ => None,
    })
    .await;
    assert_eq!(interrupt.run.status, HookRunStatus::Completed);

    let _aborted = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TurnAborted(aborted) if aborted.reason == TurnAbortReason::Interrupted => {
            Some(aborted.clone())
        }
        _ => None,
    })
    .await;
    assert_eq!(read_interrupt_hook_inputs(test.codex_home_path())?.len(), 1);

    Ok(())
}

#[tokio::test]
async fn startup_interrupt_without_active_turn_does_not_run_interrupt_hook() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let test = build_test(&server, Some("should not run")).await?;

    test.codex.submit(Op::Interrupt).await?;
    test.codex.shutdown_and_wait().await?;

    assert!(
        !test
            .codex_home_path()
            .join("interrupt_hook_log.jsonl")
            .exists(),
        "startup interrupt should not invoke Interrupt hooks without an active turn",
    );

    Ok(())
}
