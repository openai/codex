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

    pub(crate) async fn is_running(&self) -> Result<bool> {
        match self {
            Self::Systemd(backend) => backend.is_running().await,
            Self::Pid(backend) => backend.is_running().await,
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
    pub(crate) codex_bin: PathBuf,
    pub(crate) pid_file: PathBuf,
}

pub(crate) async fn preferred_backend(paths: BackendPaths) -> Backend {
    if SystemdBackend::is_available().await {
        Backend::Systemd(SystemdBackend::new(paths.codex_bin))
    } else {
        Backend::Pid(PidBackend::new(paths.codex_bin, paths.pid_file))
    }
}

pub(crate) fn managed_backends(paths: BackendPaths) -> [Backend; 2] {
    [
        Backend::Systemd(SystemdBackend::new(paths.codex_bin.clone())),
        Backend::Pid(PidBackend::new(paths.codex_bin, paths.pid_file)),
    ]
}

pub(crate) fn pid_backend(paths: BackendPaths) -> Backend {
    Backend::Pid(PidBackend::new(paths.codex_bin, paths.pid_file))
}
