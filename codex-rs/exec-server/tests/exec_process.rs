#![cfg(unix)]

mod common;

use std::sync::Arc;

use anyhow::Result;
use codex_exec_server::Environment;
use codex_exec_server::ExecBackend;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecProcess;
use codex_exec_server::ProcessId;
use codex_exec_server::ReadResponse;
use codex_exec_server::StartedExecProcess;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::SandboxPolicy;
use codex_sandboxing::SandboxLaunchConfig;
use codex_sandboxing::SandboxablePreference;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use test_case::test_case;
use tokio::sync::watch;
use tokio::time::Duration;
use tokio::time::timeout;

use common::exec_server::ExecServerHarness;
use common::exec_server::exec_server;

struct ProcessContext {
    backend: Arc<dyn ExecBackend>,
    server: Option<ExecServerHarness>,
}

async fn create_process_context(use_remote: bool) -> Result<ProcessContext> {
    if use_remote {
        let server = exec_server().await?;
        let environment = Environment::create(Some(server.websocket_url().to_string())).await?;
        Ok(ProcessContext {
            backend: environment.get_exec_backend(),
            server: Some(server),
        })
    } else {
        let environment = Environment::create(/*exec_server_url*/ None).await?;
        Ok(ProcessContext {
            backend: environment.get_exec_backend(),
            server: None,
        })
    }
}

async fn assert_exec_process_starts_and_exits(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let cwd = std::env::current_dir()?;
    let session = context
        .backend
        .start(ExecParams {
            process_id: ProcessId::from("proc-1"),
            argv: vec!["true".to_string()],
            cwd: cwd.clone(),
            env: Default::default(),
            tty: false,
            arg0: None,
            sandbox: SandboxLaunchConfig::no_sandbox(cwd),
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), "proc-1");
    let wake_rx = session.process.subscribe_wake();
    let (_, exit_code, closed) =
        collect_process_output_from_reads(session.process, wake_rx).await?;

    assert_eq!(exit_code, Some(0));
    assert!(closed);
    Ok(())
}

async fn read_process_until_change(
    session: Arc<dyn ExecProcess>,
    wake_rx: &mut watch::Receiver<u64>,
    after_seq: Option<u64>,
) -> Result<ReadResponse> {
    let response = session
        .read(after_seq, /*max_bytes*/ None, /*wait_ms*/ Some(0))
        .await?;
    if !response.chunks.is_empty() || response.closed || response.failure.is_some() {
        return Ok(response);
    }

    timeout(Duration::from_secs(2), wake_rx.changed()).await??;
    session
        .read(after_seq, /*max_bytes*/ None, /*wait_ms*/ Some(0))
        .await
        .map_err(Into::into)
}

async fn collect_process_output_from_reads(
    session: Arc<dyn ExecProcess>,
    mut wake_rx: watch::Receiver<u64>,
) -> Result<(String, Option<i32>, bool)> {
    let mut output = String::new();
    let mut exit_code = None;
    let mut after_seq = None;
    loop {
        let response =
            read_process_until_change(Arc::clone(&session), &mut wake_rx, after_seq).await?;
        if let Some(message) = response.failure {
            anyhow::bail!("process failed before closed state: {message}");
        }
        for chunk in response.chunks {
            output.push_str(&String::from_utf8_lossy(&chunk.chunk.into_inner()));
            after_seq = Some(chunk.seq);
        }
        if response.exited {
            exit_code = response.exit_code;
        }
        if response.closed {
            break;
        }
        after_seq = response.next_seq.checked_sub(1).or(after_seq);
    }
    drop(session);
    Ok((output, exit_code, true))
}

async fn assert_exec_process_streams_output(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let cwd = std::env::current_dir()?;
    let process_id = "proc-stream".to_string();
    let session = context
        .backend
        .start(ExecParams {
            process_id: process_id.clone().into(),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "sleep 0.05; printf 'session output\\n'".to_string(),
            ],
            cwd: cwd.clone(),
            env: Default::default(),
            tty: false,
            arg0: None,
            sandbox: SandboxLaunchConfig::no_sandbox(cwd),
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), process_id);

    let StartedExecProcess { process, .. } = session;
    let wake_rx = process.subscribe_wake();
    let (output, exit_code, closed) = collect_process_output_from_reads(process, wake_rx).await?;
    assert_eq!(output, "session output\n");
    assert_eq!(exit_code, Some(0));
    assert!(closed);
    Ok(())
}

async fn assert_exec_process_write_then_read(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let cwd = std::env::current_dir()?;
    let process_id = "proc-stdin".to_string();
    let session = context
        .backend
        .start(ExecParams {
            process_id: process_id.clone().into(),
            argv: vec![
                "/usr/bin/python3".to_string(),
                "-c".to_string(),
                "import sys; line = sys.stdin.readline(); sys.stdout.write(f'from-stdin:{line}'); sys.stdout.flush()".to_string(),
            ],
            cwd: cwd.clone(),
            env: Default::default(),
            tty: true,
            arg0: None,
            sandbox: SandboxLaunchConfig::no_sandbox(cwd),
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), process_id);

    tokio::time::sleep(Duration::from_millis(200)).await;
    session.process.write(b"hello\n".to_vec()).await?;
    let StartedExecProcess { process, .. } = session;
    let wake_rx = process.subscribe_wake();
    let (output, exit_code, closed) = collect_process_output_from_reads(process, wake_rx).await?;

    assert!(
        output.contains("from-stdin:hello"),
        "unexpected output: {output:?}"
    );
    assert_eq!(exit_code, Some(0));
    assert!(closed);
    Ok(())
}

async fn assert_exec_process_preserves_queued_events_before_subscribe(
    use_remote: bool,
) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let cwd = std::env::current_dir()?;
    let session = context
        .backend
        .start(ExecParams {
            process_id: ProcessId::from("proc-queued"),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "printf 'queued output\\n'".to_string(),
            ],
            cwd: cwd.clone(),
            env: Default::default(),
            tty: false,
            arg0: None,
            sandbox: SandboxLaunchConfig::no_sandbox(cwd),
        })
        .await?;

    tokio::time::sleep(Duration::from_millis(200)).await;

    let StartedExecProcess { process, .. } = session;
    let wake_rx = process.subscribe_wake();
    let (output, exit_code, closed) = collect_process_output_from_reads(process, wake_rx).await?;
    assert_eq!(output, "queued output\n");
    assert_eq!(exit_code, Some(0));
    assert!(closed);
    Ok(())
}

fn write_outside_workspace_sandbox(workspace_root: &std::path::Path) -> SandboxLaunchConfig {
    let mut policy = SandboxPolicy::new_workspace_write_policy();
    if let SandboxPolicy::WorkspaceWrite {
        exclude_tmpdir_env_var,
        exclude_slash_tmp,
        ..
    } = &mut policy
    {
        *exclude_tmpdir_env_var = true;
        *exclude_slash_tmp = true;
    }
    SandboxLaunchConfig {
        sandbox_preference: SandboxablePreference::Require,
        policy: policy.clone(),
        file_system_policy: FileSystemSandboxPolicy::from_legacy_sandbox_policy(
            &policy,
            workspace_root,
        ),
        network_policy: NetworkSandboxPolicy::from(&policy),
        sandbox_policy_cwd: workspace_root.to_path_buf(),
        additional_permissions: None,
        enforce_managed_network: false,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
        windows_sandbox_private_desktop: false,
        use_legacy_landlock: false,
    }
}

async fn assert_exec_process_sandbox_denies_write_outside_workspace(
    use_remote: bool,
) -> Result<()> {
    let temp_dir = TempDir::new()?;
    let workspace_root = temp_dir.path().join("workspace");
    std::fs::create_dir(&workspace_root)?;
    let blocked_path = temp_dir.path().join("blocked.txt");
    let context = create_process_context(use_remote).await?;
    let session = context
        .backend
        .start(ExecParams {
            process_id: ProcessId::from("proc-sandbox-denied"),
            argv: vec![
                "/usr/bin/python3".to_string(),
                "-c".to_string(),
                "from pathlib import Path; import sys; Path(sys.argv[1]).write_text('blocked')"
                    .to_string(),
                blocked_path.to_string_lossy().into_owned(),
            ],
            cwd: workspace_root.clone(),
            env: Default::default(),
            tty: false,
            arg0: None,
            sandbox: write_outside_workspace_sandbox(&workspace_root),
        })
        .await?;

    assert_eq!(session.sandbox_type, platform_sandbox_type());
    let StartedExecProcess { process, .. } = session;
    let wake_rx = process.subscribe_wake();
    let (_output, exit_code, closed) = collect_process_output_from_reads(process, wake_rx).await?;

    assert_ne!(exit_code, Some(0));
    assert!(closed);
    assert!(
        !blocked_path.exists(),
        "sandboxed process unexpectedly wrote outside the workspace root"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_exec_process_reports_transport_disconnect() -> Result<()> {
    let mut context = create_process_context(/*use_remote*/ true).await?;
    let session = context
        .backend
        .start(ExecParams {
            process_id: ProcessId::from("proc-disconnect"),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "sleep 10".to_string(),
            ],
            cwd: std::env::current_dir()?,
            env: Default::default(),
            tty: false,
            arg0: None,
            sandbox: SandboxLaunchConfig::no_sandbox(
                std::env::current_dir().expect("read current dir"),
            ),
        })
        .await?;

    let server = context
        .server
        .as_mut()
        .expect("remote context should include exec-server harness");
    server.shutdown().await?;

    let mut wake_rx = session.process.subscribe_wake();
    let response =
        read_process_until_change(session.process, &mut wake_rx, /*after_seq*/ None).await?;
    let message = response
        .failure
        .expect("disconnect should surface as a failure");
    assert!(
        message.starts_with("exec-server transport disconnected"),
        "unexpected failure message: {message}"
    );
    assert!(
        response.closed,
        "disconnect should close the process session"
    );

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_process_starts_and_exits(use_remote: bool) -> Result<()> {
    assert_exec_process_starts_and_exits(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_process_streams_output(use_remote: bool) -> Result<()> {
    assert_exec_process_streams_output(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_process_write_then_read(use_remote: bool) -> Result<()> {
    assert_exec_process_write_then_read(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_process_preserves_queued_events_before_subscribe(use_remote: bool) -> Result<()> {
    assert_exec_process_preserves_queued_events_before_subscribe(use_remote).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_exec_process_sandbox_denies_write_outside_workspace() -> Result<()> {
    assert_exec_process_sandbox_denies_write_outside_workspace(/*use_remote*/ true).await
}
