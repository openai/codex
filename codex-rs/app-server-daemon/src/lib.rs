mod backend;
mod client;
mod managed_install;
mod settings;

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use backend::Backend;
pub use backend::BackendKind;
use backend::BackendPaths;
use codex_app_server::app_server_control_socket_path;
use codex_core::config::find_codex_home;
use managed_install::managed_codex_bin;
use managed_install::preferred_codex_bin;
use serde::Serialize;
use settings::DaemonSettings;
use tokio::time::sleep;

const START_POLL_INTERVAL: Duration = Duration::from_millis(50);
const START_TIMEOUT: Duration = Duration::from_secs(10);
const PID_FILE_NAME: &str = "app-server.pid";
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

pub async fn run(command: LifecycleCommand) -> Result<LifecycleOutput> {
    Daemon::from_environment()?.run(command).await
}

pub async fn bootstrap(options: BootstrapOptions) -> Result<BootstrapOutput> {
    Daemon::from_environment()?.bootstrap(options).await
}

struct Daemon {
    codex_home: PathBuf,
    socket_path: PathBuf,
    current_exe: PathBuf,
    pid_file: PathBuf,
    settings_file: PathBuf,
    managed_codex_bin: PathBuf,
}

impl Daemon {
    fn from_environment() -> Result<Self> {
        let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
        let socket_path = app_server_control_socket_path(codex_home.as_path())?
            .as_path()
            .to_path_buf();
        let current_exe =
            std::env::current_exe().context("failed to resolve current executable")?;
        let state_dir = codex_home.as_path().join(STATE_DIR_NAME);
        Ok(Self {
            codex_home: codex_home.as_path().to_path_buf(),
            socket_path,
            current_exe,
            pid_file: state_dir.join(PID_FILE_NAME),
            settings_file: state_dir.join(SETTINGS_FILE_NAME),
            managed_codex_bin: managed_codex_bin(codex_home.as_path()),
        })
    }

    async fn run(&self, command: LifecycleCommand) -> Result<LifecycleOutput> {
        match command {
            LifecycleCommand::Start => self.start().await,
            LifecycleCommand::Restart => self.restart().await,
            LifecycleCommand::Stop => self.stop().await,
            LifecycleCommand::Version => self.version().await,
        }
    }

    async fn start(&self) -> Result<LifecycleOutput> {
        let settings = self.load_settings().await?;
        if let Ok(info) = client::probe(&self.socket_path).await {
            return Ok(self.output(
                LifecycleStatus::AlreadyRunning,
                self.running_backend(&settings).await?,
                None,
                Some(info.app_server_version),
            ));
        }

        if let Some(backend) = self.running_backend_instance(&settings).await? {
            let info = self.wait_until_ready().await?;
            return Ok(self.output(
                LifecycleStatus::AlreadyRunning,
                Some(backend.kind()),
                None,
                Some(info.app_server_version),
            ));
        }

        let (backend, pid) = self.start_managed_backend(&settings).await?;
        let info = self.wait_until_ready().await?;
        Ok(self.output(
            LifecycleStatus::Started,
            Some(backend.kind()),
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

        if let Some(backend) = self.running_backend_instance(&settings).await? {
            backend.stop().await?;
        }

        let (backend, pid) = self.start_managed_backend(&settings).await?;
        let info = self.wait_until_ready().await?;
        Ok(self.output(
            LifecycleStatus::Restarted,
            Some(backend.kind()),
            pid,
            Some(info.app_server_version),
        ))
    }

    async fn stop(&self) -> Result<LifecycleOutput> {
        let settings = self.load_settings().await?;
        if let Some(backend) = self.running_backend_instance(&settings).await? {
            let kind = backend.kind();
            backend.stop().await?;
            return Ok(self.output(LifecycleStatus::Stopped, Some(kind), None, None));
        }

        if client::probe(&self.socket_path).await.is_ok() {
            return Err(anyhow!(
                "app server is running but is not managed by codex app-server daemon"
            ));
        }

        Ok(self.output(LifecycleStatus::NotRunning, None, None, None))
    }

    async fn version(&self) -> Result<LifecycleOutput> {
        let settings = self.load_settings().await?;
        let info = client::probe(&self.socket_path).await?;
        Ok(self.output(
            LifecycleStatus::Running,
            self.running_backend(&settings).await?,
            None,
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
        if !self.managed_codex_bin.is_file() {
            return Err(anyhow!(
                "managed standalone Codex install not found at {}; install Codex first",
                self.managed_codex_bin.display()
            ));
        }

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

        let (backend, auto_update_enabled) = if backend::SystemdBackend::is_available().await {
            backend::SystemdBackend::bootstrap(
                &self.codex_home,
                &self.managed_codex_bin,
                settings.remote_control_enabled,
            )
            .await?;
            (
                Backend::Systemd(backend::SystemdBackend::new(
                    self.managed_codex_bin.clone(),
                    settings.remote_control_enabled,
                )),
                true,
            )
        } else {
            if let Some(backend) = self.running_backend_instance(&settings).await? {
                backend.stop().await?;
            }
            let fallback = backend::pid_backend(self.backend_paths(&settings));
            fallback.start().await?;
            (fallback, false)
        };

        let info = self.wait_until_ready().await?;
        Ok(BootstrapOutput {
            status: BootstrapStatus::Bootstrapped,
            backend: backend.kind(),
            auto_update_enabled,
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
            .map(|backend| backend.kind()))
    }

    async fn running_backend_instance(&self, settings: &DaemonSettings) -> Result<Option<Backend>> {
        for backend in backend::managed_backends(self.backend_paths(settings)) {
            if backend.is_running().await? {
                return Ok(Some(backend));
            }
        }
        Ok(None)
    }

    async fn start_managed_backend(
        &self,
        settings: &DaemonSettings,
    ) -> Result<(Backend, Option<u32>)> {
        let backend = backend::preferred_backend(self.backend_paths(settings)).await;
        match backend.start().await {
            Ok(pid) => Ok((backend, pid)),
            Err(systemd_err) if backend.kind() == BackendKind::SystemdUser => {
                let fallback = backend::pid_backend(self.backend_paths(settings));
                let pid = fallback.start().await.with_context(|| {
                    format!(
                        "failed to start app server through user systemd ({systemd_err}); pid fallback also failed"
                    )
                })?;
                Ok((fallback, pid))
            }
            Err(err) => Err(err),
        }
    }

    fn backend_paths(&self, settings: &DaemonSettings) -> BackendPaths {
        BackendPaths {
            codex_bin: preferred_codex_bin(&self.codex_home, self.current_exe.clone()),
            pid_file: self.pid_file.clone(),
            remote_control_enabled: settings.remote_control_enabled,
        }
    }

    async fn load_settings(&self) -> Result<DaemonSettings> {
        DaemonSettings::load(&self.settings_file).await
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
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::BootstrapStatus;
    use super::LifecycleStatus;

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
}
