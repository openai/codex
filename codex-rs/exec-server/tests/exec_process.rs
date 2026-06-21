mod common;

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use codex_exec_server::Environment;
use codex_exec_server::ExecBackend;
use codex_exec_server::ExecOutputStream;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecProcess;
use codex_exec_server::ExecProcessEvent;
use codex_exec_server::ProcessId;
use codex_exec_server::ProcessSignal;
use codex_exec_server::ReadResponse;
use codex_exec_server::StartedExecProcess;
use codex_exec_server::WriteStatus;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use test_case::test_case;
use tokio::sync::watch;
use tokio::time::Duration;
use tokio::time::sleep;
use tokio::time::timeout;

use common::DELAYED_OUTPUT_AFTER_EXIT_PARENT_ARG;
use common::current_test_binary_helper_paths;
use common::exec_server::ExecServerHarness;
use common::exec_server::exec_server;

struct ProcessContext {
    backend: Arc<dyn ExecBackend>,
    _server: Option<ExecServerHarness>,
}

#[derive(Debug, PartialEq, Eq)]
enum ProcessEventSnapshot {
    Output {
        seq: u64,
        stream: ExecOutputStream,
        text: String,
    },
    Exited {
        seq: u64,
        exit_code: i32,
    },
    Closed {
        seq: u64,
    },
}

async fn create_process_context(use_remote: bool) -> Result<ProcessContext> {
    if use_remote {
        let server = exec_server().await?;
        let environment = Environment::create_for_tests(Some(server.websocket_url().to_string()))?;
        Ok(ProcessContext {
            backend: environment.get_exec_backend(),
            _server: Some(server),
        })
    } else {
        let environment = Environment::create_for_tests(/*exec_server_url*/ None)?;
        Ok(ProcessContext {
            backend: environment.get_exec_backend(),
            _server: None,
        })
    }
}

async fn assert_exec_process_starts_and_exits(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let session = context
        .backend
        .start(ExecParams {
            process_id: ProcessId::from("proc-1"),
            argv: vec!["true".to_string()],
            cwd: PathUri::from_path(std::env::current_dir()?)?,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
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

async fn collect_process_output_from_events(
    session: Arc<dyn ExecProcess>,
) -> Result<(String, String, Option<i32>, bool)> {
    let mut events = session.subscribe_events();
    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code = None;
    loop {
        match timeout(Duration::from_secs(2), events.recv()).await?? {
            ExecProcessEvent::Output(chunk) => match chunk.stream {
                ExecOutputStream::Stdout | ExecOutputStream::Pty => {
                    stdout.push_str(&String::from_utf8_lossy(&chunk.chunk.into_inner()));
                }
                ExecOutputStream::Stderr => {
                    stderr.push_str(&String::from_utf8_lossy(&chunk.chunk.into_inner()));
                }
            },
            ExecProcessEvent::Exited {
                seq: _,
                exit_code: code,
            } => {
                exit_code = Some(code);
            }
            ExecProcessEvent::Closed { seq: _ } => {
                drop(session);
                return Ok((stdout, stderr, exit_code, true));
            }
            ExecProcessEvent::Failed(message) => {
                anyhow::bail!("process failed before closed state: {message}");
            }
        }
    }
}

async fn collect_process_event_snapshots(
    session: Arc<dyn ExecProcess>,
) -> Result<Vec<ProcessEventSnapshot>> {
    let mut events = session.subscribe_events();
    let mut snapshots = Vec::new();
    loop {
        let snapshot = match timeout(Duration::from_secs(2), events.recv()).await?? {
            ExecProcessEvent::Output(chunk) => ProcessEventSnapshot::Output {
                seq: chunk.seq,
                stream: chunk.stream,
                text: String::from_utf8_lossy(&chunk.chunk.into_inner()).into_owned(),
            },
            ExecProcessEvent::Exited { seq, exit_code } => {
                ProcessEventSnapshot::Exited { seq, exit_code }
            }
            ExecProcessEvent::Closed { seq } => ProcessEventSnapshot::Closed { seq },
            ExecProcessEvent::Failed(message) => {
                anyhow::bail!("process failed before closed state: {message}");
            }
        };
        let closed = matches!(snapshot, ProcessEventSnapshot::Closed { .. });
        snapshots.push(snapshot);
        if closed {
            drop(session);
            return Ok(snapshots);
        }
    }
}

async fn assert_exec_process_streams_output(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
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
            cwd: PathUri::from_path(std::env::current_dir()?)?,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), process_id);

    let StartedExecProcess { process } = session;
    let wake_rx = process.subscribe_wake();
    let (output, exit_code, closed) = collect_process_output_from_reads(process, wake_rx).await?;
    assert_eq!(output, "session output\n");
    assert_eq!(exit_code, Some(0));
    assert!(closed);
    Ok(())
}

async fn assert_exec_process_pushes_events(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let process_id = "proc-events".to_string();
    let session = context
        .backend
        .start(ExecParams {
            process_id: process_id.clone().into(),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "printf 'event output\\n'; sleep 0.1; printf 'event err\\n' >&2; sleep 0.1; exit 7".to_string(),
            ],
            cwd: PathUri::from_path(std::env::current_dir()?)?,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), process_id);

    let StartedExecProcess { process } = session;
    let actual = collect_process_event_snapshots(process).await?;
    assert_eq!(
        actual,
        vec![
            ProcessEventSnapshot::Output {
                seq: 1,
                stream: ExecOutputStream::Stdout,
                text: "event output\n".to_string(),
            },
            ProcessEventSnapshot::Output {
                seq: 2,
                stream: ExecOutputStream::Stderr,
                text: "event err\n".to_string(),
            },
            ProcessEventSnapshot::Exited {
                seq: 3,
                exit_code: 7,
            },
            ProcessEventSnapshot::Closed { seq: 4 },
        ]
    );
    Ok(())
}

async fn assert_exec_process_replays_events_after_close(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let process_id = "proc-events-late".to_string();
    let session = context
        .backend
        .start(ExecParams {
            process_id: process_id.clone().into(),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "printf 'late one\\n'; printf 'late two\\n'".to_string(),
            ],
            cwd: PathUri::from_path(std::env::current_dir()?)?,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), process_id);

    let StartedExecProcess { process } = session;
    let wake_rx = process.subscribe_wake();
    let read_result = collect_process_output_from_reads(Arc::clone(&process), wake_rx).await?;
    assert_eq!(
        read_result,
        ("late one\nlate two\n".to_string(), Some(0), true)
    );

    let event_result = collect_process_output_from_events(process).await?;
    assert_eq!(
        event_result,
        (
            "late one\nlate two\n".to_string(),
            String::new(),
            Some(0),
            true
        )
    );
    Ok(())
}

async fn assert_exec_process_retains_output_after_exit_until_streams_close(
    use_remote: bool,
) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let (helper_binary, _) = current_test_binary_helper_paths()?;
    let release_dir = TempDir::new()?;
    let release_path = release_dir.path().join("release-delayed-output");
    let process_id = "proc-output-after-exit".to_string();
    let session = context
        .backend
        .start(ExecParams {
            process_id: process_id.clone().into(),
            argv: vec![
                helper_binary.to_string_lossy().into_owned(),
                DELAYED_OUTPUT_AFTER_EXIT_PARENT_ARG.to_string(),
                release_path.to_string_lossy().into_owned(),
            ],
            cwd: PathUri::from_path(std::env::current_dir()?)?,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), process_id);

    let StartedExecProcess { process } = session;

    let exit_response = timeout(
        Duration::from_secs(2),
        process.read(
            /*after_seq*/ None,
            /*max_bytes*/ None,
            /*wait_ms*/ Some(2_000),
        ),
    )
    .await??;
    assert!(
        exit_response.chunks.is_empty(),
        "parent should exit before child writes delayed output"
    );
    assert_eq!(exit_response.exit_code, Some(0));
    assert!(!exit_response.closed);
    let exit_seq = exit_response
        .next_seq
        .checked_sub(1)
        .context("exit response should advance next_seq")?;
    std::fs::write(&release_path, b"go")?;

    let late_response = timeout(
        Duration::from_secs(2),
        process.read(
            /*after_seq*/ Some(exit_seq),
            /*max_bytes*/ None,
            /*wait_ms*/ Some(2_000),
        ),
    )
    .await??;
    let mut late_output = String::new();
    for chunk in late_response.chunks {
        assert_eq!(chunk.stream, ExecOutputStream::Stdout);
        late_output.push_str(&String::from_utf8_lossy(&chunk.chunk.into_inner()));
    }
    assert_eq!(late_output, "late output after exit\n");

    let wake_rx = process.subscribe_wake();
    let actual = collect_process_output_from_reads(process, wake_rx).await?;
    assert_eq!(
        actual,
        ("late output after exit\n".to_string(), Some(0), true)
    );
    Ok(())
}

async fn assert_exec_process_write_then_read(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let process_id = "proc-stdin".to_string();
    let session = context
        .backend
        .start(ExecParams {
            process_id: process_id.clone().into(),
            argv: vec![
                // Use `/bin/sh` instead of Python so this stdin round-trip test
                // stays portable across Bazel and non-macOS runners where
                // `/usr/bin/python3` is not guaranteed to exist.
                "/bin/sh".to_string(),
                "-c".to_string(),
                "IFS= read line; printf 'from-stdin:%s\\n' \"$line\"".to_string(),
            ],
            cwd: PathUri::from_path(std::env::current_dir()?)?,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: true,
            pipe_stdin: false,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), process_id);

    tokio::time::sleep(Duration::from_millis(200)).await;
    session.process.write(b"hello\n".to_vec()).await?;
    let StartedExecProcess { process } = session;
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

async fn assert_exec_process_write_then_read_without_tty(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let process_id = "proc-stdin-pipe".to_string();
    let session = context
        .backend
        .start(ExecParams {
            process_id: process_id.clone().into(),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "IFS= read line; printf 'from-stdin:%s\\n' \"$line\"".to_string(),
            ],
            cwd: PathUri::from_path(std::env::current_dir()?)?,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: false,
            pipe_stdin: true,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), process_id);

    tokio::time::sleep(Duration::from_millis(200)).await;
    let write_response = session.process.write(b"hello\n".to_vec()).await?;
    assert_eq!(write_response.status, WriteStatus::Accepted);
    let StartedExecProcess { process } = session;
    let wake_rx = process.subscribe_wake();
    let actual = collect_process_output_from_reads(process, wake_rx).await?;

    assert_eq!(actual, ("from-stdin:hello\n".to_string(), Some(0), true));
    Ok(())
}

async fn assert_exec_process_rejects_write_without_pipe_stdin(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let process_id = "proc-stdin-closed".to_string();
    let session = context
        .backend
        .start(ExecParams {
            process_id: process_id.clone().into(),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "sleep 0.3; if IFS= read -r line; then printf 'read:%s\\n' \"$line\"; else printf 'eof\\n'; fi".to_string(),
            ],
            cwd: PathUri::from_path(std::env::current_dir()?)?,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), process_id);

    let write_response = session.process.write(b"ignored\n".to_vec()).await?;
    assert_eq!(write_response.status, WriteStatus::StdinClosed);
    let StartedExecProcess { process } = session;
    let wake_rx = process.subscribe_wake();
    let (output, exit_code, closed) = collect_process_output_from_reads(process, wake_rx).await?;

    assert_eq!(output, "eof\n");
    assert_eq!(exit_code, Some(0));
    assert!(closed);
    Ok(())
}

async fn assert_exec_process_signal_interrupts_process(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let process_id = "proc-signal".to_string();
    let session = context
        .backend
        .start(ExecParams {
            process_id: process_id.clone().into(),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "trap 'printf \"signal:2\\n\"; exit 7' INT; printf 'ready\\n'; while :; do :; done".to_string(),
            ],
            cwd: PathUri::from_path(std::env::current_dir()?)?,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), process_id);

    let StartedExecProcess { process } = session;
    let mut wake_rx = process.subscribe_wake();
    let mut ready_output = String::new();
    let mut after_seq = None;
    loop {
        let response =
            read_process_until_change(Arc::clone(&process), &mut wake_rx, after_seq).await?;
        for chunk in response.chunks {
            ready_output.push_str(&String::from_utf8_lossy(&chunk.chunk.into_inner()));
            after_seq = Some(chunk.seq);
        }
        if ready_output.contains("ready\n") {
            break;
        }
        if response.closed {
            anyhow::bail!("process closed before readiness marker: {ready_output:?}");
        }
        after_seq = response.next_seq.checked_sub(1).or(after_seq);
    }

    process.signal(ProcessSignal::Interrupt).await?;
    let (output, exit_code, closed) = collect_process_output_from_reads(process, wake_rx).await?;

    assert!(
        output.contains("signal:2"),
        "expected signal handler output, got {output:?}"
    );
    assert_eq!(exit_code, Some(7));
    assert!(closed);
    Ok(())
}

async fn assert_exec_process_signal_reports_unsupported_on_windows(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let session = context
        .backend
        .start(ExecParams {
            process_id: ProcessId::from("proc-windows-signal"),
            argv: vec![
                "cmd".to_string(),
                "/C".to_string(),
                "echo ready && ping -n 30 127.0.0.1 >NUL".to_string(),
            ],
            cwd: PathUri::from_path(std::env::current_dir()?)?,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
        })
        .await?;

    let err = match session.process.signal(ProcessSignal::Interrupt).await {
        Ok(()) => anyhow::bail!("Windows non-TTY signal should report unsupported"),
        Err(err) => err,
    };
    let message = err.to_string();
    assert!(
        message.contains("failed to signal process"),
        "unexpected signal error: {message}"
    );
    assert!(
        message.contains("process interrupt is not supported by this process backend"),
        "unexpected signal error: {message}"
    );

    session.process.terminate().await?;
    Ok(())
}

async fn assert_exec_process_preserves_queued_events_before_subscribe(
    use_remote: bool,
) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let session = context
        .backend
        .start(ExecParams {
            process_id: ProcessId::from("proc-queued"),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "printf 'queued output\\n'".to_string(),
            ],
            cwd: PathUri::from_path(std::env::current_dir()?)?,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
        })
        .await?;

    tokio::time::sleep(Duration::from_millis(200)).await;

    let StartedExecProcess { process } = session;
    let wake_rx = process.subscribe_wake();
    let (output, exit_code, closed) = collect_process_output_from_reads(process, wake_rx).await?;
    assert_eq!(output, "queued output\n");
    assert_eq!(exit_code, Some(0));
    assert!(closed);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(not(unix), ignore = "Unix-only exec-server process test")]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn remote_exec_process_recovers_after_transport_disconnect() -> Result<()> {
    let server = exec_server().await?;
    let mut proxy = server.disconnectable_websocket_proxy().await?;
    let environment = Environment::create_for_tests(Some(proxy.websocket_url().to_string()))?;
    let backend = environment.get_exec_backend();
    let temp_dir = TempDir::new()?;
    let gate_path = temp_dir.path().join("release-output");
    let emitted_path = temp_dir.path().join("output-emitted");
    let session = backend
        .start(ExecParams {
            process_id: ProcessId::from("proc-recover"),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                concat!(
                    "printf 'ready:%s\\n' \"$$\"; ",
                    "while [ ! -f \"$GATE\" ]; do /bin/sleep 0.01; done; ",
                    "printf 'during:%s\\n' \"$$\"; ",
                    ": > \"$EMITTED\"; ",
                    "IFS= read -r line; ",
                    "printf 'after:%s:%s\\n' \"$$\" \"$line\"; ",
                    "exit 7",
                )
                .to_string(),
            ],
            cwd: PathUri::from_path(std::env::current_dir()?)?,
            env_policy: /*env_policy*/ None,
            env: HashMap::from([
                (
                    "GATE".to_string(),
                    gate_path.to_string_lossy().into_owned(),
                ),
                (
                    "EMITTED".to_string(),
                    emitted_path.to_string_lossy().into_owned(),
                ),
            ]),
            tty: false,
            pipe_stdin: true,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
        })
        .await?;

    let process = Arc::clone(&session.process);
    let mut events = process.subscribe_events();
    let mut output = Vec::new();
    let mut last_seq = 0;
    while !output.ends_with(b"\n") {
        match timeout(Duration::from_secs(5), events.recv()).await?? {
            ExecProcessEvent::Output(chunk) => {
                assert_eq!(chunk.seq, last_seq + 1);
                last_seq = chunk.seq;
                output.extend_from_slice(&chunk.chunk.into_inner());
            }
            event => anyhow::bail!("expected ready output before disconnect, got {event:?}"),
        }
    }
    let ready = String::from_utf8(output.clone())?;
    let pid = ready
        .strip_prefix("ready:")
        .and_then(|line| line.strip_suffix('\n'))
        .context("ready output should contain the process id")?
        .to_string();

    proxy.pause_and_disconnect().await?;
    tokio::fs::write(&gate_path, b"").await?;
    timeout(Duration::from_secs(5), async {
        while tokio::fs::metadata(&emitted_path).await.is_err() {
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .context("process did not emit output while disconnected")?;

    let process_for_read = Arc::clone(&process);
    let mut pending_read = tokio::spawn(async move {
        process_for_read
            .read(
                /*after_seq*/ Some(last_seq),
                /*max_bytes*/ None,
                /*wait_ms*/ Some(0),
            )
            .await
    });
    assert!(
        timeout(Duration::from_millis(200), &mut pending_read)
            .await
            .is_err(),
        "process reads should wait while recovery is in progress"
    );
    proxy.resume()?;

    let recovered_read = timeout(Duration::from_secs(5), pending_read)
        .await
        .context("timed out waiting for a read after recovery")??;
    let recovered_read = recovered_read?;
    assert_eq!(recovered_read.failure, None);
    let recovered_output = recovered_read
        .chunks
        .into_iter()
        .flat_map(|chunk| chunk.chunk.into_inner())
        .collect::<Vec<_>>();
    assert_eq!(
        String::from_utf8(recovered_output)?,
        format!("during:{pid}\n")
    );

    let write = timeout(Duration::from_secs(5), process.write(b"hello\n".to_vec()))
        .await
        .context("timed out waiting for a write after recovery")??;
    assert_eq!(write.status, WriteStatus::Accepted);

    let mut saw_exit = false;
    loop {
        match timeout(Duration::from_secs(5), events.recv()).await?? {
            ExecProcessEvent::Output(chunk) => {
                assert_eq!(chunk.seq, last_seq + 1);
                last_seq = chunk.seq;
                output.extend_from_slice(&chunk.chunk.into_inner());
            }
            ExecProcessEvent::Exited { seq, exit_code } => {
                assert_eq!(seq, last_seq + 1);
                assert_eq!(exit_code, 7);
                last_seq = seq;
                saw_exit = true;
            }
            ExecProcessEvent::Closed { seq } => {
                assert!(saw_exit, "closed must be delivered after exit");
                assert_eq!(seq, last_seq + 1);
                break;
            }
            ExecProcessEvent::Failed(message) => {
                anyhow::bail!("process recovery failed: {message}");
            }
        }
    }
    assert_eq!(
        String::from_utf8(output)?,
        format!("ready:{pid}\nduring:{pid}\nafter:{pid}:hello\n")
    );

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[cfg_attr(not(unix), ignore = "Unix-only exec-server process test")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_starts_and_exits(use_remote: bool) -> Result<()> {
    assert_exec_process_starts_and_exits(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[cfg_attr(not(unix), ignore = "Unix-only exec-server process test")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_streams_output(use_remote: bool) -> Result<()> {
    assert_exec_process_streams_output(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[cfg_attr(not(unix), ignore = "Unix-only exec-server process test")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_pushes_events(use_remote: bool) -> Result<()> {
    assert_exec_process_pushes_events(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[cfg_attr(not(unix), ignore = "Unix-only exec-server process test")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_replays_events_after_close(use_remote: bool) -> Result<()> {
    assert_exec_process_replays_events_after_close(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[cfg_attr(not(unix), ignore = "Unix-only exec-server process test")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_retains_output_after_exit_until_streams_close(
    use_remote: bool,
) -> Result<()> {
    assert_exec_process_retains_output_after_exit_until_streams_close(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[cfg_attr(not(unix), ignore = "Unix-only exec-server process test")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_write_then_read(use_remote: bool) -> Result<()> {
    assert_exec_process_write_then_read(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[cfg_attr(not(unix), ignore = "Unix-only exec-server process test")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_write_then_read_without_tty(use_remote: bool) -> Result<()> {
    assert_exec_process_write_then_read_without_tty(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[cfg_attr(not(unix), ignore = "Unix-only exec-server process test")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_rejects_write_without_pipe_stdin(use_remote: bool) -> Result<()> {
    assert_exec_process_rejects_write_without_pipe_stdin(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[cfg_attr(not(unix), ignore = "Unix-only exec-server process test")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_signal_interrupts_process(use_remote: bool) -> Result<()> {
    assert_exec_process_signal_interrupts_process(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[cfg_attr(not(windows), ignore = "Windows-only exec-server process test")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_signal_reports_unsupported_on_windows(use_remote: bool) -> Result<()> {
    assert_exec_process_signal_reports_unsupported_on_windows(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[cfg_attr(not(unix), ignore = "Unix-only exec-server process test")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_preserves_queued_events_before_subscribe(use_remote: bool) -> Result<()> {
    assert_exec_process_preserves_queued_events_before_subscribe(use_remote).await
}
