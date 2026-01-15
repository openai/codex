use std::collections::HashMap;
use std::path::Path;

use pretty_assertions::assert_eq;

use crate::spawn_pipe_process;
use crate::spawn_pty_process;

fn find_python() -> Option<String> {
    for candidate in ["python3", "python"] {
        if let Ok(output) = std::process::Command::new(candidate)
            .arg("--version")
            .output()
        {
            if output.status.success() {
                return Some(candidate.to_string());
            }
        }
    }
    None
}

fn setsid_available() -> bool {
    if cfg!(windows) {
        return false;
    }
    std::process::Command::new("setsid")
        .arg("true")
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn shell_command(program: &str) -> (String, Vec<String>) {
    if cfg!(windows) {
        let cmd = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());
        (cmd, vec!["/C".to_string(), program.to_string()])
    } else {
        (
            "/bin/sh".to_string(),
            vec!["-c".to_string(), program.to_string()],
        )
    }
}

fn echo_sleep_command(marker: &str) -> String {
    if cfg!(windows) {
        format!("echo {marker} & ping -n 2 127.0.0.1 > NUL")
    } else {
        format!("echo {marker}; sleep 0.05")
    }
}

async fn collect_output_until_exit(
    mut output_rx: tokio::sync::broadcast::Receiver<Vec<u8>>,
    exit_rx: tokio::sync::oneshot::Receiver<i32>,
    timeout_ms: u64,
) -> (Vec<u8>, i32) {
    let mut collected = Vec::new();
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_millis(timeout_ms);
    tokio::pin!(exit_rx);

    loop {
        tokio::select! {
            res = output_rx.recv() => {
                if let Ok(chunk) = res {
                    collected.extend_from_slice(&chunk);
                }
            }
            res = &mut exit_rx => {
                let code = res.unwrap_or(-1);
                // On Windows (ConPTY in particular), it's possible to observe the exit notification
                // before the final bytes are drained from the PTY reader thread. Drain for a brief
                // "quiet" window to make output assertions deterministic.
                let (quiet_ms, max_ms) = if cfg!(windows) { (200, 2_000) } else { (50, 500) };
                let quiet = tokio::time::Duration::from_millis(quiet_ms);
                let max_deadline =
                    tokio::time::Instant::now() + tokio::time::Duration::from_millis(max_ms);
                while tokio::time::Instant::now() < max_deadline {
                    match tokio::time::timeout(quiet, output_rx.recv()).await {
                        Ok(Ok(chunk)) => collected.extend_from_slice(&chunk),
                        Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => continue,
                        Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => break,
                        Err(_) => break,
                    }
                }
                return (collected, code);
            }
            _ = tokio::time::sleep_until(deadline) => {
                return (collected, -1);
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pty_python_repl_emits_output_and_exits() -> anyhow::Result<()> {
    let Some(python) = find_python() else {
        eprintln!("python not found; skipping pty_python_repl_emits_output_and_exits");
        return Ok(());
    };

    let env_map: HashMap<String, String> = std::env::vars().collect();
    let spawned = spawn_pty_process(&python, &[], Path::new("."), &env_map, &None).await?;
    let writer = spawned.session.writer_sender();
    let newline = if cfg!(windows) { "\r\n" } else { "\n" };
    writer
        .send(format!("print('hello from pty'){newline}").into_bytes())
        .await?;
    writer.send(format!("exit(){newline}").into_bytes()).await?;

    let timeout_ms = if cfg!(windows) { 10_000 } else { 5_000 };
    let (output, code) =
        collect_output_until_exit(spawned.output_rx, spawned.exit_rx, timeout_ms).await;
    let text = String::from_utf8_lossy(&output);

    assert!(
        text.contains("hello from pty"),
        "expected python output in PTY: {text:?}"
    );
    assert_eq!(code, 0, "expected python to exit cleanly");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pipe_process_round_trips_stdin() -> anyhow::Result<()> {
    let Some(python) = find_python() else {
        eprintln!("python not found; skipping pipe_process_round_trips_stdin");
        return Ok(());
    };

    let args = vec![
        "-u".to_string(),
        "-c".to_string(),
        "import sys; print(sys.stdin.readline().strip());".to_string(),
    ];
    let env_map: HashMap<String, String> = std::env::vars().collect();
    let spawned = spawn_pipe_process(&python, &args, Path::new("."), &env_map, &None).await?;
    let writer = spawned.session.writer_sender();
    writer.send(b"roundtrip\n".to_vec()).await?;

    let (output, code) = collect_output_until_exit(spawned.output_rx, spawned.exit_rx, 5_000).await;
    let text = String::from_utf8_lossy(&output);

    assert!(
        text.contains("roundtrip"),
        "expected pipe process to echo stdin: {text:?}"
    );
    assert_eq!(code, 0, "expected python -c to exit cleanly");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pipe_and_pty_share_interface() -> anyhow::Result<()> {
    let env_map: HashMap<String, String> = std::env::vars().collect();

    let (pipe_program, pipe_args) = shell_command(&echo_sleep_command("pipe_ok"));
    let (pty_program, pty_args) = shell_command(&echo_sleep_command("pty_ok"));

    let pipe =
        spawn_pipe_process(&pipe_program, &pipe_args, Path::new("."), &env_map, &None).await?;
    let pty = spawn_pty_process(&pty_program, &pty_args, Path::new("."), &env_map, &None).await?;

    let (pipe_out, pipe_code) =
        collect_output_until_exit(pipe.output_rx, pipe.exit_rx, 3_000).await;
    let (pty_out, pty_code) = collect_output_until_exit(pty.output_rx, pty.exit_rx, 3_000).await;

    assert_eq!(pipe_code, 0);
    assert_eq!(pty_code, 0);
    assert!(
        String::from_utf8_lossy(&pipe_out).contains("pipe_ok"),
        "pipe output mismatch: {pipe_out:?}"
    );
    assert!(
        String::from_utf8_lossy(&pty_out).contains("pty_ok"),
        "pty output mismatch: {pty_out:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pipe_drains_stderr_without_stdout_activity() -> anyhow::Result<()> {
    let Some(python) = find_python() else {
        eprintln!("python not found; skipping pipe_drains_stderr_without_stdout_activity");
        return Ok(());
    };

    let script = "import sys\nchunk = 'E' * 65536\nfor _ in range(64):\n    sys.stderr.write(chunk)\n    sys.stderr.flush()\n";
    let args = vec!["-c".to_string(), script.to_string()];
    let env_map: HashMap<String, String> = std::env::vars().collect();
    let spawned = spawn_pipe_process(&python, &args, Path::new("."), &env_map, &None).await?;

    let (output, code) =
        collect_output_until_exit(spawned.output_rx, spawned.exit_rx, 10_000).await;

    assert_eq!(code, 0, "expected python to exit cleanly");
    assert!(!output.is_empty(), "expected stderr output to be drained");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pipe_terminate_aborts_detached_readers() -> anyhow::Result<()> {
    if !setsid_available() {
        eprintln!("setsid not available; skipping pipe_terminate_aborts_detached_readers");
        return Ok(());
    }

    let env_map: HashMap<String, String> = std::env::vars().collect();
    let script =
        "setsid sh -c 'i=0; while [ $i -lt 200 ]; do echo tick; sleep 0.01; i=$((i+1)); done' &";
    let (program, args) = shell_command(script);
    let mut spawned = spawn_pipe_process(&program, &args, Path::new("."), &env_map, &None).await?;

    let _ = tokio::time::timeout(
        tokio::time::Duration::from_millis(500),
        spawned.output_rx.recv(),
    )
    .await
    .map_err(|_| anyhow::anyhow!("expected detached output before terminate"))??;

    spawned.session.terminate();
    let mut post_rx = spawned.session.output_receiver();

    let post_terminate =
        tokio::time::timeout(tokio::time::Duration::from_millis(200), post_rx.recv()).await;

    match post_terminate {
        Err(_) => Ok(()),
        Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => Ok(()),
        Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {
            anyhow::bail!("unexpected output after terminate (lagged)")
        }
        Ok(Ok(chunk)) => anyhow::bail!(
            "unexpected output after terminate: {:?}",
            String::from_utf8_lossy(&chunk)
        ),
    }
}
