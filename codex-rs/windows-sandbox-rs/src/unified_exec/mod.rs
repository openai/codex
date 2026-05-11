//! Unified exec session spawner for Windows sandboxing.
//!
//! This module is the thin orchestration layer for Windows unified-exec sessions.
//! Backend-specific mechanics live in sibling modules:
//! - `backends::legacy` adapts the direct restricted-token spawn path into a live session.
//! - `backends::elevated` adapts the elevated command-runner IPC path into the same session API.
//! - `backends::windows_common` holds the small shared Windows backend helpers
//!   used by both.

mod backends;

use anyhow::Result;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_pty::SpawnedProcess;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

#[allow(clippy::too_many_arguments)]
pub async fn spawn_windows_sandbox_session_legacy(
    policy_json_or_preset: &str,
    sandbox_policy_cwd: &Path,
    codex_home: &Path,
    command: Vec<String>,
    cwd: &Path,
    env_map: HashMap<String, String>,
    timeout_ms: Option<u64>,
    additional_deny_read_paths: &[AbsolutePathBuf],
    additional_deny_write_paths: &[AbsolutePathBuf],
    tty: bool,
    stdin_open: bool,
    use_private_desktop: bool,
) -> Result<SpawnedProcess> {
    backends::legacy::spawn_windows_sandbox_session_legacy(
        policy_json_or_preset,
        sandbox_policy_cwd,
        codex_home,
        command,
        cwd,
        env_map,
        timeout_ms,
        additional_deny_read_paths,
        additional_deny_write_paths,
        tty,
        stdin_open,
        use_private_desktop,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn spawn_windows_sandbox_session_elevated(
    policy_json_or_preset: &str,
    sandbox_policy_cwd: &Path,
    codex_home: &Path,
    command: Vec<String>,
    cwd: &Path,
    env_map: HashMap<String, String>,
    timeout_ms: Option<u64>,
    read_roots_override: Option<&[PathBuf]>,
    read_roots_include_platform_defaults: bool,
    write_roots_override: Option<&[PathBuf]>,
    deny_read_paths_override: &[AbsolutePathBuf],
    deny_write_paths_override: &[AbsolutePathBuf],
    tty: bool,
    stdin_open: bool,
    use_private_desktop: bool,
) -> Result<SpawnedProcess> {
    backends::elevated::spawn_windows_sandbox_session_elevated(
        policy_json_or_preset,
        sandbox_policy_cwd,
        codex_home,
        command,
        cwd,
        env_map,
        timeout_ms,
        read_roots_override,
        read_roots_include_platform_defaults,
        write_roots_override,
        deny_read_paths_override,
        deny_write_paths_override,
        tty,
        stdin_open,
        use_private_desktop,
    )
    .await
}

#[cfg(test)]
pub(crate) use backends::windows_common::finish_driver_spawn;
#[cfg(test)]
pub(crate) use backends::windows_common::make_runner_resizer;
#[cfg(test)]
pub(crate) use backends::windows_common::start_runner_pipe_writer;
#[cfg(test)]
pub(crate) use backends::windows_common::start_runner_stdin_writer;

#[cfg(test)]
mod tests;
