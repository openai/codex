mod backend;
mod client;
mod managed_install;
mod settings;
mod update_loop;

use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
pub use backend::BackendKind;
use backend::BackendPaths;
use codex_app_server_transport::app_server_control_socket_path;
use codex_utils_home_dir::find_codex_home;
use managed_install::managed_codex_bin;
#[cfg(unix)]
use managed_install::managed_codex_version;
use serde::Serialize;
use settings::DaemonSettings;
use tokio::time::sleep;

const START_POLL_INTERVAL: Duration = Duration::from_millis(50);
const START_TIMEOUT: Duration = Duration::from_secs(10);
const OPERATION_LOCK_TIMEOUT: Duration = Duration::from_secs(75);
const PID_FILE_NAME: &str = "app-server.pid";
const UPDATE_PID_FILE_NAME: &str = "app-server-updater.pid";
const OPERATION_LOCK_FILE_NAME: &str = "daemon.lock";
const SETTINGS_FILE_NAME: &str = "settings.json";
const STATE_DIR_NAME: &str = "app-server-daemon";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleCommand {
    Start,
    Restart,
    Stop,
    Version,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LifecycleStatus {
    AlreadyRunning,
    Started,
    Restarted,
    Stopped,
    NotRunning,
    Running,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleOutput {
    pub status: LifecycleStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend: Option<BackendKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    pub socket_path: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cli_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_server_version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootstrapOptions {
    pub remote_control_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BootstrapStatus {
    Bootstrapped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapOutput {
    pub status: BootstrapStatus,
    pub backend: BackendKind,
    pub auto_update_enabled: bool,
    pub remote_control_enabled: bool,
    pub managed_codex_path: PathBuf,
    pub socket_path: PathBuf,
    pub cli_version: String,
    pub app_server_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteControlMode {
    Enabled,
    Disabled,
}

impl RemoteControlMode {
    fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RemoteControlStatus {
    Enabled,
    Disabled,
    AlreadyEnabled,
    AlreadyDisabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteControlOutput {
    pub status: RemoteControlStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend: Option<BackendKind>,
    pub remote_control_enabled: bool,
    pub socket_path: PathBuf,
    pub cli_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_server_version: Option<String>,
}

#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RestartIfRunningOutcome {
    Busy,
    NotRunning,
    NotReady,
    AlreadyCurrent,
    Restarted,
}

#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RestartMode {
    IfVersionChanged,
    Always,
}

#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UpdaterRefreshMode {
    None,
    ReexecIfManagedBinaryChanged,
}

#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RestartDecision {
    NotReady,
    AlreadyCurrent,
    Restart,
}

pub async fn run(command: LifecycleCommand) -> Result<LifecycleOutput> {
    ensure_supported_platform()?;
    Daemon::from_environment()?.run(command).await
}

pub async fn bootstrap(options: BootstrapOptions) -> Result<BootstrapOutput> {
    ensure_supported_platform()?;
    Daemon::from_environment()?.bootstrap(options).await
}

pub async fn set_remote_control(mode: RemoteControlMode) -> Result<RemoteControlOutput> {
    ensure_supported_platform()?;
    Daemon::from_environment()?.set_remote_control(mode).await
}

pub async fn run_pid_update_loop() -> Result<()> {
    ensure_supported_platform()?;
    update_loop::run().await
}

#[cfg(unix)]
fn ensure_supported_platform() -> Result<()> {
    Ok(())
}

#[cfg(not(unix))]
fn ensure_supported_platform() -> Result<()> {
    Err(anyhow!(
        "codex app-server daemon lifecycle is only supported on Unix platforms"
    ))
}

struct Daemon {
    socket_path: PathBuf,
    pid_file: PathBuf,
    update_pid_file: PathBuf,
    operation_lock_file: PathBuf,
    settings_file: PathBuf,
    managed_codex_bin: PathBuf,
}

impl Daemon {
    fn from_environment() -> Result<Self> {
        let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
        let socket_path = app_server_control_socket_path(codex_home.as_path())?
            .as_path()
            .to_path_buf();
        let state_dir = codex_home.as_path().join(STATE_DIR_NAME);
        Ok(Self {
            socket_path,
            pid_file: state_dir.join(PID_FILE_NAME),
            update_pid_file: state_dir.join(UPDATE_PID_FILE_NAME),
            operation_lock_file: state_dir.join(OPERATION_LOCK_FILE_NAME),
            settings_file: state_dir.join(SETTINGS_FILE_NAME),
            managed_codex_bin: managed_codex_bin(codex_home.as_path()),
        })
    }

    async fn run(&self, command: LifecycleCommand) -> Result<LifecycleOutput> {
        match command {
            LifecycleCommand::Start => {
                let _operation_lock = self.acquire_operation_lock().await?;
                self.start().await
            }
            LifecycleCommand::Restart => {
                let _operation_lock = self.acquire_operation_lock().await?;
                self.restart().await
            }
            LifecycleCommand::Stop => {
                let _operation_lock = self.acquire_operation_lock().await?;
                self.stop().await
            }
            LifecycleCommand::Version => self.version().await,
        }
    }

    async fn start(&self) -> Result<LifecycleOutput> {
        let settings = self.load_settings().await?;
        if let Ok(info) = client::probe(&self.socket_path).await {
            return Ok(self.output(
                LifecycleStatus::AlreadyRunning,
                self.running_backend(&settings).await?,
                /*pid*/ None,
                Some(info.app_server_version),
            ));
        }

        if self.running_backend_instance(&settings).await?.is_some() {
            let info = self.wait_until_ready().await?;
            return Ok(self.output(
                LifecycleStatus::AlreadyRunning,
                Some(BackendKind::Pid),
                /*pid*/ None,
                Some(info.app_server_version),
            ));
        }

        self.ensure_managed_codex_bin()?;
        let pid = self.start_managed_backend(&settings).await?;
        let info = self.wait_until_ready().await?;
        Ok(self.output(
            LifecycleStatus::Started,
            Some(BackendKind::Pid),
            pid,
            Some(info.app_server_version),
        ))
    }

    async fn restart(&self) -> Result<LifecycleOutput> {
        let settings = self.load_settings().await?;
        if client::probe(&self.socket_path).await.is_ok()
            && self.running_backend(&settings).await?.is_none()
        {
            return Err(anyhow!(
                "app server is running but is not managed by codex app-server daemon"
            ));
        }

        self.ensure_managed_codex_bin()?;
        if let Some(backend) = self.running_backend_instance(&settings).await? {
            backend.stop().await?;
        }

        let pid = self.start_managed_backend(&settings).await?;
        let info = self.wait_until_ready().await?;
        Ok(self.output(
            LifecycleStatus::Restarted,
            Some(BackendKind::Pid),
            pid,
            Some(info.app_server_version),
        ))
    }

    #[cfg(unix)]
    pub(crate) async fn try_restart_if_running(
        &self,
        mode: RestartMode,
        updater_refresh_mode: UpdaterRefreshMode,
        managed_codex_bin: &Path,
    ) -> Result<RestartIfRunningOutcome> {
        let operation_lock = self.open_operation_lock_file().await?;
        if !try_lock_file(&operation_lock)? {
            return Ok(RestartIfRunningOutcome::Busy);
        }
        let settings = self.load_settings().await?;
        let outcome = if let Some(backend) = self.running_backend_instance(&settings).await? {
            let info = client::probe(&self.socket_path).await.ok();
            let managed_version = if info.is_some() {
                Some(managed_codex_version(managed_codex_bin).await?)
            } else {
                None
            };
            match restart_decision(mode, info.as_ref(), managed_version.as_deref()) {
                RestartDecision::NotReady => return Ok(RestartIfRunningOutcome::NotReady),
                RestartDecision::AlreadyCurrent => RestartIfRunningOutcome::AlreadyCurrent,
                RestartDecision::Restart => {
                    backend.stop().await?;
                    let _ = self
                        .start_managed_backend_with_bin(&settings, managed_codex_bin)
                        .await?;
                    self.wait_until_ready().await?;
                    RestartIfRunningOutcome::Restarted
                }
            }
        } else if client::probe(&self.socket_path).await.is_ok() {
            return Err(anyhow!(
                "app server is running but is not managed by codex app-server daemon"
            ));
        } else {
            RestartIfRunningOutcome::NotRunning
        };

        if should_reexec_updater(updater_refresh_mode, outcome) {
            crate::update_loop::reexec_managed_updater(managed_codex_bin)?;
        }

        Ok(outcome)
    }

    async fn stop(&self) -> Result<LifecycleOutput> {
        let settings = self.load_settings().await?;
        if let Some(backend) = self.running_backend_instance(&settings).await? {
            backend.stop().await?;
            return Ok(self.output(
                LifecycleStatus::Stopped,
                Some(BackendKind::Pid),
                /*pid*/ None,
                /*app_server_version*/ None,
            ));
        }

        if client::probe(&self.socket_path).await.is_ok() {
            return Err(anyhow!(
                "app server is running but is not managed by codex app-server daemon"
            ));
        }

        Ok(self.output(
            LifecycleStatus::NotRunning,
            /*backend*/ None,
            /*pid*/ None,
            /*app_server_version*/ None,
        ))
    }

    async fn version(&self) -> Result<LifecycleOutput> {
        let settings = self.load_settings().await?;
        let info = client::probe(&self.socket_path).await?;
        Ok(self.output(
            LifecycleStatus::Running,
            self.running_backend(&settings).await?,
            /*pid*/ None,
            Some(info.app_server_version),
        ))
    }

    async fn wait_until_ready(&self) -> Result<client::ProbeInfo> {
        let deadline = tokio::time::Instant::now() + START_TIMEOUT;
        loop {
            match client::probe(&self.socket_path).await {
                Ok(info) => return Ok(info),
                Err(err) if tokio::time::Instant::now() < deadline => {
                    let _ = err;
                    sleep(START_POLL_INTERVAL).await;
                }
                Err(err) => {
                    return Err(err).with_context(|| {
                        format!(
                            "app server did not become ready on {}",
                            self.socket_path.display()
                        )
                    });
                }
            }
        }
    }

    async fn bootstrap(&self, options: BootstrapOptions) -> Result<BootstrapOutput> {
        let _operation_lock = self.acquire_operation_lock().await?;
        self.bootstrap_locked(options).await
    }

    async fn set_remote_control(&self, mode: RemoteControlMode) -> Result<RemoteControlOutput> {
        let _operation_lock = self.acquire_operation_lock().await?;
        let previous_settings = self.load_settings().await?;
        let mut settings = previous_settings.clone();
        let remote_control_enabled = mode.is_enabled();
        let backend = self.running_backend_instance(&previous_settings).await?;

        if backend.is_none() && client::probe(&self.socket_path).await.is_ok() {
            return Err(anyhow!(
                "app server is running but is not managed by codex app-server daemon"
            ));
        }

        if settings.remote_control_enabled == remote_control_enabled {
            let info = if backend.is_some() {
                Some(self.wait_until_ready().await?)
            } else {
                None
            };
            return Ok(self.remote_control_output(
                already_remote_control_status(mode),
                backend.map(|_| BackendKind::Pid),
                remote_control_enabled,
                info.map(|info| info.app_server_version),
            ));
        }

        settings.remote_control_enabled = remote_control_enabled;
        settings.save(&self.settings_file).await?;

        let app_server_version = if let Some(backend) = backend {
            self.ensure_managed_codex_bin()?;
            backend.stop().await?;
            let _ = self.start_managed_backend(&settings).await?;
            Some(self.wait_until_ready().await?.app_server_version)
        } else {
            None
        };

        Ok(self.remote_control_output(
            remote_control_status(mode),
            app_server_version.as_ref().map(|_| BackendKind::Pid),
            remote_control_enabled,
            app_server_version,
        ))
    }

    async fn bootstrap_locked(&self, options: BootstrapOptions) -> Result<BootstrapOutput> {
        self.ensure_managed_codex_bin()?;

        let settings = DaemonSettings {
            remote_control_enabled: options.remote_control_enabled,
        };
        if client::probe(&self.socket_path).await.is_ok()
            && self.running_backend(&settings).await?.is_none()
        {
            return Err(anyhow!(
                "app server is running but is not managed by codex app-server daemon"
            ));
        }
        settings.save(&self.settings_file).await?;

        if let Some(backend) = self.running_backend_instance(&settings).await? {
            backend.stop().await?;
        }

        let backend = backend::pid_backend(self.backend_paths(&settings));
        backend.start().await?;
        let updater = backend::pid_update_loop_backend(self.backend_paths(&settings));
        if updater.is_starting_or_running().await? {
            updater.stop().await?;
        }
        updater.start().await?;

        let info = self.wait_until_ready().await?;
        Ok(BootstrapOutput {
            status: BootstrapStatus::Bootstrapped,
            backend: BackendKind::Pid,
            auto_update_enabled: true,
            remote_control_enabled: settings.remote_control_enabled,
            managed_codex_path: self.managed_codex_bin.clone(),
            socket_path: self.socket_path.clone(),
            cli_version: env!("CARGO_PKG_VERSION").to_string(),
            app_server_version: info.app_server_version,
        })
    }

    async fn running_backend(&self, settings: &DaemonSettings) -> Result<Option<BackendKind>> {
        Ok(self
            .running_backend_instance(settings)
            .await?
            .map(|_| BackendKind::Pid))
    }

    async fn running_backend_instance(
        &self,
        settings: &DaemonSettings,
    ) -> Result<Option<backend::PidBackend>> {
        let backend = backend::pid_backend(self.backend_paths(settings));
        if backend.is_starting_or_running().await? {
            return Ok(Some(backend));
        }
        Ok(None)
    }

    async fn start_managed_backend(&self, settings: &DaemonSettings) -> Result<Option<u32>> {
        self.start_managed_backend_with_bin(settings, &self.managed_codex_bin)
            .await
    }

    async fn start_managed_backend_with_bin(
        &self,
        settings: &DaemonSettings,
        managed_codex_bin: &Path,
    ) -> Result<Option<u32>> {
        let backend =
            backend::pid_backend(self.backend_paths_with_bin(settings, managed_codex_bin));
        backend.start().await
    }

    fn ensure_managed_codex_bin(&self) -> Result<()> {
        if self.managed_codex_bin.is_file() {
            return Ok(());
        }

        Err(anyhow!(
            "managed standalone Codex install not found at {}; install Codex first",
            self.managed_codex_bin.display()
        ))
    }

    fn backend_paths(&self, settings: &DaemonSettings) -> BackendPaths {
        self.backend_paths_with_bin(settings, &self.managed_codex_bin)
    }

    fn backend_paths_with_bin(
        &self,
        settings: &DaemonSettings,
        managed_codex_bin: &Path,
    ) -> BackendPaths {
        BackendPaths {
            codex_bin: managed_codex_bin.to_path_buf(),
            pid_file: self.pid_file.clone(),
            update_pid_file: self.update_pid_file.clone(),
            remote_control_enabled: settings.remote_control_enabled,
        }
    }

    async fn load_settings(&self) -> Result<DaemonSettings> {
        DaemonSettings::load(&self.settings_file).await
    }

    async fn acquire_operation_lock(&self) -> Result<tokio::fs::File> {
        let operation_lock = self.open_operation_lock_file().await?;
        let deadline = tokio::time::Instant::now() + OPERATION_LOCK_TIMEOUT;
        while !try_lock_file(&operation_lock)? {
            if tokio::time::Instant::now() >= deadline {
                return Err(anyhow!(
                    "timed out waiting for daemon operation lock {}",
                    self.operation_lock_file.display()
                ));
            }
            sleep(START_POLL_INTERVAL).await;
        }
        Ok(operation_lock)
    }

    async fn open_operation_lock_file(&self) -> Result<tokio::fs::File> {
        if let Some(parent) = self.operation_lock_file.parent() {
            tokio::fs::create_dir_all(parent).await.with_context(|| {
                format!(
                    "failed to create daemon state directory {}",
                    parent.display()
                )
            })?;
        }
        tokio::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&self.operation_lock_file)
            .await
            .with_context(|| {
                format!(
                    "failed to open daemon operation lock {}",
                    self.operation_lock_file.display()
                )
            })
    }

    fn output(
        &self,
        status: LifecycleStatus,
        backend: Option<BackendKind>,
        pid: Option<u32>,
        app_server_version: Option<String>,
    ) -> LifecycleOutput {
        LifecycleOutput {
            status,
            backend,
            pid,
            socket_path: self.socket_path.clone(),
            cli_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            app_server_version,
        }
    }

    fn remote_control_output(
        &self,
        status: RemoteControlStatus,
        backend: Option<BackendKind>,
        remote_control_enabled: bool,
        app_server_version: Option<String>,
    ) -> RemoteControlOutput {
        RemoteControlOutput {
            status,
            backend,
            remote_control_enabled,
            socket_path: self.socket_path.clone(),
            cli_version: env!("CARGO_PKG_VERSION").to_string(),
            app_server_version,
        }
    }
}

fn remote_control_status(mode: RemoteControlMode) -> RemoteControlStatus {
    match mode {
        RemoteControlMode::Enabled => RemoteControlStatus::Enabled,
        RemoteControlMode::Disabled => RemoteControlStatus::Disabled,
    }
}

fn already_remote_control_status(mode: RemoteControlMode) -> RemoteControlStatus {
    match mode {
        RemoteControlMode::Enabled => RemoteControlStatus::AlreadyEnabled,
        RemoteControlMode::Disabled => RemoteControlStatus::AlreadyDisabled,
    }
}

#[cfg(unix)]
fn restart_decision(
    mode: RestartMode,
    info: Option<&client::ProbeInfo>,
    managed_version: Option<&str>,
) -> RestartDecision {
    match (mode, info, managed_version) {
        (RestartMode::IfVersionChanged, None, _) => RestartDecision::NotReady,
        (RestartMode::IfVersionChanged, Some(info), Some(managed_version))
            if info.app_server_version == managed_version =>
        {
            RestartDecision::AlreadyCurrent
        }
        _ => RestartDecision::Restart,
    }
}

#[cfg(unix)]
fn should_reexec_updater(
    updater_refresh_mode: UpdaterRefreshMode,
    outcome: RestartIfRunningOutcome,
) -> bool {
    updater_refresh_mode == UpdaterRefreshMode::ReexecIfManagedBinaryChanged
        && outcome == RestartIfRunningOutcome::Restarted
}

#[cfg(unix)]
fn try_lock_file(file: &tokio::fs::File) -> Result<bool> {
    use std::os::fd::AsRawFd;

    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if result == 0 {
        return Ok(true);
    }

    let err = std::io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::EWOULDBLOCK) {
        return Ok(false);
    }
    Err(err).context("failed to lock daemon operation")
}

#[cfg(not(unix))]
fn try_lock_file(_file: &tokio::fs::File) -> Result<bool> {
    Ok(true)
}

#[cfg(all(test, unix))]
mod tests {
    use pretty_assertions::assert_eq;

    use super::BootstrapStatus;
    use super::LifecycleStatus;
    use super::RemoteControlStatus;
    use super::RestartDecision;
    use super::RestartIfRunningOutcome;
    use super::RestartMode;
    use super::UpdaterRefreshMode;
    use super::restart_decision;
    use super::should_reexec_updater;
    use crate::client::ProbeInfo;

    #[test]
    fn lifecycle_status_uses_camel_case_json() {
        assert_eq!(
            serde_json::to_string(&LifecycleStatus::AlreadyRunning).expect("serialize"),
            "\"alreadyRunning\""
        );
    }

    #[test]
    fn bootstrap_status_uses_camel_case_json() {
        assert_eq!(
            serde_json::to_string(&BootstrapStatus::Bootstrapped).expect("serialize"),
            "\"bootstrapped\""
        );
    }

    #[test]
    fn remote_control_status_uses_camel_case_json() {
        assert_eq!(
            serde_json::to_string(&RemoteControlStatus::AlreadyEnabled).expect("serialize"),
            "\"alreadyEnabled\""
        );
    }

    #[test]
    fn updater_reexec_waits_for_validated_restart() {
        assert_eq!(
            [
                RestartIfRunningOutcome::Busy,
                RestartIfRunningOutcome::NotReady,
                RestartIfRunningOutcome::AlreadyCurrent,
                RestartIfRunningOutcome::NotRunning,
                RestartIfRunningOutcome::Restarted,
            ]
            .map(|outcome| {
                should_reexec_updater(UpdaterRefreshMode::ReexecIfManagedBinaryChanged, outcome)
            }),
            [false, false, false, false, true]
        );
    }

    #[test]
    fn unchanged_updater_never_reexecs() {
        assert_eq!(
            [
                RestartIfRunningOutcome::Busy,
                RestartIfRunningOutcome::NotReady,
                RestartIfRunningOutcome::AlreadyCurrent,
                RestartIfRunningOutcome::NotRunning,
                RestartIfRunningOutcome::Restarted,
            ]
            .map(|outcome| should_reexec_updater(UpdaterRefreshMode::None, outcome)),
            [false, false, false, false, false]
        );
    }

    #[test]
    fn restart_decision_preserves_forced_refreshes() {
        let current_info = ProbeInfo {
            app_server_version: "0.1.0".to_string(),
        };

        assert_eq!(
            [
                restart_decision(
                    RestartMode::IfVersionChanged,
                    Some(&current_info),
                    Some("0.1.0"),
                ),
                restart_decision(
                    RestartMode::IfVersionChanged,
                    /*info*/ None,
                    /*managed_version*/ None,
                ),
                restart_decision(RestartMode::Always, Some(&current_info), Some("0.1.0")),
                restart_decision(
                    RestartMode::Always,
                    /*info*/ None,
                    /*managed_version*/ None
                ),
            ],
            [
                RestartDecision::AlreadyCurrent,
                RestartDecision::NotReady,
                RestartDecision::Restart,
                RestartDecision::Restart,
            ]
        );
    }
}
