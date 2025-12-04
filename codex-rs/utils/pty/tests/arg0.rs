#[cfg(unix)]
use std::collections::HashMap;
#[cfg(unix)]
use std::time::Duration;

#[cfg(unix)]
use pretty_assertions::assert_eq;
#[cfg(unix)]
use tokio::sync::broadcast::error::RecvError;

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawn_pty_preserves_arg0_without_path_lookup() -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let mut env = HashMap::new();
    env.insert("PATH".to_string(), "/usr/bin:/bin".to_string());

    let arg0 = Some("codex-linux-sandbox".to_string());
    let args = vec!["-c".to_string(), "echo $0".to_string()];

    let spawned = codex_utils_pty::spawn_pty_process("/bin/sh", &args, &cwd, &env, &arg0).await?;

    let mut output_rx = spawned.output_rx;
    let mut exit_rx = spawned.exit_rx;
    let mut collected = Vec::new();

    let exit_code = loop {
        tokio::select! {
            exit_code = &mut exit_rx => break exit_code?,
            chunk = output_rx.recv() => match chunk {
                Ok(chunk) => collected.extend_from_slice(&chunk),
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break -1,
            }
        }
    };
    assert_eq!(exit_code, 0);

    loop {
        match tokio::time::timeout(Duration::from_millis(25), output_rx.recv()).await {
            Ok(Ok(chunk)) => collected.extend_from_slice(&chunk),
            Ok(Err(RecvError::Lagged(_))) => continue,
            Ok(Err(RecvError::Closed)) | Err(_) => break,
        }
    }

    let output = String::from_utf8_lossy(&collected);
    assert!(
        output.contains("codex-linux-sandbox"),
        "expected argv0 to include codex-linux-sandbox, got {output:?}"
    );

    Ok(())
}

#[cfg(not(unix))]
#[test]
fn spawn_pty_preserves_arg0_without_path_lookup() {}
