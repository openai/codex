use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use crate::process::ProcessHandle;
use crate::process::SpawnedProcess;

fn unavailable_spawned_process() -> SpawnedProcess {
    let (stdout_tx, stdout_rx) = mpsc::channel(1);
    let (stderr_tx, stderr_rx) = mpsc::channel(1);
    let (exit_tx, exit_rx) = oneshot::channel();
    drop(stdout_tx);
    drop(stderr_tx);
    let _ = exit_tx.send(1);
    SpawnedProcess {
        session: ProcessHandle::unavailable(),
        stdout_rx,
        stderr_rx,
        exit_rx,
    }
}

pub async fn spawn_process(
    _program: &str,
    _args: &[String],
    _cwd: &Path,
    _env: &HashMap<String, String>,
    _arg0: &Option<String>,
) -> Result<SpawnedProcess> {
    anyhow::bail!("pipe process execution is unavailable on wasm32");
}

pub async fn spawn_process_no_stdin(
    _program: &str,
    _args: &[String],
    _cwd: &Path,
    _env: &HashMap<String, String>,
    _arg0: &Option<String>,
) -> Result<SpawnedProcess> {
    anyhow::bail!("pipe process execution is unavailable on wasm32");
}

pub async fn spawn_process_no_stdin_with_inherited_fds(
    _program: &str,
    _args: &[String],
    _cwd: &Path,
    _env: &HashMap<String, String>,
    _arg0: &Option<String>,
    _inherited_fds: &[i32],
) -> Result<SpawnedProcess> {
    Ok(unavailable_spawned_process())
}
