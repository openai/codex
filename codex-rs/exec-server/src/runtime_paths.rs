use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use codex_install_context::InstallContext;
use codex_utils_absolute_path::AbsolutePathBuf;

/// Runtime paths needed by exec-server child processes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecServerRuntimePaths {
    /// Stable path to the Codex executable used to launch hidden helper modes.
    pub codex_self_exe: AbsolutePathBuf,
    /// Path to the Linux sandbox helper alias used when the platform sandbox
    /// needs to re-enter Codex by argv0.
    pub codex_linux_sandbox_exe: Option<AbsolutePathBuf>,
    package_path_dir: Option<AbsolutePathBuf>,
}

impl ExecServerRuntimePaths {
    pub fn from_optional_paths(
        codex_self_exe: Option<PathBuf>,
        codex_linux_sandbox_exe: Option<PathBuf>,
    ) -> std::io::Result<Self> {
        let codex_self_exe = codex_self_exe.ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Codex executable path is not configured",
            )
        })?;
        Self::new(codex_self_exe, codex_linux_sandbox_exe)
    }

    pub fn new(
        codex_self_exe: PathBuf,
        codex_linux_sandbox_exe: Option<PathBuf>,
    ) -> std::io::Result<Self> {
        let codex_self_exe = absolute_path(codex_self_exe)?;
        let package_path_dir = InstallContext::from_exe(
            cfg!(target_os = "macos"),
            Some(codex_self_exe.as_path()),
            /*method_override*/ None,
        )
        .package_layout
        .and_then(|layout| layout.path_dir);
        Ok(Self {
            codex_self_exe,
            codex_linux_sandbox_exe: codex_linux_sandbox_exe.map(absolute_path).transpose()?,
            package_path_dir,
        })
    }

    pub(crate) fn apply_to_env(&self, env: &mut HashMap<String, String>) {
        if let Some(package_path_dir) = &self.package_path_dir {
            prepend_package_path(env, package_path_dir.as_path());
        }
    }
}

fn prepend_package_path(env: &mut HashMap<String, String>, package_path_dir: &Path) {
    let path_key = env
        .keys()
        .find(|key| *key == "PATH" || (cfg!(windows) && key.eq_ignore_ascii_case("PATH")))
        .cloned();
    let Some(path_key) = path_key else {
        return;
    };
    let Some(current_path) = env.get(&path_key) else {
        return;
    };
    if current_path.is_empty() {
        return;
    }

    let entries = std::iter::once(package_path_dir.to_path_buf())
        .chain(std::env::split_paths(current_path).filter(|entry| entry != package_path_dir));
    if let Ok(path) = std::env::join_paths(entries) {
        env.insert(path_key, path.to_string_lossy().into_owned());
    }
}

fn absolute_path(path: PathBuf) -> std::io::Result<AbsolutePathBuf> {
    AbsolutePathBuf::from_absolute_path(path.as_path())
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))
}

#[cfg(test)]
#[path = "runtime_paths_tests.rs"]
mod tests;
