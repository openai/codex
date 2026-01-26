use anyhow::Result;
use codex_core::features::Feature;
use codex_core::protocol::AskForApproval;
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
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::path::Path;
use std::path::PathBuf;
use tokio::fs;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio::time::sleep;

#[derive(Debug)]
struct SnapshotRun {
    begin: ExecCommandBeginEvent,
    end: ExecCommandEndEvent,
    snapshot_path: PathBuf,
    snapshot_content: String,
    codex_home: PathBuf,
}

async fn wait_for_snapshot(codex_home: &Path) -> Result<PathBuf> {
    let snapshot_dir = codex_home.join("shell_snapshots");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(mut entries) = fs::read_dir(&snapshot_dir).await
            && let Some(entry) = entries.next_entry().await?
        {
            return Ok(entry.path());
        }

        if Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for shell snapshot");
        }

        sleep(Duration::from_millis(25)).await;
    }
}

#[allow(clippy::expect_used)]
async fn run_snapshot_command(command: &str) -> Result<SnapshotRun> {
    let builder = test_codex().with_config(|config| {
        config.use_experimental_unified_exec_tool = true;
        config.features.enable(Feature::UnifiedExec);
        config.features.enable(Feature::ShellSnapshot);
    });
    let harness = TestCodexHarness::with_builder(builder).await?;
    let args = json!({
        "cmd": command,
        "yield_time_ms": 1000,
    });
    let call_id = "shell-snapshot-exec";
    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "exec_command", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(harness.server(), responses).await;

    let test = harness.test();
    let codex = test.codex.clone();
    let codex_home = test.home.path().to_path_buf();
    let session_model = test.session_configured.model.clone();
    let cwd = test.cwd_path().to_path_buf();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "run unified exec with shell snapshot".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd,
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    let begin = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::ExecCommandBegin(ev) if ev.call_id == call_id => Some(ev.clone()),
        _ => None,
    })
    .await;
    let snapshot_path = wait_for_snapshot(&codex_home).await?;
    let snapshot_content = fs::read_to_string(&snapshot_path).await?;

    let end = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::ExecCommandEnd(ev) if ev.call_id == call_id => Some(ev.clone()),
        _ => None,
    })
    .await;

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    Ok(SnapshotRun {
        begin,
        end,
        snapshot_path,
        snapshot_content,
        codex_home,
    })
}

#[allow(clippy::expect_used)]
async fn run_shell_command_snapshot(command: &str) -> Result<SnapshotRun> {
    let builder = test_codex().with_config(|config| {
        config.features.enable(Feature::ShellSnapshot);
    });
    let harness = TestCodexHarness::with_builder(builder).await?;
    let args = json!({
        "command": command,
        "timeout_ms": 1000,
    });
    let call_id = "shell-snapshot-command";
    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell_command", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(harness.server(), responses).await;

    let test = harness.test();
    let codex = test.codex.clone();
    let codex_home = test.home.path().to_path_buf();
    let session_model = test.session_configured.model.clone();
    let cwd = test.cwd_path().to_path_buf();

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "run shell_command with shell snapshot".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd,
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    let begin = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::ExecCommandBegin(ev) if ev.call_id == call_id => Some(ev.clone()),
        _ => None,
    })
    .await;
    let snapshot_path = wait_for_snapshot(&codex_home).await?;
    let snapshot_content = fs::read_to_string(&snapshot_path).await?;

    let end = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::ExecCommandEnd(ev) if ev.call_id == call_id => Some(ev.clone()),
        _ => None,
    })
    .await;

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    Ok(SnapshotRun {
        begin,
        end,
        snapshot_path,
        snapshot_content,
        codex_home,
    })
}

fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn assert_posix_snapshot_sections(snapshot: &str) {
    assert!(snapshot.contains("# Snapshot file"));
    assert!(snapshot.contains("aliases "));
    assert!(snapshot.contains("exports "));
    assert!(snapshot.contains("setopts "));
    assert!(
        snapshot.contains("PATH"),
        "snapshot should include PATH exports; snapshot={snapshot:?}"
    );
}

#[cfg_attr(not(target_os = "linux"), ignore)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn linux_unified_exec_uses_shell_snapshot() -> Result<()> {
    let command = "echo snapshot-linux";
    let run = run_snapshot_command(command).await?;
    let stdout = normalize_newlines(&run.end.stdout);

    assert_eq!(run.begin.command.get(1).map(String::as_str), Some("-lc"));
    assert_eq!(run.begin.command.get(2).map(String::as_str), Some(command));
    assert_eq!(run.begin.command.len(), 3);
    assert!(run.snapshot_path.starts_with(&run.codex_home));
    assert_posix_snapshot_sections(&run.snapshot_content);
    assert_eq!(run.end.exit_code, 0);
    assert!(
        stdout.contains("snapshot-linux"),
        "stdout should contain snapshot marker; stdout={stdout:?}"
    );

    Ok(())
}

#[cfg_attr(target_os = "windows", ignore)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn linux_shell_command_uses_shell_snapshot() -> Result<()> {
    let command = "echo shell-command-snapshot-linux";
    let run = run_shell_command_snapshot(command).await?;

    assert_eq!(run.begin.command.get(1).map(String::as_str), Some("-lc"));
    assert_eq!(run.begin.command.get(2).map(String::as_str), Some(command));
    assert_eq!(run.begin.command.len(), 3);
    assert!(run.snapshot_path.starts_with(&run.codex_home));
    assert_posix_snapshot_sections(&run.snapshot_content);
    assert_eq!(
        normalize_newlines(&run.end.stdout).trim(),
        "shell-command-snapshot-linux"
    );
    assert_eq!(run.end.exit_code, 0);

    Ok(())
}

#[cfg_attr(target_os = "windows", ignore)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_command_snapshot_still_intercepts_apply_patch() -> Result<()> {
    let builder = test_codex().with_config(|config| {
        config.features.enable(Feature::ShellSnapshot);
        config.include_apply_patch_tool = true;
    });
    let harness = TestCodexHarness::with_builder(builder).await?;

    let test = harness.test();
    let codex = test.codex.clone();
    let cwd = test.cwd_path().to_path_buf();
    let codex_home = test.home.path().to_path_buf();
    let target = cwd.join("snapshot-apply.txt");

    let script = "apply_patch <<'EOF'\n*** Begin Patch\n*** Add File: snapshot-apply.txt\n+hello from snapshot\n*** End Patch\nEOF\n";
    let args = json!({
        "command": script,
        "timeout_ms": 1_000,
    });
    let call_id = "shell-snapshot-apply-patch";
    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell_command", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(harness.server(), responses).await;

    let model = test.session_configured.model.clone();
    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "apply patch via shell_command with snapshot".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: cwd.clone(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model,
            effort: None,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    assert_eq!(fs::read_to_string(&target).await?, "hello from snapshot\n");

    let snapshot_path = wait_for_snapshot(&codex_home).await?;
    let snapshot_content = fs::read_to_string(&snapshot_path).await?;
    assert_posix_snapshot_sections(&snapshot_content);

    Ok(())
}

#[cfg_attr(target_os = "windows", ignore)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_snapshot_deleted_after_shutdown_with_skills() -> Result<()> {
    let builder = test_codex().with_config(|config| {
        config.features.enable(Feature::ShellSnapshot);
    });
    let harness = TestCodexHarness::with_builder(builder).await?;
    let home = harness.test().home.clone();
    let codex_home = home.path().to_path_buf();
    let codex = harness.test().codex.clone();

    let snapshot_path = wait_for_snapshot(&codex_home).await?;
    assert!(snapshot_path.exists());

    codex.submit(Op::Shutdown {}).await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ShutdownComplete)).await;

    drop(codex);
    drop(harness);
    sleep(Duration::from_millis(150)).await;

    assert_eq!(
        snapshot_path.exists(),
        false,
        "snapshot should be removed after shutdown"
    );

    Ok(())
}

#[cfg_attr(not(target_os = "macos"), ignore)]
#[cfg_attr(
    target_os = "macos",
    ignore = "requires unrestricted networking on macOS"
)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn macos_unified_exec_uses_shell_snapshot() -> Result<()> {
    let command = "echo snapshot-macos";
    let run = run_snapshot_command(command).await?;

    let shell_path = run
        .begin
        .command
        .first()
        .expect("shell path recorded")
        .clone();
    assert_eq!(run.begin.command.get(1).map(String::as_str), Some("-c"));
    assert_eq!(
        run.begin.command.get(2).map(String::as_str),
        Some(". \"$0\" && exec \"$@\"")
    );
    assert_eq!(run.begin.command.get(4), Some(&shell_path));
    assert_eq!(run.begin.command.get(5).map(String::as_str), Some("-c"));
    assert_eq!(run.begin.command.last(), Some(&command.to_string()));

    assert!(run.snapshot_path.starts_with(&run.codex_home));
    assert_posix_snapshot_sections(&run.snapshot_content);
    assert_eq!(normalize_newlines(&run.end.stdout).trim(), "snapshot-macos");
    assert_eq!(run.end.exit_code, 0);

    Ok(())
}

// #[cfg_attr(not(target_os = "windows"), ignore)]
#[ignore]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn windows_unified_exec_uses_shell_snapshot() -> Result<()> {
    let command = "Write-Output snapshot-windows";
    let run = run_snapshot_command(command).await?;

    let snapshot_index = run
        .begin
        .command
        .iter()
        .position(|arg| arg.contains("shell_snapshots"))
        .expect("snapshot argument exists");
    assert!(run.begin.command.iter().any(|arg| arg == "-NoProfile"));
    assert!(
        run.begin
            .command
            .iter()
            .any(|arg| arg == "param($snapshot) . $snapshot; & @args")
    );
    assert!(snapshot_index > 0);
    assert_eq!(run.begin.command.last(), Some(&command.to_string()));

    assert!(run.snapshot_path.starts_with(&run.codex_home));
    assert!(run.snapshot_content.contains("# Snapshot file"));
    assert!(run.snapshot_content.contains("# aliases "));
    assert!(run.snapshot_content.contains("# exports "));
    assert_eq!(
        normalize_newlines(&run.end.stdout).trim(),
        "snapshot-windows"
    );
    assert_eq!(run.end.exit_code, 0);

    Ok(())
}
