use std::io;
use std::path::Path;
use std::path::PathBuf;

#[cfg(target_os = "linux")]
use codex_sandboxing::landlock::CODEX_LINUX_SANDBOX_ARG0;
use codex_utils_absolute_path::AbsolutePathBuf;

/// Runtime paths needed by exec-server child processes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecServerRuntimePaths {
    /// Stable path to the Codex executable used to launch hidden helper modes.
    pub codex_self_exe: AbsolutePathBuf,
    /// Path to the Linux sandbox helper alias used when the platform sandbox
    /// needs to re-enter Codex by argv0.
    pub codex_linux_sandbox_exe: Option<PathBuf>,
}

impl ExecServerRuntimePaths {
    pub fn new(
        codex_self_exe: PathBuf,
        codex_linux_sandbox_exe: Option<PathBuf>,
    ) -> io::Result<Self> {
        Ok(Self {
            codex_self_exe: absolute_path(codex_self_exe.as_path())?,
            codex_linux_sandbox_exe,
        })
    }

    pub(crate) fn from_current_environment() -> io::Result<Self> {
        Self::new(
            std::env::current_exe()?,
            linux_sandbox_exe_from_path(std::env::var_os("PATH")),
        )
    }
}

fn absolute_path(path: &Path) -> io::Result<AbsolutePathBuf> {
    AbsolutePathBuf::from_absolute_path(path)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))
}

#[cfg(target_os = "linux")]
fn linux_sandbox_exe_from_path(path_env: Option<std::ffi::OsString>) -> Option<PathBuf> {
    find_program_in_path(path_env.as_ref(), CODEX_LINUX_SANDBOX_ARG0)
}

#[cfg(not(target_os = "linux"))]
fn linux_sandbox_exe_from_path(_path_env: Option<std::ffi::OsString>) -> Option<PathBuf> {
    None
}

#[cfg(target_os = "linux")]
fn find_program_in_path(path_env: Option<&std::ffi::OsString>, program: &str) -> Option<PathBuf> {
    let path_env = path_env?;
    std::env::split_paths(path_env)
        .map(|dir| dir.join(program))
        .find(|path| path.is_file())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn path_env_for(dir: &std::path::Path) -> std::ffi::OsString {
        std::env::join_paths([dir]).expect("join path")
    }

    #[test]
    fn discovers_linux_sandbox_helper_from_path_env() -> std::io::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let helper = temp_dir.path().join(CODEX_LINUX_SANDBOX_ARG0);
        std::fs::write(&helper, "")?;

        assert_eq!(
            linux_sandbox_exe_from_path(Some(path_env_for(temp_dir.path()))),
            Some(helper)
        );
        Ok(())
    }
}
