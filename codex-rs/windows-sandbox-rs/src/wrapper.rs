//! Internal `codex.exe --run-as-windows-sandbox` wrapper.
//!
//! This gives direct-spawn callers an argv-shaped Windows sandbox launcher,
//! analogous to the macOS seatbelt and Linux sandbox wrapper paths. The wrapper
//! parses sandbox metadata from argv, launches the requested inner command in a
//! Windows sandbox session, and forwards stdio to that inner command.

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_utils_absolute_path::AbsolutePathBuf;

pub const CODEX_WINDOWS_SANDBOX_ARG1: &str = "--run-as-windows-sandbox";

const COMMAND_CWD_FLAG: &str = "--command-cwd";
const CODEX_HOME_FLAG: &str = "--codex-home";
const DENY_READ_PATHS_JSON_FLAG: &str = "--deny-read-paths-json";
const DENY_WRITE_PATHS_JSON_FLAG: &str = "--deny-write-paths-json";
const ENV_JSON_FLAG: &str = "--env-json";
const PERMISSION_PROFILE_FLAG: &str = "--permission-profile";
const PRIVATE_DESKTOP_FLAG: &str = "--windows-sandbox-private-desktop";
const PROXY_ENFORCED_FLAG: &str = "--proxy-enforced";
const READ_ROOTS_INCLUDE_PLATFORM_DEFAULTS_FLAG: &str = "--read-roots-include-platform-defaults";
const READ_ROOTS_JSON_FLAG: &str = "--read-roots-json";
const SANDBOX_LEVEL_FLAG: &str = "--windows-sandbox-level";
const WRITE_ROOTS_JSON_FLAG: &str = "--write-roots-json";
const WORKSPACE_ROOT_FLAG: &str = "--workspace-root";

#[allow(clippy::too_many_arguments)]
pub fn create_windows_sandbox_command_args_for_permission_profile(
    command: Vec<String>,
    command_cwd: &AbsolutePathBuf,
    workspace_roots: &[AbsolutePathBuf],
    env_map: &HashMap<String, String>,
    permission_profile: &PermissionProfile,
    windows_sandbox_level: WindowsSandboxLevel,
    windows_sandbox_private_desktop: bool,
    proxy_enforced: bool,
    read_roots_override: Option<&[PathBuf]>,
    read_roots_include_platform_defaults: bool,
    write_roots_override: Option<&[PathBuf]>,
    deny_read_paths_override: &[AbsolutePathBuf],
    deny_write_paths_override: &[AbsolutePathBuf],
    codex_home: &Path,
) -> Vec<String> {
    let permission_profile_json = serde_json::to_string(permission_profile)
        .unwrap_or_else(|err| panic!("failed to serialize permission profile: {err}"));
    let env_json = serde_json::to_string(env_map)
        .unwrap_or_else(|err| panic!("failed to serialize env: {err}"));
    let mut args = vec![
        CODEX_WINDOWS_SANDBOX_ARG1.to_string(),
        CODEX_HOME_FLAG.to_string(),
        codex_home.to_string_lossy().into_owned(),
        COMMAND_CWD_FLAG.to_string(),
        command_cwd.as_path().to_string_lossy().into_owned(),
        PERMISSION_PROFILE_FLAG.to_string(),
        permission_profile_json,
        ENV_JSON_FLAG.to_string(),
        env_json,
        SANDBOX_LEVEL_FLAG.to_string(),
        windows_sandbox_level.to_string(),
    ];
    let workspace_roots = if workspace_roots.is_empty() {
        std::slice::from_ref(command_cwd)
    } else {
        workspace_roots
    };
    for root in workspace_roots {
        args.push(WORKSPACE_ROOT_FLAG.to_string());
        args.push(root.as_path().to_string_lossy().into_owned());
    }
    if windows_sandbox_private_desktop {
        args.push(PRIVATE_DESKTOP_FLAG.to_string());
    }
    if proxy_enforced {
        args.push(PROXY_ENFORCED_FLAG.to_string());
    }
    if let Some(read_roots_override) = read_roots_override {
        push_json_arg(&mut args, READ_ROOTS_JSON_FLAG, &read_roots_override);
    }
    if read_roots_include_platform_defaults {
        args.push(READ_ROOTS_INCLUDE_PLATFORM_DEFAULTS_FLAG.to_string());
    }
    if let Some(write_roots_override) = write_roots_override {
        push_json_arg(&mut args, WRITE_ROOTS_JSON_FLAG, &write_roots_override);
    }
    if !deny_read_paths_override.is_empty() {
        push_json_arg(
            &mut args,
            DENY_READ_PATHS_JSON_FLAG,
            &deny_read_paths_override,
        );
    }
    if !deny_write_paths_override.is_empty() {
        push_json_arg(
            &mut args,
            DENY_WRITE_PATHS_JSON_FLAG,
            &deny_write_paths_override,
        );
    }
    args.push("--".to_string());
    args.extend(command);
    args
}

fn push_json_arg<T: serde::Serialize>(args: &mut Vec<String>, flag: &str, value: &T) {
    args.push(flag.to_string());
    args.push(
        serde_json::to_string(value)
            .unwrap_or_else(|err| panic!("failed to serialize {flag}: {err}")),
    );
}

pub fn run_windows_sandbox_wrapper_main() -> ! {
    let args = std::env::args().skip(2).collect::<Vec<_>>();
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("windows sandbox failed to build runtime: {err}");
            std::process::exit(1);
        }
    };
    let exit_code = match runtime.block_on(run_windows_sandbox_wrapper_args(args)) {
        Ok(exit_code) => exit_code,
        Err(err) => {
            eprintln!("windows sandbox failed: {err:#}");
            1
        }
    };
    std::process::exit(exit_code);
}

async fn run_windows_sandbox_wrapper_args(args: Vec<String>) -> Result<i32> {
    let request = parse_windows_sandbox_wrapper_args(args)?;
    run_windows_sandbox_wrapper_request(request).await
}

struct WindowsSandboxWrapperRequest {
    codex_home: PathBuf,
    command_cwd: AbsolutePathBuf,
    workspace_roots: Vec<AbsolutePathBuf>,
    env_map: HashMap<String, String>,
    permission_profile: PermissionProfile,
    windows_sandbox_level: WindowsSandboxLevel,
    windows_sandbox_private_desktop: bool,
    proxy_enforced: bool,
    read_roots_override: Option<Vec<PathBuf>>,
    read_roots_include_platform_defaults: bool,
    write_roots_override: Option<Vec<PathBuf>>,
    deny_read_paths_override: Vec<AbsolutePathBuf>,
    deny_write_paths_override: Vec<AbsolutePathBuf>,
    command: Vec<String>,
}

async fn run_windows_sandbox_wrapper_request(request: WindowsSandboxWrapperRequest) -> Result<i32> {
    if request.command.is_empty() {
        bail!("missing sandboxed command in windows sandbox wrapper request");
    }
    let spawned =
        crate::spawn_windows_sandbox_session_for_level(crate::WindowsSandboxSessionRequest {
            permission_profile: &request.permission_profile,
            workspace_roots: request.workspace_roots.as_slice(),
            codex_home: request.codex_home.as_path(),
            command: request.command,
            cwd: request.command_cwd.as_path(),
            env_map: request.env_map,
            windows_sandbox_level: request.windows_sandbox_level,
            proxy_enforced: request.proxy_enforced,
            timeout_ms: None,
            read_roots_override: request.read_roots_override.as_deref(),
            read_roots_include_platform_defaults: request.read_roots_include_platform_defaults,
            write_roots_override: request.write_roots_override.as_deref(),
            deny_read_paths_override: request.deny_read_paths_override.as_slice(),
            deny_write_paths_override: request.deny_write_paths_override.as_slice(),
            tty: false,
            stdin_open: true,
            use_private_desktop: request.windows_sandbox_private_desktop,
        })
        .await?;

    Ok(crate::forward_sandbox_session_stdio(spawned).await)
}

fn parse_windows_sandbox_wrapper_args(args: Vec<String>) -> Result<WindowsSandboxWrapperRequest> {
    let mut args = args.into_iter();
    let mut codex_home = None;
    let mut command_cwd = None;
    let mut workspace_roots = Vec::new();
    let mut env_map = None;
    let mut permission_profile = None;
    let mut windows_sandbox_level = None;
    let mut windows_sandbox_private_desktop = false;
    let mut proxy_enforced = false;
    let mut read_roots_override = None;
    let mut read_roots_include_platform_defaults = false;
    let mut write_roots_override = None;
    let mut deny_read_paths_override = Vec::new();
    let mut deny_write_paths_override = Vec::new();
    let mut command = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            CODEX_HOME_FLAG => codex_home = Some(PathBuf::from(next_flag_value(&mut args, &arg)?)),
            COMMAND_CWD_FLAG => {
                command_cwd = Some(absolute_path_arg(next_flag_value(&mut args, &arg)?, &arg)?);
            }
            WORKSPACE_ROOT_FLAG => {
                workspace_roots.push(absolute_path_arg(next_flag_value(&mut args, &arg)?, &arg)?);
            }
            ENV_JSON_FLAG => {
                let value = next_flag_value(&mut args, &arg)?;
                env_map = Some(serde_json::from_str(&value).context("failed to parse env json")?);
            }
            DENY_READ_PATHS_JSON_FLAG => {
                deny_read_paths_override =
                    json_flag_value(next_flag_value(&mut args, &arg)?, &arg)?;
            }
            DENY_WRITE_PATHS_JSON_FLAG => {
                deny_write_paths_override =
                    json_flag_value(next_flag_value(&mut args, &arg)?, &arg)?;
            }
            PERMISSION_PROFILE_FLAG => {
                let value = next_flag_value(&mut args, &arg)?;
                permission_profile = Some(
                    serde_json::from_str(&value).context("failed to parse permission profile")?,
                );
            }
            SANDBOX_LEVEL_FLAG => {
                let value = next_flag_value(&mut args, &arg)?;
                windows_sandbox_level = Some(parse_windows_sandbox_level(&value)?);
            }
            PRIVATE_DESKTOP_FLAG => windows_sandbox_private_desktop = true,
            PROXY_ENFORCED_FLAG => proxy_enforced = true,
            READ_ROOTS_INCLUDE_PLATFORM_DEFAULTS_FLAG => {
                read_roots_include_platform_defaults = true;
            }
            READ_ROOTS_JSON_FLAG => {
                read_roots_override =
                    Some(json_flag_value(next_flag_value(&mut args, &arg)?, &arg)?);
            }
            WRITE_ROOTS_JSON_FLAG => {
                write_roots_override =
                    Some(json_flag_value(next_flag_value(&mut args, &arg)?, &arg)?);
            }
            "--" => {
                command = Some(args.collect::<Vec<_>>());
                break;
            }
            _ => bail!("unexpected windows sandbox wrapper argument: {arg}"),
        }
    }

    let codex_home = codex_home.ok_or_else(|| anyhow!("missing required {CODEX_HOME_FLAG}"))?;
    if !codex_home.is_absolute() {
        bail!(
            "{CODEX_HOME_FLAG} must be absolute: {}",
            codex_home.display()
        );
    }
    let command_cwd = command_cwd.ok_or_else(|| anyhow!("missing required {COMMAND_CWD_FLAG}"))?;
    if workspace_roots.is_empty() {
        workspace_roots.push(command_cwd.clone());
    }
    Ok(WindowsSandboxWrapperRequest {
        codex_home,
        command_cwd,
        workspace_roots,
        env_map: env_map.ok_or_else(|| anyhow!("missing required {ENV_JSON_FLAG}"))?,
        permission_profile: permission_profile
            .ok_or_else(|| anyhow!("missing required {PERMISSION_PROFILE_FLAG}"))?,
        windows_sandbox_level: windows_sandbox_level
            .ok_or_else(|| anyhow!("missing required {SANDBOX_LEVEL_FLAG}"))?,
        windows_sandbox_private_desktop,
        proxy_enforced,
        read_roots_override,
        read_roots_include_platform_defaults,
        write_roots_override,
        deny_read_paths_override,
        deny_write_paths_override,
        command: command.ok_or_else(|| anyhow!("missing sandboxed command separator --"))?,
    })
}

fn next_flag_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String> {
    args.next()
        .ok_or_else(|| anyhow!("missing value for {flag}"))
}

fn absolute_path_arg(value: String, flag: &str) -> Result<AbsolutePathBuf> {
    let path = PathBuf::from(value);
    AbsolutePathBuf::from_absolute_path(path.as_path())
        .with_context(|| format!("{flag} must be absolute: {}", path.display()))
}

fn json_flag_value<T: serde::de::DeserializeOwned>(value: String, flag: &str) -> Result<T> {
    serde_json::from_str(&value).with_context(|| format!("failed to parse {flag}"))
}

fn parse_windows_sandbox_level(value: &str) -> Result<WindowsSandboxLevel> {
    match value {
        "disabled" => Ok(WindowsSandboxLevel::Disabled),
        "restricted-token" => Ok(WindowsSandboxLevel::RestrictedToken),
        "elevated" => Ok(WindowsSandboxLevel::Elevated),
        _ => bail!("invalid windows sandbox level: {value}"),
    }
}

#[cfg(test)]
#[path = "wrapper_tests.rs"]
mod tests;
