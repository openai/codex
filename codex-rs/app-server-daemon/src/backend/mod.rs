mod pid;

use std::path::PathBuf;

use serde::Serialize;

pub(crate) use pid::PidBackend;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BackendKind {
    Pid,
}

#[derive(Debug, Clone)]
pub(crate) struct BackendPaths {
    pub(crate) codex_bin: PathBuf,
    pub(crate) pid_file: PathBuf,
    pub(crate) update_pid_file: PathBuf,
    pub(crate) remote_control_enabled: bool,
}

pub(crate) fn pid_backend(paths: BackendPaths) -> PidBackend {
    PidBackend::new(
        paths.codex_bin,
        paths.pid_file,
        paths.remote_control_enabled,
    )
}

pub(crate) fn pid_update_loop_backend(paths: BackendPaths) -> PidBackend {
    PidBackend::new_update_loop(paths.codex_bin, paths.update_pid_file)
}
