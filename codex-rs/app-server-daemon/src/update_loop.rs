#[cfg(unix)]
use std::process::Command as StdCommand;
#[cfg(unix)]
use std::process::Stdio;
#[cfg(unix)]
use std::time::Duration;

#[cfg(unix)]
use anyhow::Context;
use anyhow::Result;
#[cfg(not(unix))]
use anyhow::bail;
#[cfg(unix)]
use futures::FutureExt;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
#[cfg(unix)]
use tokio::io::AsyncWriteExt;
#[cfg(unix)]
use tokio::process::Command;
#[cfg(unix)]
use tokio::signal::unix::Signal;
#[cfg(unix)]
use tokio::signal::unix::SignalKind;
#[cfg(unix)]
use tokio::signal::unix::signal;
#[cfg(unix)]
use tokio::time::sleep;

#[cfg(unix)]
use crate::Daemon;
#[cfg(unix)]
use crate::RestartIfRunningOutcome;
#[cfg(unix)]
use crate::RestartMode;
#[cfg(unix)]
use crate::UpdaterRefreshMode;
#[cfg(unix)]
use crate::managed_install::ExecutableIdentity;
#[cfg(unix)]
use crate::managed_install::executable_identity;
#[cfg(unix)]
use crate::managed_install::resolved_managed_codex_bin;

#[cfg(unix)]
const INITIAL_UPDATE_DELAY: Duration = Duration::from_secs(5 * 60);
#[cfg(unix)]
const RESTART_RETRY_INTERVAL: Duration = Duration::from_millis(50);
#[cfg(unix)]
const UPDATE_INTERVAL: Duration = Duration::from_secs(60 * 60);

#[cfg(unix)]
pub(crate) async fn run() -> Result<()> {
    let mut terminate =
        signal(SignalKind::terminate()).context("failed to install updater shutdown handler")?;
    let running_updater_identity = current_updater_identity().await?;
    if sleep_or_terminate(INITIAL_UPDATE_DELAY, &mut terminate).await {
        return Ok(());
    }
    loop {
        match update_once(&running_updater_identity, &mut terminate).await {
            Ok(UpdateLoopControl::Continue) | Err(_) => {}
            Ok(UpdateLoopControl::Stop) => return Ok(()),
        }
        if sleep_or_terminate(UPDATE_INTERVAL, &mut terminate).await {
            return Ok(());
        }
    }
}

#[cfg(not(unix))]
pub(crate) async fn run() -> Result<()> {
    bail!("pid-managed updater loop is unsupported on this platform")
}

#[cfg(unix)]
async fn sleep_or_terminate(duration: Duration, terminate: &mut Signal) -> bool {
    tokio::select! {
        _ = sleep(duration) => false,
        _ = terminate.recv() => true,
    }
}

#[cfg(unix)]
enum UpdateLoopControl {
    Continue,
    Stop,
}

#[cfg(unix)]
async fn update_once(
    running_updater_identity: &ExecutableIdentity,
    terminate: &mut Signal,
) -> Result<UpdateLoopControl> {
    install_latest_standalone().await?;

    let daemon = Daemon::from_environment()?;
    let managed_codex_bin = resolved_managed_codex_bin(&daemon.managed_codex_bin).await?;
    let managed_identity = executable_identity(&managed_codex_bin).await?;
    let (restart_mode, updater_refresh_mode) =
        update_modes_for_identities(running_updater_identity, &managed_identity);

    loop {
        if terminate.recv().now_or_never().flatten().is_some() {
            return Ok(UpdateLoopControl::Stop);
        }
        match daemon
            .try_restart_if_running(restart_mode, updater_refresh_mode, &managed_codex_bin)
            .await?
        {
            RestartIfRunningOutcome::Busy => {
                if sleep_or_terminate(RESTART_RETRY_INTERVAL, terminate).await {
                    return Ok(UpdateLoopControl::Stop);
                }
            }
            _ => return Ok(UpdateLoopControl::Continue),
        }
    }
}

#[cfg(unix)]
async fn current_updater_identity() -> Result<ExecutableIdentity> {
    let current_exe =
        std::env::current_exe().context("failed to resolve current updater executable")?;
    executable_identity(&current_exe).await
}

#[cfg(unix)]
fn update_modes_for_identities(
    running_updater_identity: &ExecutableIdentity,
    managed_identity: &ExecutableIdentity,
) -> (RestartMode, UpdaterRefreshMode) {
    if running_updater_identity == managed_identity {
        (RestartMode::IfVersionChanged, UpdaterRefreshMode::None)
    } else {
        (
            RestartMode::Always,
            UpdaterRefreshMode::ReexecIfManagedBinaryChanged,
        )
    }
}

#[cfg(unix)]
pub(crate) fn reexec_managed_updater(managed_codex_bin: &std::path::Path) -> Result<()> {
    let err = StdCommand::new(managed_codex_bin)
        .args(["app-server", "daemon", "pid-update-loop"])
        .exec();
    Err(err).with_context(|| {
        format!(
            "failed to replace updater with managed Codex binary {}",
            managed_codex_bin.display()
        )
    })
}

#[cfg(unix)]
async fn install_latest_standalone() -> Result<()> {
    let script = reqwest::get("https://chatgpt.com/codex/install.sh")
        .await
        .context("failed to fetch standalone Codex updater")?
        .error_for_status()
        .context("standalone Codex updater request failed")?
        .bytes()
        .await
        .context("failed to read standalone Codex updater")?;

    let mut child = Command::new("/bin/sh")
        .arg("-s")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to invoke standalone Codex updater")?;
    let mut stdin = child
        .stdin
        .take()
        .context("standalone Codex updater stdin was unavailable")?;
    stdin
        .write_all(&script)
        .await
        .context("failed to pass standalone Codex updater to shell")?;
    drop(stdin);
    let status = child
        .wait()
        .await
        .context("failed to wait for standalone Codex updater")?;

    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("standalone Codex updater exited with status {status}")
    }
}

#[cfg(all(test, unix))]
#[path = "update_loop_tests.rs"]
mod tests;
