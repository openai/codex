use std::path::PathBuf;

use codex_utils_absolute_path::AbsolutePathBuf;

/// Runtime paths needed by exec-server child processes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecServerRuntimePaths {
    /// Stable path to the Codex executable used to launch hidden helper modes.
    pub codex_self_exe: AbsolutePathBuf,
}

impl ExecServerRuntimePaths {
    pub fn from_optional_paths(codex_self_exe: Option<PathBuf>) -> std::io::Result<Self> {
        let codex_self_exe = codex_self_exe.ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Codex executable path is not configured",
            )
        })?;
        Self::new(codex_self_exe)
    }

    pub fn new(codex_self_exe: PathBuf) -> std::io::Result<Self> {
        Ok(Self {
            codex_self_exe: absolute_path(codex_self_exe)?,
        })
    }
}

fn absolute_path(path: PathBuf) -> std::io::Result<AbsolutePathBuf> {
    AbsolutePathBuf::from_absolute_path(path.as_path())
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))
}
