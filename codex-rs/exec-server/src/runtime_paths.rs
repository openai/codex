use std::path::PathBuf;

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
