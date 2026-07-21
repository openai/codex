use std::collections::HashMap;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_pty::SpawnedProcess;
use codex_utils_pty::TerminalSize;

use crate::SandboxType;
use crate::WindowsSandboxFilesystemOverrides;
use crate::WindowsSandboxProxySettingsMode;

/// Windows-specific inputs for an executor-native process spawn.
pub struct WindowsSandboxSpawnRequest<'a> {
    pub permission_profile: &'a PermissionProfile,
    pub workspace_roots: &'a [AbsolutePathBuf],
    pub windows_sandbox_level: WindowsSandboxLevel,
    pub proxy_enforced: bool,
    pub network_proxy_restricting_sid: Option<&'a str>,
    pub proxy_settings_mode: WindowsSandboxProxySettingsMode,
    pub filesystem_overrides: Option<&'a WindowsSandboxFilesystemOverrides>,
    pub use_private_desktop: bool,
}

/// Executor-native process launch request shared by local and exec-server execution.
pub struct SpawnRequest<'a> {
    pub command: &'a [String],
    pub cwd: &'a Path,
    pub env: &'a HashMap<String, String>,
    pub arg0: &'a Option<String>,
    pub sandbox: SandboxType,
    pub windows_sandbox: Option<WindowsSandboxSpawnRequest<'a>>,
    pub tty: bool,
    pub stdin_open: bool,
    pub inherited_fds: &'a [i32],
}

/// Spawn a process using the backend selected by the prepared sandbox request.
pub async fn spawn_process(request: SpawnRequest<'_>) -> Result<SpawnedProcess> {
    if request.sandbox == SandboxType::WindowsRestrictedToken {
        #[cfg(target_os = "windows")]
        {
            let windows = request
                .windows_sandbox
                .context("missing Windows sandbox spawn request")?;
            let codex_home = codex_utils_home_dir::find_codex_home()
                .context("windows sandbox: failed to resolve codex_home")?;
            let empty_paths = &[];
            let overrides = windows.filesystem_overrides;

            return codex_windows_sandbox::spawn_windows_sandbox_session_for_level(
                codex_windows_sandbox::WindowsSandboxSessionRequest {
                    permission_profile: windows.permission_profile,
                    workspace_roots: windows.workspace_roots,
                    codex_home: codex_home.as_path(),
                    command: request.command.to_vec(),
                    cwd: request.cwd,
                    env_map: request.env.clone(),
                    windows_sandbox_level: windows.windows_sandbox_level,
                    proxy_enforced: windows.proxy_enforced,
                    network_proxy_restricting_sid: windows
                        .network_proxy_restricting_sid
                        .map(str::to_owned),
                    proxy_settings_mode: windows.proxy_settings_mode,
                    timeout_ms: None,
                    read_roots_override: overrides
                        .and_then(|value| value.read_roots_override.as_deref()),
                    read_roots_include_platform_defaults: overrides
                        .is_some_and(|value| value.read_roots_include_platform_defaults),
                    write_roots_override: overrides
                        .and_then(|value| value.write_roots_override.as_deref()),
                    deny_read_paths_override: overrides.map_or(empty_paths, |value| {
                        value.additional_deny_read_paths.as_slice()
                    }),
                    deny_write_paths_override: overrides.map_or(empty_paths, |value| {
                        value.additional_deny_write_paths.as_slice()
                    }),
                    tty: request.tty,
                    stdin_open: request.stdin_open,
                    use_private_desktop: windows.use_private_desktop,
                },
            )
            .await;
        }

        #[cfg(not(target_os = "windows"))]
        anyhow::bail!("Windows sandbox process spawn is unavailable on this platform");
    }

    let (program, args) = request
        .command
        .split_first()
        .context("missing program for process spawn")?;
    if request.tty {
        codex_utils_pty::pty::spawn_process(
            program,
            args,
            request.cwd,
            request.env,
            request.arg0,
            TerminalSize::default(),
            request.inherited_fds,
        )
        .await
    } else if request.stdin_open {
        codex_utils_pty::pipe::spawn_process(
            program,
            args,
            request.cwd,
            request.env,
            request.arg0,
            request.inherited_fds,
        )
        .await
    } else {
        codex_utils_pty::pipe::spawn_process_no_stdin(
            program,
            args,
            request.cwd,
            request.env,
            request.arg0,
            request.inherited_fds,
        )
        .await
    }
}
