use anyhow::Result;
use codex_exec_server::ExecBackend;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecProcess;
use codex_exec_server::ProcessId;
use codex_exec_server::RemoveOptions;
use codex_sandboxing::SandboxLaunchConfig;
use core_test_support::PathBufExt;
use core_test_support::get_remote_test_env;
use core_test_support::test_codex::test_env;
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tokio::sync::watch;
use tokio::time::Duration;
use tokio::time::timeout;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_test_env_can_connect_and_use_filesystem() -> Result<()> {
    let Some(_remote_env) = get_remote_test_env() else {
        return Ok(());
    };

    let test_env = test_env().await?;
    let file_system = test_env.environment().get_filesystem();

    let file_path_abs = remote_test_file_path().abs();
    let payload = b"remote-test-env-ok".to_vec();

    file_system
        .write_file(&file_path_abs, payload.clone())
        .await?;
    let actual = file_system.read_file(&file_path_abs).await?;
    assert_eq!(actual, payload);

    file_system
        .remove(
            &file_path_abs,
            RemoveOptions {
                recursive: false,
                force: true,
            },
        )
        .await?;

    Ok(())
}

async fn read_remote_process_until_change(
    process: Arc<dyn ExecProcess>,
    wake_rx: &mut watch::Receiver<u64>,
    after_seq: Option<u64>,
) -> Result<codex_exec_server::ReadResponse> {
    let response = process
        .read(after_seq, /*max_bytes*/ None, /*wait_ms*/ Some(0))
        .await?;
    if !response.chunks.is_empty() || response.closed || response.failure.is_some() {
        return Ok(response);
    }

    timeout(Duration::from_secs(2), wake_rx.changed()).await??;
    process
        .read(after_seq, /*max_bytes*/ None, /*wait_ms*/ Some(0))
        .await
        .map_err(Into::into)
}

async fn collect_remote_process_output(
    process: Arc<dyn ExecProcess>,
    mut wake_rx: watch::Receiver<u64>,
) -> Result<(String, Option<i32>)> {
    let mut output = String::new();
    let mut exit_code = None;
    let mut after_seq = None;

    loop {
        let response =
            read_remote_process_until_change(Arc::clone(&process), &mut wake_rx, after_seq).await?;
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

    Ok((output, exit_code))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_test_env_can_start_process_with_remote_cwd_env_and_arg0() -> Result<()> {
    let Some(_remote_env) = get_remote_test_env() else {
        return Ok(());
    };

    let test_env = test_env().await?;
    let remote_cwd = test_env.cwd().clone().into_path_buf();
    let command =
        "printf 'PATH=%s\\n' \"$PATH\"; printf 'PWD=%s\\n' \"$PWD\"; printf 'ZERO=%s\\n' \"$0\""
            .to_string();
    let started = test_env
        .environment()
        .get_exec_backend()
        .start(ExecParams {
            process_id: ProcessId::from("remote-test-env-proc"),
            argv: vec!["/bin/sh".to_string(), "-lc".to_string(), command],
            cwd: remote_cwd.clone(),
            env: Default::default(),
            tty: false,
            arg0: Some("sandbox-wrapper".to_string()),
            sandbox: SandboxLaunchConfig::no_sandbox(remote_cwd.clone()),
            managed_network: None,
        })
        .await?;

    let wake_rx = started.process.subscribe_wake();
    let (output, exit_code) = collect_remote_process_output(started.process, wake_rx).await?;
    assert_eq!(exit_code, Some(0));

    let lines = output.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 3, "unexpected remote output: {output:?}");
    assert!(
        lines[0].starts_with("PATH=") && lines[0].len() > "PATH=".len(),
        "PATH should come from the remote exec-server"
    );
    assert_eq!(lines[1], format!("PWD={}", remote_cwd.display()));
    assert_eq!(lines[2], "ZERO=sandbox-wrapper");

    Ok(())
}

fn remote_test_file_path() -> PathBuf {
    let nanos = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(_) => 0,
    };
    PathBuf::from(format!(
        "/tmp/codex-remote-test-env-{}-{nanos}.txt",
        std::process::id()
    ))
}
