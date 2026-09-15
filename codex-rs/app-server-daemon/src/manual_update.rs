//! Forwards manual requests to the updater and reports its install and restart result.

use anyhow::Context;
use anyhow::Result;
use codex_uds::UnixStream;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::time::Duration;
use tokio::time::sleep;
use tokio::time::timeout;

use super::InstallerHttp;
use super::Signal;
use super::UpdateLoopControl;
use super::UpdateTrigger;
use super::selected_release;
use super::update_once;
use crate::Daemon;
use crate::RestartIfRunningOutcome;
use crate::UpdateOutput;
use crate::UpdateStatus;
use crate::client;
use crate::managed_install::executable_identity;
use crate::managed_install::managed_codex_version;

pub(crate) async fn request(daemon: &Daemon) -> Result<UpdateOutput> {
    let socket_path = daemon.manual_update_socket_path();
    let mut replacement_deadline = None;
    'request: loop {
        let mut connection = if let Some(deadline) = replacement_deadline {
            loop {
                if let Ok(connection) = UnixStream::connect(&socket_path).await {
                    break connection;
                }
                // A one-shot updater may exit after accepting the connection but
                // before reading our request. Give a successor time to appear,
                // then return to the normal startup path if none owns the PID.
                if tokio::time::Instant::now() + Duration::from_secs(29) >= deadline {
                    let settings = daemon.load_settings().await?;
                    if !crate::backend::pid_update_loop_backend(daemon.backend_paths(&settings))
                        .is_starting_or_running()
                        .await?
                    {
                        replacement_deadline = None;
                        continue 'request;
                    }
                }
                anyhow::ensure!(
                    tokio::time::Instant::now() < deadline,
                    "timed out waiting for the replacement daemon updater"
                );
                sleep(Duration::from_millis(50)).await;
            }
        } else {
            match UnixStream::connect(&socket_path).await {
                Ok(connection) => connection,
                Err(_) => {
                    let _operation_lock = daemon.acquire_operation_lock().await?;
                    if let Ok(connection) = UnixStream::connect(&socket_path).await {
                        connection
                    } else {
                        let settings = daemon.load_settings().await?;
                        if !supported(daemon)?
                            || (daemon.running_backend_instance(&settings).await?.is_none()
                                && client::probe(&daemon.socket_path).await.is_ok())
                        {
                            return unsupported(daemon).await;
                        }
                        crate::backend::pid_update_loop_backend(daemon.backend_paths(&settings))
                            .stop()
                            .await?;
                        let current_exe = std::env::current_exe()?;
                        let paths = daemon.backend_paths_with_bin(&settings, &current_exe);
                        let restore_release = if daemon.is_stable_standalone_release()? {
                            None
                        } else {
                            Some(selected_release(daemon)?.2)
                        };
                        let worker = crate::backend::PidBackend::new_update_loop(
                            paths.codex_bin,
                            paths.update_pid_file,
                            restore_release,
                        );
                        worker.start().await?;
                        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
                        loop {
                            if let Ok(connection) = UnixStream::connect(&socket_path).await {
                                break connection;
                            }
                            anyhow::ensure!(
                                tokio::time::Instant::now() < deadline,
                                "timed out waiting for the daemon updater to accept a manual request"
                            );
                            sleep(Duration::from_millis(50)).await;
                        }
                    }
                }
            }
        };
        let mut response = Vec::new();
        let transport = async {
            #[cfg(windows)]
            connection.ensure_non_elevated_peer()?;
            connection.write_all(b"update\n").await?;
            connection
                .take(16 * 1024)
                .read_to_end(&mut response)
                .await?;
            Ok::<_, std::io::Error>(())
        }
        .await;
        if response.is_empty() {
            if transport
                .as_ref()
                .is_err_and(|err| err.kind() == std::io::ErrorKind::PermissionDenied)
            {
                transport?;
            }
            let deadline = replacement_deadline
                .get_or_insert_with(|| tokio::time::Instant::now() + Duration::from_secs(30));
            anyhow::ensure!(
                tokio::time::Instant::now() < *deadline,
                "daemon updater disconnected before responding"
            );
            sleep(Duration::from_millis(50)).await;
            continue;
        }
        transport?;
        let outcome: std::result::Result<UpdateOutput, String> =
            serde_json::from_slice(&response).context("invalid response from daemon updater")?;
        let outcome = outcome.map_err(anyhow::Error::msg)?;
        if outcome.status == UpdateStatus::Unsupported && supported(daemon)? {
            // A scheduled or older updater cannot authorize restoring a pin.
            // Replace it under the writable operation lock, even with identical bytes.
            let _operation_lock = daemon.acquire_operation_lock().await?;
            let settings = daemon.load_settings().await?;
            let worker = crate::backend::pid_update_loop_backend(daemon.backend_paths(&settings));
            if !daemon.is_stable_standalone_release()? {
                worker.stop().await?;
                continue 'request;
            }
        }
        return Ok(outcome);
    }
}

pub(super) fn supported(daemon: &Daemon) -> Result<bool> {
    if daemon.is_stable_standalone_release()? {
        return Ok(true);
    }
    let Ok((root, release, _)) = selected_release(daemon) else {
        return Ok(false);
    };
    let bin = daemon.current_managed_codex_bin()?;
    Ok(std::fs::canonicalize(root.join("releases"))
        .is_ok_and(|releases| release.parent() == Some(releases.as_path()))
        && bin.is_file()
        && std::fs::canonicalize(bin).is_ok_and(|bin| bin.starts_with(&release)))
}

const UNSUPPORTED_MESSAGE: &str =
    "This command requires a daemon package selected from its managed releases directory.";

pub(super) async fn unsupported(daemon: &Daemon) -> Result<UpdateOutput> {
    let managed_codex_path = daemon.current_managed_codex_bin()?;
    Ok(UpdateOutput {
        status: UpdateStatus::Unsupported,
        installed_version: managed_codex_version(&managed_codex_path).await.ok(),
        running_version: client::probe(&daemon.socket_path)
            .await
            .ok()
            .map(|info| info.app_server_version),
        managed_codex_path,
        message: UNSUPPORTED_MESSAGE.to_string(),
    })
}

pub(super) async fn handle_request(
    mut connection: UnixStream,
    http: &impl InstallerHttp,
    daemon: &Daemon,
    running_updater_identity: &crate::managed_install::ExecutableIdentity,
    terminate: &mut Signal,
    trigger: UpdateTrigger<'_>,
) -> Result<RequestDisposition> {
    #[cfg(windows)]
    if connection.ensure_non_elevated_peer().is_err() {
        return Ok(RequestDisposition::Unchanged);
    }
    let mut request = [0; 7];
    if !matches!(
        timeout(Duration::from_secs(5), connection.read_exact(&mut request)).await,
        Ok(Ok(_))
    ) || &request != b"update\n"
    {
        return Ok(RequestDisposition::Unchanged);
    }
    let result = run(http, daemon, running_updater_identity, terminate, trigger).await;
    let interrupted = result.as_ref().err().is_some_and(|err| {
        err.downcast_ref::<std::io::Error>()
            .is_some_and(|err| err.kind() == std::io::ErrorKind::Interrupted)
    });
    let unsupported = result
        .as_ref()
        .is_ok_and(|output| output.status == UpdateStatus::Unsupported);
    let outcome: std::result::Result<UpdateOutput, String> =
        result.map_err(|err| format!("{err:#}"));
    let sent = connection.write_all(&serde_json::to_vec(&outcome)?).await;
    if interrupted {
        return Ok(RequestDisposition::Stop);
    }
    sent?;
    Ok(if unsupported {
        RequestDisposition::Unchanged
    } else {
        RequestDisposition::Continue
    })
}

pub(super) enum RequestDisposition {
    Continue,
    Unchanged,
    Stop,
}

pub(super) async fn run(
    http: &impl InstallerHttp,
    daemon: &Daemon,
    running_updater_identity: &crate::managed_install::ExecutableIdentity,
    terminate: &mut Signal,
    trigger: UpdateTrigger<'_>,
) -> Result<UpdateOutput> {
    let settings = daemon.load_settings().await?;
    let managed_running = daemon.running_backend_instance(&settings).await?.is_some();
    if !(daemon.is_stable_standalone_release()?
        || matches!(trigger, UpdateTrigger::RestoreProduction(_)) && supported(daemon)?)
        || (!managed_running && client::probe(&daemon.socket_path).await.is_ok())
    {
        return unsupported(daemon).await;
    }

    let managed_codex_path = daemon.current_managed_codex_bin()?;
    let (_, previous_release, _) = selected_release(daemon)?;
    let previous_identity = executable_identity(&managed_codex_path).await?;
    let (control, restart) =
        update_once(http, daemon, running_updater_identity, terminate, trigger).await?;
    if matches!(control, UpdateLoopControl::Stop) {
        return Err(std::io::Error::from(std::io::ErrorKind::Interrupted).into());
    }
    let current_managed_codex_path = daemon.current_managed_codex_bin()?;
    let installed_version = Some(managed_codex_version(&current_managed_codex_path).await?);
    let updated = previous_release != selected_release(daemon)?.1
        || previous_identity != executable_identity(&current_managed_codex_path).await?;
    let running_version = client::probe(&daemon.socket_path)
        .await
        .ok()
        .map(|info| info.app_server_version);
    let message = match restart {
        Some(RestartIfRunningOutcome::Restarted) => {
            "The managed installation is ready and the running daemon was restarted. Active or queued work may have been interrupted."
        }
        Some(RestartIfRunningOutcome::AlreadyCurrent) => {
            "The managed installation and running daemon are already current; the daemon was left running."
        }
        Some(RestartIfRunningOutcome::NotRunning) => {
            "The managed installation is ready; no daemon was running when the updater checked."
        }
        _ => unreachable!("successful manual update has a restart outcome"),
    };
    Ok(UpdateOutput {
        status: if updated {
            UpdateStatus::Updated
        } else {
            UpdateStatus::NoUpdate
        },
        managed_codex_path: current_managed_codex_path,
        installed_version,
        running_version,
        message: message.to_string(),
    })
}
