use std::path::PathBuf;

use codex_sandboxing::LinuxSandboxPidNamespace;
use codex_utils_absolute_path::AbsolutePathBuf;

/// Paths and sandbox settings initialized when creating an executor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecServerRuntimePaths {
    /// Stable path to the Codex executable used to launch hidden helper modes.
    pub codex_self_exe: AbsolutePathBuf,
    /// Path to the Linux sandbox helper alias used when the platform sandbox
    /// needs to re-enter Codex by argv0.
    pub codex_linux_sandbox_exe: Option<AbsolutePathBuf>,
    /// Trusted startup policy; requests and repository config cannot change PID isolation.
    pub linux_sandbox_pid_namespace: LinuxSandboxPidNamespace,
    /// User-config opt-out of writable-root symlink checks beneath this host's home.
    #[cfg(target_os = "macos")]
    pub allowed_symlinked_codex_home: Option<AbsolutePathBuf>,
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
        Ok(Self {
            linux_sandbox_pid_namespace: LinuxSandboxPidNamespace::default(),
            codex_self_exe: absolute_path(codex_self_exe)?,
            codex_linux_sandbox_exe: codex_linux_sandbox_exe.map(absolute_path).transpose()?,
            #[cfg(target_os = "macos")]
            allowed_symlinked_codex_home: None,
        })
    }

    /// Applies the PID namespace policy chosen by trusted executor provisioning.
    pub fn with_linux_sandbox_pid_namespace(mut self, mode: LinuxSandboxPidNamespace) -> Self {
        self.linux_sandbox_pid_namespace = mode;
        self
    }

    /// Applies the symlink opt-in resolved by the execution host's config loader.
    #[cfg(target_os = "macos")]
    pub fn with_allowed_symlinked_codex_home(
        mut self,
        allowed_symlinked_codex_home: Option<AbsolutePathBuf>,
    ) -> Self {
        self.allowed_symlinked_codex_home = allowed_symlinked_codex_home;
        self
    }
}

fn absolute_path(path: PathBuf) -> std::io::Result<AbsolutePathBuf> {
    AbsolutePathBuf::from_absolute_path(path.as_path())
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))
}
