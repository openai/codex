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
    writer.send(b"print('hello from pty')\n".to_vec()).await?;
    writer.send(b"exit()\n".to_vec()).await?;

    let (output, code) = collect_output_until_exit(spawned.output_rx, spawned.exit_rx, 5_000).await;
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

    let pipe = spawn_pipe_process(
        "/bin/sh",
        &[String::from("-c"), String::from("echo pipe_ok; sleep 0.05")],
        Path::new("."),
        &env_map,
        &None,
    )
    .await?;
    let pty = spawn_pty_process(
        "/bin/sh",
        &[String::from("-c"), String::from("echo pty_ok; sleep 0.05")],
        Path::new("."),
        &env_map,
        &None,
    )
    .await?;

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
