//! Owns scheduled and manual installs, daemon restarts, and updater replacement.

use std::path::Path;
#[cfg(unix)]
use std::process::Command as StdCommand;
use std::process::Stdio;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientFactory;
use codex_http_client::RouteAwareClientPool;
use futures::FutureExt;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
#[cfg(unix)]
use tokio::signal::unix::Signal;
#[cfg(unix)]
use tokio::signal::unix::SignalKind;
#[cfg(unix)]
use tokio::signal::unix::signal;
use tokio::time::Instant;
use tokio::time::sleep;
use tokio::time::sleep_until;

use crate::Daemon;
use crate::RestartIfRunningOutcome;
use crate::RestartMode;
use crate::managed_install::ExecutableIdentity;
use crate::managed_install::executable_identity;
use crate::managed_install::resolved_managed_codex_bin;
#[cfg(windows)]
use crate::settings::DaemonSettings;
use crate::settings::UpdaterSettings;

#[path = "manual_update.rs"]
mod manual_update;
#[path = "migration.rs"]
mod migration;

pub(crate) async fn request_manual_update(
    daemon: &Daemon,
    http_client_factory: HttpClientFactory,
) -> Result<crate::UpdateOutput> {
    if daemon
        .pid_file
        .file_name()
        .is_some_and(|name| name == crate::LEGACY_PID_FILE_NAME)
        && daemon.is_stable_standalone_release()?
    {
        let http = RouteAwareClientPool::new_without_request_logging(
            http_client_factory,
            ClientRouteClass::Other,
        );
        return migration::run(&http, daemon).await;
    }
    manual_update::request(daemon).await
}

const INITIAL_UPDATE_DELAY: Duration = Duration::from_secs(5 * 60);
const RESTART_RETRY_INTERVAL: Duration = Duration::from_millis(50);
#[cfg(unix)]
const INSTALL_URL: &str = "https://chatgpt.com/codex/install.sh";
#[cfg(windows)]
const INSTALL_URL: &str = "https://chatgpt.com/codex/install.ps1";

pub(crate) async fn run(http_client_factory: HttpClientFactory) -> Result<()> {
    let http = RouteAwareClientPool::new_without_request_logging(
        http_client_factory,
        ClientRouteClass::Other,
    );
    run_with_http(
        &http,
        &Daemon::from_environment()?,
        &current_updater_identity().await?,
    )
    .await
}

async fn run_with_http(
    http: &impl InstallerHttp,
    daemon: &Daemon,
    running_updater_identity: &ExecutableIdentity,
) -> Result<()> {
    #[cfg(unix)]
    let mut terminate =
        signal(SignalKind::terminate()).context("failed to install updater shutdown handler")?;
    #[cfg(windows)]
    let updater = {
        // Updater ownership needs only paths, not settings that may be mid-edit.
        crate::backend::pid_update_loop_backend(daemon.backend_paths(&DaemonSettings::default()))
    };
    #[cfg(windows)]
    updater.wait_for_ownership().await?;
    #[cfg(windows)]
    let mut terminate = Signal;
    #[cfg(windows)]
    let _installer_job = crate::backend::windows::updater_job()?;
    let socket_path = daemon.manual_update_socket_path();
    codex_uds::prepare_private_socket_directory(
        socket_path
            .parent()
            .context("updater socket has no parent")?,
    )
    .await?;
    if socket_path.exists() {
        tokio::fs::remove_file(&socket_path).await?;
    }
    let mut listener = Some(codex_uds::UnixListener::bind(&socket_path).await?);
    #[cfg(windows)]
    updater.mark_ready().await?;
    let needs_managed_handoff =
        match resolved_managed_codex_bin(&daemon.current_managed_codex_bin()?).await {
            Ok(managed_bin) => {
                executable_identity(&managed_bin).await.ok().as_ref()
                    != Some(running_updater_identity)
            }
            Err(_) => true,
        };
    let auto_update_enabled = UpdaterSettings::load(&daemon.settings_file)
        .await
        .map(|settings| settings.auto_update_enabled)
        .unwrap_or(true);
    let mut next_check = Instant::now()
        + if needs_managed_handoff || !auto_update_enabled {
            Duration::from_secs(15)
        } else {
            INITIAL_UPDATE_DELAY
        };
    let mut manual_handoff_pending = needs_managed_handoff;
    loop {
        tokio::select! {
            biased;
            _ = terminate.recv() => return Ok(()),
            connection = listener.as_mut().context("updater listener closed")?.accept() => {
                let connection = connection.context("failed to accept updater request")?;
                let disposition = match manual_update::handle_request(connection, http, daemon, running_updater_identity, &mut terminate).await {
                    Ok(manual_update::RequestDisposition::Stop) => return Ok(()),
                    Ok(disposition) => disposition,
                    Err(_) => manual_update::RequestDisposition::Continue,
                };
                if UpdaterSettings::load(&daemon.settings_file)
                    .await
                    .is_ok_and(|settings| !settings.auto_update_enabled)
                {
                    // Drain requests queued during this one-shot update before exiting.
                    next_check = Instant::now() + Duration::from_millis(100);
                    continue;
                }
                if matches!(disposition, manual_update::RequestDisposition::Unchanged) {
                    continue;
                }
                manual_handoff_pending = true;
                next_check = Instant::now();
            }
            _ = sleep_until(next_check) => {
                match UpdaterSettings::load(&daemon.settings_file).await {
                    Ok(settings) if !settings.auto_update_enabled => return Ok(()),
                    Err(_) => {
                        next_check = Instant::now() + Duration::from_secs(60);
                        continue;
                    }
                    Ok(_) => {}
                }
                // Failed successor cleanup leaves its PID published. The predecessor
                // must stop instead of installing again without ownership.
                #[cfg(windows)]
                updater.wait_for_ownership().await?;
                if manual_handoff_pending {
                    if !daemon.is_stable_standalone_release()? {
                        if !daemon.has_latest_selection_marker() {
                            return Ok(());
                        }
                        next_check = Instant::now() + Duration::from_secs(30);
                        continue;
                    }
                    match adopt_managed_updater(daemon, running_updater_identity, &mut listener).await {
                        Ok(UpdateLoopControl::Stop) => return Ok(()),
                        Ok(UpdateLoopControl::Continue) => {
                            manual_handoff_pending = false;
                            let Some(delay) = next_update_delay(daemon).await else {
                                return Ok(());
                            };
                            next_check = Instant::now() + delay;
                        }
                        Err(err) => {
                            if listener.is_none() {
                                return Err(err);
                            }
                            eprintln!("warning: failed to refresh managed updater: {err:#}");
                            next_check = Instant::now() + Duration::from_secs(30);
                        }
                    }
                    continue;
                }
                match update_once(http, daemon, running_updater_identity, &mut terminate, UpdateTrigger::Scheduled).await {
                    Ok((UpdateLoopControl::Continue, Some(_))) => {
                        manual_handoff_pending = true;
                        next_check = Instant::now();
                        continue;
                    }
                    Ok((UpdateLoopControl::Continue, None)) | Err(_) => {}
                    Ok((UpdateLoopControl::Stop, _)) => return Ok(()),
                }
                let Some(delay) = next_update_delay(daemon).await else {
                    return Ok(());
                };
                next_check = Instant::now() + delay;
            }
        }
    }
}

async fn next_update_delay(daemon: &Daemon) -> Option<Duration> {
    match UpdaterSettings::load(&daemon.settings_file).await {
        Ok(settings) if !settings.auto_update_enabled => None,
        Ok(settings) => Some(settings.update_interval(Duration::from_secs(60))),
        Err(_) => Some(Duration::from_secs(60)),
    }
}

async fn adopt_managed_updater(
    daemon: &Daemon,
    running_identity: &ExecutableIdentity,
    listener: &mut Option<codex_uds::UnixListener>,
) -> Result<UpdateLoopControl> {
    let managed_bin = resolved_managed_codex_bin(&daemon.current_managed_codex_bin()?).await?;
    if executable_identity(&managed_bin).await? == *running_identity {
        return Ok(UpdateLoopControl::Continue);
    }
    if !crate::managed_install::supports_daemon_update_loop(&managed_bin).await {
        return Ok(UpdateLoopControl::Stop);
    }
    #[cfg(unix)]
    {
        let _ = listener;
        reexec_managed_updater(&managed_bin).map(|_| UpdateLoopControl::Stop)
    }
    #[cfg(windows)]
    {
        let replacement = crate::backend::pid_update_loop_backend(
            daemon.backend_paths_with_bin(&daemon.load_settings().await?, &managed_bin),
        );
        listener.take();
        if let Err(err) = replacement.replace_current_updater().await {
            // A failed replacement may still own the PID if its cleanup could
            // not terminate the successor. Never reopen our request socket then.
            replacement.wait_for_ownership().await?;
            let socket_path = daemon.manual_update_socket_path();
            if socket_path.exists() {
                tokio::fs::remove_file(&socket_path).await?;
            }
            *listener = Some(codex_uds::UnixListener::bind(&socket_path).await?);
            return Err(err);
        }
        Ok(UpdateLoopControl::Stop)
    }
}

async fn sleep_or_terminate(duration: Duration, terminate: &mut Signal) -> bool {
    tokio::select! {
        _ = sleep(duration) => false,
        _ = terminate.recv() => true,
    }
}

enum UpdateLoopControl {
    Continue,
    Stop,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum UpdateTrigger {
    Scheduled,
    Manual,
}

async fn update_once(
    http: &impl InstallerHttp,
    daemon: &Daemon,
    running_updater_identity: &ExecutableIdentity,
    terminate: &mut Signal,
    trigger: UpdateTrigger,
) -> Result<(UpdateLoopControl, Option<RestartIfRunningOutcome>)> {
    if trigger == UpdateTrigger::Scheduled
        && !UpdaterSettings::load(&daemon.settings_file)
            .await?
            .auto_update_enabled
    {
        return Ok((UpdateLoopControl::Stop, None));
    }
    if release_selection_unstable(daemon, trigger)? {
        // An installer can be between changing current and publishing its
        // latest-channel marker. Retry after the interval instead of exiting.
        return Ok((UpdateLoopControl::Continue, None));
    }
    let (package_root, _, previous_release) = selected_release(daemon)?;
    let codex_home = package_root
        .parent()
        .and_then(Path::parent)
        .context("daemon package root has no Codex home")?;
    let script = tokio::select! {
        result = fetch_installer_script(http) => result?,
        _ = terminate.recv() => return Ok((UpdateLoopControl::Stop, None)),
    };
    anyhow::ensure!(
        script
            .windows(b"CODEX_INSTALL_IF_LATEST".len())
            .any(|window| window == b"CODEX_INSTALL_IF_LATEST"),
        "standalone installer does not support guarded updates"
    );
    if package_root.ends_with("app-server-daemon") {
        anyhow::ensure!(
            script
                .windows(b"CODEX_INSTALL_DAEMON_ONLY".len())
                .any(|window| window == b"CODEX_INSTALL_DAEMON_ONLY"),
            "installer does not support daemon-owned packages"
        );
    }
    if trigger == UpdateTrigger::Scheduled
        && !UpdaterSettings::load(&daemon.settings_file)
            .await?
            .auto_update_enabled
    {
        return Ok((UpdateLoopControl::Stop, None));
    }
    if release_selection_unstable(daemon, trigger)? {
        return Ok((UpdateLoopControl::Continue, None));
    }
    anyhow::ensure!(
        crate::managed_install::package_root(codex_home) == package_root,
        "daemon package root changed during the update; retry the command"
    );
    #[cfg(unix)]
    if matches!(
        run_installer_script(
            &script,
            InstallerMode::Update(&previous_release),
            &package_root,
            terminate.recv()
        )
        .await?,
        UpdateLoopControl::Stop
    ) {
        return Ok((UpdateLoopControl::Stop, None));
    }
    #[cfg(windows)]
    tokio::select! {
        result = run_installer_script(&script, InstallerMode::Update(&previous_release), &package_root) => { result?; },
        _ = terminate.recv() => return Ok((UpdateLoopControl::Stop, None)),
    }
    if release_selection_unstable(daemon, trigger)? {
        return Ok((UpdateLoopControl::Continue, None));
    }

    let managed_codex_bin =
        resolved_managed_codex_bin(&daemon.current_managed_codex_bin()?).await?;
    let restart_mode = match trigger {
        UpdateTrigger::Manual => RestartMode::IfBinaryOrVersionChanged,
        UpdateTrigger::Scheduled
            if executable_identity(&managed_codex_bin).await? != *running_updater_identity =>
        {
            RestartMode::Always
        }
        UpdateTrigger::Scheduled => RestartMode::IfVersionChanged,
    };

    loop {
        if terminate.recv().now_or_never().flatten().is_some() {
            return Ok((UpdateLoopControl::Stop, None));
        }
        match daemon
            .try_restart_if_running(restart_mode, &managed_codex_bin)
            .await?
        {
            RestartIfRunningOutcome::Busy => {
                if sleep_or_terminate(RESTART_RETRY_INTERVAL, terminate).await {
                    return Ok((UpdateLoopControl::Stop, None));
                }
            }
            RestartIfRunningOutcome::Restarted => {
                return Ok((
                    UpdateLoopControl::Continue,
                    Some(RestartIfRunningOutcome::Restarted),
                ));
            }
            RestartIfRunningOutcome::NotRunning => {
                return Ok((
                    UpdateLoopControl::Continue,
                    Some(RestartIfRunningOutcome::NotRunning),
                ));
            }
            RestartIfRunningOutcome::AlreadyCurrent
                if trigger == UpdateTrigger::Manual
                    && restart_mode == RestartMode::IfBinaryOrVersionChanged =>
            {
                anyhow::ensure!(
                    daemon.is_stable_standalone_release()?
                        && resolved_managed_codex_bin(&daemon.current_managed_codex_bin()?).await?
                            == managed_codex_bin,
                    "managed daemon changed during the update; retry"
                );
                return Ok((
                    UpdateLoopControl::Continue,
                    Some(RestartIfRunningOutcome::AlreadyCurrent),
                ));
            }
            RestartIfRunningOutcome::NotReady | RestartIfRunningOutcome::AlreadyCurrent => {
                anyhow::ensure!(
                    trigger != UpdateTrigger::Manual,
                    "managed daemon could not restart; retry when it is ready"
                );
                return Ok((
                    if daemon.is_stable_standalone_release()? {
                        UpdateLoopControl::Continue
                    } else {
                        UpdateLoopControl::Stop
                    },
                    None,
                ));
            }
        }
    }
}

fn release_selection_unstable(daemon: &Daemon, trigger: UpdateTrigger) -> Result<bool> {
    if daemon.is_stable_standalone_release()? {
        return Ok(false);
    }
    anyhow::ensure!(
        trigger != UpdateTrigger::Manual,
        "standalone install selection changed during the update"
    );
    Ok(true)
}

fn selected_release(daemon: &Daemon) -> Result<(std::path::PathBuf, std::path::PathBuf, String)> {
    let home = daemon
        .settings_file
        .parent()
        .and_then(Path::parent)
        .context("daemon settings path has no Codex home")?;
    let root = crate::managed_install::package_root(home);
    let release = std::fs::canonicalize(root.join("current"))?;
    let name = release
        .file_name()
        .context("managed release has no name")?
        .to_string_lossy()
        .into_owned();
    Ok((root, release, name))
}

async fn current_updater_identity() -> Result<ExecutableIdentity> {
    let current_exe =
        std::env::current_exe().context("failed to resolve current updater executable")?;
    executable_identity(&current_exe).await
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

enum InstallerMode<'a> {
    Update(&'a str),
    Migration,
}

async fn run_installer_script(
    script: &[u8],
    mode: InstallerMode<'_>,
    package_root: &Path,
    #[cfg(unix)] terminate: impl std::future::Future<Output = Option<()>>,
) -> Result<UpdateLoopControl> {
    #[cfg(unix)]
    let mut command = {
        let mut command = Command::new("/bin/sh");
        command.arg("-s");
        command.process_group(0);
        command
    };
    #[cfg(windows)]
    let mut command = {
        let mut command = Command::new("powershell.exe");
        command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
        command.args(["-NoLogo", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", "try { Invoke-Expression ([Console]::In.ReadToEnd()) } catch { Write-Error $_; exit 1 }"])
            .kill_on_drop(true);
        command
    };
    let (guard, previous_release, defer_selection) = match mode {
        InstallerMode::Update(release) => ("1", release, "0"),
        InstallerMode::Migration => ("0", "", "1"),
    };
    let mut child = command
        .env(
            "CODEX_HOME",
            package_root
                .parent()
                .and_then(Path::parent)
                .context("package root has no Codex home")?,
        )
        .env("CODEX_INSTALL_DEFER_SELECTION", defer_selection)
        .env(
            "CODEX_INSTALL_DAEMON_ONLY",
            if package_root.ends_with("app-server-daemon") {
                "1"
            } else {
                "0"
            },
        )
        .env("CODEX_RELEASE", "latest")
        .env("CODEX_NON_INTERACTIVE", "1")
        .env("CODEX_INSTALL_IF_LATEST", guard)
        .env("CODEX_UPDATE_FROM_RELEASE", previous_release)
        .kill_on_drop(true)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to invoke standalone Codex updater")?;
    #[cfg(windows)]
    let _installer_job = crate::backend::windows::installer_job(&child)?;
    let mut stdin = child
        .stdin
        .take()
        .context("standalone Codex updater stdin was unavailable")?;
    #[cfg(unix)]
    let mut terminate = std::pin::pin!(terminate);
    #[cfg(unix)]
    let write_result = tokio::select! {
        result = stdin.write_all(script) => Some(result),
        _ = &mut terminate => None,
    };
    #[cfg(windows)]
    let write_result = Some(stdin.write_all(script).await);
    drop(stdin);
    #[cfg(unix)]
    if write_result.is_none() {
        cancel_installer(&mut child, package_root).await;
        return Ok(UpdateLoopControl::Stop);
    }
    write_result
        .context("installer write was cancelled")?
        .context("failed to pass standalone Codex updater to shell")?;
    #[cfg(unix)]
    let status = tokio::select! {
        result = child.wait() => result,
        _ = &mut terminate => {
            cancel_installer(&mut child, package_root).await;
            return Ok(UpdateLoopControl::Stop);
        }
    };
    #[cfg(windows)]
    let status = child.wait().await;
    let status = status.context("failed to wait for standalone Codex updater")?;

    if status.success() {
        Ok(UpdateLoopControl::Continue)
    } else {
        anyhow::bail!("standalone Codex updater exited with status {status}")
    }
}

#[cfg(unix)]
async fn cancel_installer(child: &mut tokio::process::Child, package_root: &Path) {
    let Some(pid) = child.id().and_then(|pid| libc::pid_t::try_from(pid).ok()) else {
        return;
    };
    // Let the shell's EXIT/TERM trap release the installer lock first.
    unsafe { libc::kill(-pid, libc::SIGTERM) };
    sleep(Duration::from_secs(2)).await;
    // Keep the shell unreaped until after the group kill, so its PID cannot
    // be reused while descendants that ignored TERM are still running.
    unsafe { libc::kill(-pid, libc::SIGKILL) };
    // A forced kill can bypass the shell trap on hosts using the mkdir lock.
    // The lock is ours only if its recorded owner is this still-unreaped shell.
    let lock = package_root.join("install.lock.d");
    if std::fs::read_to_string(lock.join("pid")).is_ok_and(|owner| owner.trim() == pid.to_string())
    {
        let _ = std::fs::remove_dir_all(lock);
    }
    let _ = child.wait().await;
}

async fn fetch_installer_script(http: &impl InstallerHttp) -> Result<Vec<u8>> {
    match http.get(INSTALL_URL).await? {
        InstallerResponse::Success(body) => Ok(body),
        InstallerResponse::Unsuccessful { status } => {
            anyhow::bail!("standalone Codex updater request failed with status {status}")
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum InstallerResponse {
    Success(Vec<u8>),
    Unsuccessful { status: u16 },
}

/// HTTP boundary used to download the standalone installer.
///
/// Implementations must issue a GET for the supplied URL, return exact response bytes for a
/// successful status, and report a non-success status without buffering its response body.
trait InstallerHttp: Send + Sync {
    fn get<'a>(
        &'a self,
        url: &'a str,
    ) -> impl std::future::Future<Output = Result<InstallerResponse>> + Send + 'a;
}

impl InstallerHttp for RouteAwareClientPool {
    async fn get(&self, url: &str) -> Result<InstallerResponse> {
        let response = RouteAwareClientPool::get(self, url)
            .send()
            .await
            .context("failed to fetch standalone Codex updater")?;
        if !response.status().is_success() {
            return Ok(InstallerResponse::Unsuccessful {
                status: response.status().as_u16(),
            });
        }
        let body = response
            .bytes()
            .await
            .context("failed to read standalone Codex updater")?
            .to_vec();
        Ok(InstallerResponse::Success(body))
    }
}

#[cfg(test)]
#[path = "update_loop_tests.rs"]
mod tests;

#[cfg(windows)]
struct Signal;

#[cfg(windows)]
impl Signal {
    async fn recv(&mut self) -> Option<()> {
        // An unreadable control path must stop the updater rather than disable shutdown.
        let _ = codex_app_server_transport::daemon_shutdown_signal().await;
        Some(())
    }
}
