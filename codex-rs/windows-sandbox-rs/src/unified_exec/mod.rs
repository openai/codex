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
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_pty::SpawnedProcess;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

/// Fully resolved Windows sandbox session launch request.
///
/// Callers should parse their own input shape first, then use this request to
/// share the elevated-vs-legacy backend selection and session launch path.
pub struct WindowsSandboxSessionRequest<'a> {
    pub permission_profile: &'a PermissionProfile,
    pub workspace_roots: &'a [AbsolutePathBuf],
    pub codex_home: &'a Path,
    pub command: Vec<String>,
    pub cwd: &'a Path,
    pub env_map: HashMap<String, String>,
    pub windows_sandbox_level: WindowsSandboxLevel,
    pub proxy_enforced: bool,
    pub timeout_ms: Option<u64>,
    pub read_roots_override: Option<&'a [PathBuf]>,
    pub read_roots_include_platform_defaults: bool,
    pub write_roots_override: Option<&'a [PathBuf]>,
    pub deny_read_paths_override: &'a [AbsolutePathBuf],
    pub deny_write_paths_override: &'a [AbsolutePathBuf],
    pub tty: bool,
    pub stdin_open: bool,
    pub use_private_desktop: bool,
}

pub async fn spawn_windows_sandbox_session_for_level(
    request: WindowsSandboxSessionRequest<'_>,
) -> Result<SpawnedProcess> {
    if request.proxy_enforced
        || matches!(request.windows_sandbox_level, WindowsSandboxLevel::Elevated)
    {
        spawn_windows_sandbox_session_elevated_for_permission_profile(
            request.permission_profile,
            request.workspace_roots,
            request.codex_home,
            request.command,
            request.cwd,
            request.env_map,
            request.proxy_enforced,
            request.timeout_ms,
            request.read_roots_override,
            request.read_roots_include_platform_defaults,
            request.write_roots_override,
            request.deny_read_paths_override,
            request.deny_write_paths_override,
            request.tty,
            request.stdin_open,
            request.use_private_desktop,
        )
        .await
    } else {
        spawn_windows_sandbox_session_legacy(
            request.permission_profile,
            request.workspace_roots,
            request.codex_home,
            request.command,
            request.cwd,
            request.env_map,
            request.timeout_ms,
            request.deny_read_paths_override,
            request.deny_write_paths_override,
            request.tty,
            request.stdin_open,
            request.use_private_desktop,
        )
        .await
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn spawn_windows_sandbox_session_legacy(
    permission_profile: &PermissionProfile,
    workspace_roots: &[AbsolutePathBuf],
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
        permission_profile,
        workspace_roots,
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
pub async fn spawn_windows_sandbox_session_elevated_for_permission_profile(
    permission_profile: &PermissionProfile,
    workspace_roots: &[AbsolutePathBuf],
    codex_home: &Path,
    command: Vec<String>,
    cwd: &Path,
    env_map: HashMap<String, String>,
    proxy_enforced: bool,
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
    backends::elevated::spawn_windows_sandbox_session_elevated_for_permission_profile(
        permission_profile,
        workspace_roots,
        codex_home,
        command,
        cwd,
        env_map,
        proxy_enforced,
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
