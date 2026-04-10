use std::ffi::OsString;
use std::path::PathBuf;

use crate::CODEX_FS_HELPER_ARG0;

#[cfg(target_os = "linux")]
use codex_sandboxing::landlock::CODEX_LINUX_SANDBOX_ARG0;

/// Runtime paths for helper aliases that are created by the top-level `codex`
/// arg0 dispatcher and needed by exec-server child processes.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ExecServerRuntimePaths {
    /// Path to the `codex-fs` helper alias used for sandboxed filesystem
    /// operations.
    pub codex_fs_exe: Option<PathBuf>,
    /// Path to the Linux sandbox helper alias used when the platform sandbox
    /// needs to re-enter Codex by argv0.
    pub codex_linux_sandbox_exe: Option<PathBuf>,
}

impl ExecServerRuntimePaths {
    pub(crate) fn from_current_environment() -> Self {
        Self::from_path_env(std::env::var_os("PATH"))
    }

    fn from_path_env(path_env: Option<OsString>) -> Self {
        Self {
            codex_fs_exe: find_program_in_path(path_env.as_ref(), CODEX_FS_HELPER_ARG0),
            codex_linux_sandbox_exe: {
                #[cfg(target_os = "linux")]
                {
                    find_program_in_path(path_env.as_ref(), CODEX_LINUX_SANDBOX_ARG0)
                }
                #[cfg(not(target_os = "linux"))]
                {
                    None
                }
            },
        }
    }
}

fn find_program_in_path(path_env: Option<&OsString>, program: &str) -> Option<PathBuf> {
    let path_env = path_env?;
    std::env::split_paths(path_env)
        .flat_map(|dir| program_candidates(&dir, program))
        .find(|path| path.is_file())
}

#[cfg(windows)]
fn program_candidates(dir: &std::path::Path, program: &str) -> Vec<PathBuf> {
    vec![
        dir.join(format!("{program}.bat")),
        dir.join(format!("{program}.exe")),
        dir.join(program),
    ]
}

#[cfg(not(windows))]
fn program_candidates(dir: &std::path::Path, program: &str) -> Vec<PathBuf> {
    vec![dir.join(program)]
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn path_env_for(dir: &std::path::Path) -> OsString {
        std::env::join_paths([dir]).expect("join path")
    }

    #[cfg(windows)]
    fn helper_path(dir: &std::path::Path, program: &str) -> PathBuf {
        dir.join(format!("{program}.bat"))
    }

    #[cfg(not(windows))]
    fn helper_path(dir: &std::path::Path, program: &str) -> PathBuf {
        dir.join(program)
    }

    #[test]
    fn discovers_fs_helper_from_path_env() -> std::io::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let helper = helper_path(temp_dir.path(), CODEX_FS_HELPER_ARG0);
        std::fs::write(&helper, "")?;

        let runtime_paths =
            ExecServerRuntimePaths::from_path_env(Some(path_env_for(temp_dir.path())));

        assert_eq!(
            runtime_paths,
            ExecServerRuntimePaths {
                codex_fs_exe: Some(helper),
                codex_linux_sandbox_exe: None,
            }
        );
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn discovers_linux_sandbox_helper_from_path_env() -> std::io::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let helper = helper_path(temp_dir.path(), CODEX_LINUX_SANDBOX_ARG0);
        std::fs::write(&helper, "")?;

        let runtime_paths =
            ExecServerRuntimePaths::from_path_env(Some(path_env_for(temp_dir.path())));

        assert_eq!(
            runtime_paths,
            ExecServerRuntimePaths {
                codex_fs_exe: None,
                codex_linux_sandbox_exe: Some(helper),
            }
        );
        Ok(())
    }
}
