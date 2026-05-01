mod pid;
mod systemd;

use std::path::PathBuf;

use anyhow::Result;
use serde::Serialize;

pub(crate) use pid::PidBackend;
pub(crate) use systemd::SystemdBackend;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BackendKind {
    SystemdUser,
    Pid,
}

#[derive(Debug)]
pub(crate) enum Backend {
    Systemd(SystemdBackend),
    Pid(PidBackend),
}

impl Backend {
    pub(crate) fn kind(&self) -> BackendKind {
        match self {
            Self::Systemd(_) => BackendKind::SystemdUser,
            Self::Pid(_) => BackendKind::Pid,
        }
    }

    pub(crate) async fn is_starting_or_running(&self) -> Result<bool> {
        match self {
            Self::Systemd(backend) => backend.is_starting_or_running().await,
            Self::Pid(backend) => backend.is_starting_or_running().await,
        }
    }

    pub(crate) async fn start(&self) -> Result<Option<u32>> {
        match self {
            Self::Systemd(backend) => backend.start().await,
            Self::Pid(backend) => backend.start().await,
        }
    }

    pub(crate) async fn stop(&self) -> Result<()> {
        match self {
            Self::Systemd(backend) => backend.stop().await,
            Self::Pid(backend) => backend.stop().await,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BackendPaths {
    pub(crate) codex_home: PathBuf,
    pub(crate) codex_bin: PathBuf,
    pub(crate) pid_file: PathBuf,
    pub(crate) remote_control_enabled: bool,
}

pub(crate) async fn preferred_backend(paths: BackendPaths) -> Backend {
    if SystemdBackend::is_available().await {
        Backend::Systemd(SystemdBackend::new(
            paths.codex_home,
            paths.codex_bin,
            paths.remote_control_enabled,
        ))
    } else {
        Backend::Pid(PidBackend::new(
            paths.codex_bin,
            paths.pid_file,
            paths.remote_control_enabled,
        ))
    }
}

pub(crate) fn managed_backends(paths: BackendPaths) -> [Backend; 2] {
    [
        Backend::Systemd(SystemdBackend::new(
            paths.codex_home.clone(),
            paths.codex_bin.clone(),
            paths.remote_control_enabled,
        )),
        Backend::Pid(PidBackend::new(
            paths.codex_bin,
            paths.pid_file,
            paths.remote_control_enabled,
        )),
    ]
}

pub(crate) fn pid_backend(paths: BackendPaths) -> Backend {
    Backend::Pid(PidBackend::new(
        paths.codex_bin,
        paths.pid_file,
        paths.remote_control_enabled,
    ))
}
