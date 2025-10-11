use crate::config_types::SandboxWorkspaceWrite;
use crate::config_types::WindowsConfig;
use crate::exec::SandboxType;
use crate::protocol::SandboxPolicy;
use crate::spawn::StdioPolicy;
use crate::spawn::spawn_child_async;
use dunce::canonicalize;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command as StdCommand;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tokio::process::Child;
use tracing::info;

static WSL_AVAILABLE: OnceLock<bool> = OnceLock::new();
static NOTICE_PRINTED: AtomicBool = AtomicBool::new(false);

pub(crate) fn preferred_windows_sandbox(config: &WindowsConfig) -> Option<SandboxType> {
    if prefer_wsl(config) {
        Some(SandboxType::WindowsWslWithLinuxSeccomp)
    } else {
        None
    }
}

pub(crate) fn prefer_wsl(config: &WindowsConfig) -> bool {
    config.prefer_wsl && is_wsl_available()
}

pub(crate) fn is_wsl_available() -> bool {
    *WSL_AVAILABLE.get_or_init(run_wsl_status)
}

pub(crate) async fn spawn_command_under_windows_wsl(
    command: Vec<String>,
    command_cwd: PathBuf,
    sandbox_policy: &SandboxPolicy,
    _sandbox_policy_cwd: &Path,
    stdio_policy: StdioPolicy,
    env: HashMap<String, String>,
    windows_config: &WindowsConfig,
) -> io::Result<Child> {
    if !prefer_wsl(windows_config) {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "WSL sandboxing requested but prefer_wsl is disabled or WSL is unavailable",
        ));
    }

    maybe_print_notice(windows_config);

    let mut args: Vec<String> = Vec::new();

    if let Ok(cd_path) = convert_path_to_wsl(&command_cwd) {
        args.push("--cd".to_string());
        args.push(cd_path);
    }

    let exe_path = std::env::current_exe().map_err(|err| {
        io::Error::other(format!("failed to resolve codex executable path: {err}"))
    })?;
    let exe_in_wsl = convert_path_to_wsl(exe_path.as_path())?;
    args.push("-e".to_string());
    args.push(exe_in_wsl);
    args.push("sandbox".to_string());
    args.push("landlock".to_string());

    for override_value in sandbox_overrides_for_wsl(sandbox_policy)? {
        args.push("-c".to_string());
        args.push(override_value);
    }

    args.push("--".to_string());
    args.extend(command);

    spawn_child_async(
        PathBuf::from("wsl.exe"),
        args,
        None,
        command_cwd,
        sandbox_policy,
        stdio_policy,
        env,
    )
    .await
}

fn maybe_print_notice(config: &WindowsConfig) {
    if config.hide_wsl_notice {
        return;
    }
    if !NOTICE_PRINTED.swap(true, Ordering::SeqCst) {
        info!("Detected prefer_wsl=true; relaunching command inside WSL for sandboxed execution.");
    }
}

pub(crate) fn sandbox_overrides_for_wsl(sandbox_policy: &SandboxPolicy) -> io::Result<Vec<String>> {
    let mode = sandbox_mode_label(sandbox_policy)?;
    let mut overrides = vec![format!("sandbox_mode=\"{mode}\"")];

    if let SandboxPolicy::WorkspaceWrite {
        writable_roots,
        network_access,
        exclude_tmpdir_env_var,
        exclude_slash_tmp,
    } = sandbox_policy
    {
        let mut workspace = SandboxWorkspaceWrite {
            writable_roots: Vec::new(),
            network_access: *network_access,
            exclude_tmpdir_env_var: *exclude_tmpdir_env_var,
            exclude_slash_tmp: *exclude_slash_tmp,
        };

        for root in writable_roots {
            let converted = convert_path_to_wsl(root)?;
            workspace.writable_roots.push(PathBuf::from(converted));
        }

        let json = serde_json::to_string(&workspace).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to serialize sandbox workspace: {err}"),
            )
        })?;
        overrides.push(format!("sandbox_workspace_write={json}"));
    }

    Ok(overrides)
}

fn sandbox_mode_label(sandbox_policy: &SandboxPolicy) -> io::Result<String> {
    let value = serde_json::to_value(sandbox_policy).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to serialize sandbox policy: {err}"),
        )
    })?;
    value
        .get("mode")
        .and_then(JsonValue::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "serialized sandbox policy missing mode",
            )
        })
}

pub(crate) fn convert_path_to_wsl(path: &Path) -> io::Result<String> {
    if let Some(raw) = path.to_str().filter(|raw| is_wsl_path(raw)) {
        return Ok(raw.to_string());
    }

    let absolute = to_absolute_path(path)?;
    let simplified = canonicalize(&absolute).unwrap_or(absolute);
    run_wslpath(&simplified)
}

fn to_absolute_path(path: &Path) -> io::Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        let cwd = std::env::current_dir()?;
        Ok(cwd.join(path))
    }
}

fn run_wsl_status() -> bool {
    StdCommand::new("wsl")
        .arg("--status")
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn run_wslpath(path: &Path) -> io::Result<String> {
    let raw = path.to_string_lossy().to_string();
    let output = StdCommand::new("wsl")
        .arg("wslpath")
        .arg("-a")
        .arg(&raw)
        .output()
        .map_err(|err| io::Error::other(format!("failed to invoke wslpath for {raw}: {err}")))?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "wslpath exited with {} while converting {raw}",
            output.status
        )));
    }
    String::from_utf8(output.stdout)
        .map(|out| out.trim().to_string())
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

fn is_wsl_path(raw: &str) -> bool {
    raw.starts_with("/mnt/") || raw.starts_with("\\\\wsl$")
}
