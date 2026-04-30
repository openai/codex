use super::windows_common::finish_driver_spawn;
use super::windows_common::make_runner_resizer;
use super::windows_common::start_runner_pipe_writer;
use super::windows_common::start_runner_stdin_writer;
use super::windows_common::start_runner_stdout_reader;
use crate::ipc_framed::EmptyPayload;
use crate::ipc_framed::FramedMessage;
use crate::ipc_framed::Message;
use crate::ipc_framed::SpawnRequest;
use crate::runner_client::spawn_runner_transport;
use crate::spawn_prep::prepare_elevated_spawn_context;
use anyhow::Result;
use codex_utils_pty::ProcessDriver;
use codex_utils_pty::SpawnedProcess;
use std::collections::HashMap;
use std::path::Path;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

#[allow(clippy::too_many_arguments)]
pub(crate) async fn spawn_windows_sandbox_session_elevated(
    policy_json_or_preset: &str,
    sandbox_policy_cwd: &Path,
    codex_home: &Path,
    command: Vec<String>,
    cwd: &Path,
    mut env_map: HashMap<String, String>,
    timeout_ms: Option<u64>,
    tty: bool,
    stdin_open: bool,
    use_private_desktop: bool,
) -> Result<SpawnedProcess> {
    let elevated = prepare_elevated_spawn_context(
        policy_json_or_preset,
        sandbox_policy_cwd,
        codex_home,
        cwd,
        &mut env_map,
        &command,
    )?;

    let spawn_request = SpawnRequest {
        command: command.clone(),
        cwd: cwd.to_path_buf(),
        env: env_map.clone(),
        policy_json_or_preset: policy_json_or_preset.to_string(),
        sandbox_policy_cwd: sandbox_policy_cwd.to_path_buf(),
        codex_home: elevated.common.sandbox_base.clone(),
        real_codex_home: codex_home.to_path_buf(),
        cap_sids: elevated.cap_sids.clone(),
        timeout_ms,
        tty,
        stdin_open,
        use_private_desktop,
    };
    let codex_home = codex_home.to_path_buf();
    let cwd = cwd.to_path_buf();
    let sandbox_creds = elevated.sandbox_creds.clone();
    let logs_base_dir = elevated.common.logs_base_dir.clone();
    let transport = tokio::task::spawn_blocking(move || -> Result<_> {
        spawn_runner_transport(
            &codex_home,
            &cwd,
            &sandbox_creds,
            logs_base_dir.as_deref(),
            spawn_request,
        )
    })
    .await
    .map_err(|err| anyhow::anyhow!("runner handshake task failed: {err}"))??;
    let (pipe_write, pipe_read) = transport.into_files();

    let (writer_tx, writer_rx) = mpsc::channel::<Vec<u8>>(128);
    let (stdout_tx, stdout_rx) = broadcast::channel::<Vec<u8>>(256);
    let stderr_rx = if tty {
        None
    } else {
        Some(broadcast::channel::<Vec<u8>>(256))
    };
    let (exit_tx, exit_rx) = oneshot::channel::<i32>();

    let outbound_tx = start_runner_pipe_writer(pipe_write);
    let writer_handle = start_runner_stdin_writer(writer_rx, outbound_tx.clone(), tty, stdin_open);
    let terminator = {
        let outbound_tx = outbound_tx.clone();
        Some(Box::new(move || {
            let _ = outbound_tx.send(FramedMessage {
                version: 1,
                message: Message::Terminate {
                    payload: EmptyPayload::default(),
                },
            });
        }) as Box<dyn FnMut() + Send + Sync>)
    };

    start_runner_stdout_reader(
        pipe_read,
        stdout_tx,
        stderr_rx.as_ref().map(|(tx, _rx)| tx.clone()),
        exit_tx,
    );

    Ok(finish_driver_spawn(
        ProcessDriver {
            writer_tx,
            stdout_rx,
            stderr_rx: stderr_rx.map(|(_tx, rx)| rx),
            exit_rx,
            terminator,
            writer_handle: Some(writer_handle),
            resizer: if tty {
                Some(make_runner_resizer(outbound_tx))
            } else {
                None
            },
        },
        stdin_open,
    ))
}
