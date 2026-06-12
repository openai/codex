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
use serde::Deserialize;
use serde::Serialize;

pub const CODEX_WINDOWS_SANDBOX_ARG1: &str = "--run-as-windows-sandbox";

const REQUEST_FILE_FLAG: &str = "--request-file";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WindowsSandboxWrapperRequest {
    pub codex_home: PathBuf,
    pub command_cwd: AbsolutePathBuf,
    pub env_map: HashMap<String, String>,
    pub permission_profile: PermissionProfile,
    pub windows_sandbox_level: WindowsSandboxLevel,
    pub windows_sandbox_private_desktop: bool,
    pub command: Vec<String>,
}

pub fn create_windows_sandbox_wrapper_request_for_permission_profile(
    command: Vec<String>,
    command_cwd: AbsolutePathBuf,
    env_map: HashMap<String, String>,
    permission_profile: PermissionProfile,
    windows_sandbox_level: WindowsSandboxLevel,
    windows_sandbox_private_desktop: bool,
    codex_home: PathBuf,
) -> WindowsSandboxWrapperRequest {
    WindowsSandboxWrapperRequest {
        codex_home,
        command_cwd,
        env_map,
        permission_profile,
        windows_sandbox_level,
        windows_sandbox_private_desktop,
        command,
    }
}

pub fn create_windows_sandbox_command_args_for_request_file(request_file: &Path) -> Vec<String> {
    vec![
        CODEX_WINDOWS_SANDBOX_ARG1.to_string(),
        REQUEST_FILE_FLAG.to_string(),
        request_file.to_string_lossy().into_owned(),
    ]
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
    let request_file = parse_windows_sandbox_wrapper_args(args)?;
    let request_json = std::fs::read(&request_file).with_context(|| {
        format!(
            "failed to read windows sandbox wrapper request {}",
            request_file.display()
        )
    })?;
    let _ = std::fs::remove_file(&request_file);
    let request: WindowsSandboxWrapperRequest = serde_json::from_slice(&request_json)
        .context("failed to parse windows sandbox wrapper request")?;
    run_windows_sandbox_wrapper_request(request).await
}

async fn run_windows_sandbox_wrapper_request(
    mut request: WindowsSandboxWrapperRequest,
) -> Result<i32> {
    validate_windows_sandbox_wrapper_request(&request)?;
    let env_map = std::mem::take(&mut request.env_map);
    let workspace_roots = vec![request.command_cwd.clone()];
    let spawned = match request.windows_sandbox_level {
        WindowsSandboxLevel::Elevated => {
            let overrides = crate::resolve_windows_elevated_filesystem_overrides(
                /*windows_sandbox_active*/ true,
                &request.permission_profile,
                &request.command_cwd,
                /*use_windows_elevated_backend*/ true,
            )
            .map_err(anyhow::Error::msg)?
            .unwrap_or_default();
            crate::unified_exec::spawn_windows_sandbox_session_elevated_for_permission_profile(
                &request.permission_profile,
                workspace_roots.as_slice(),
                request.codex_home.as_path(),
                request.command,
                request.command_cwd.as_path(),
                env_map,
                /*timeout_ms*/ None,
                overrides.read_roots_override.as_deref(),
                overrides.read_roots_include_platform_defaults,
                overrides.write_roots_override.as_deref(),
                &overrides.additional_deny_read_paths,
                &overrides.additional_deny_write_paths,
                /*tty*/ false,
                /*stdin_open*/ true,
                request.windows_sandbox_private_desktop,
            )
            .await
        }
        WindowsSandboxLevel::RestrictedToken | WindowsSandboxLevel::Disabled => {
            let overrides = crate::resolve_windows_restricted_token_filesystem_overrides(
                /*windows_sandbox_active*/ true,
                &request.permission_profile,
                &request.command_cwd,
                request.windows_sandbox_level,
            )
            .map_err(anyhow::Error::msg)?
            .unwrap_or_default();
            crate::unified_exec::spawn_windows_sandbox_session_legacy(
                &request.permission_profile,
                workspace_roots.as_slice(),
                request.codex_home.as_path(),
                request.command,
                request.command_cwd.as_path(),
                env_map,
                /*timeout_ms*/ None,
                &overrides.additional_deny_read_paths,
                &overrides.additional_deny_write_paths,
                /*tty*/ false,
                /*stdin_open*/ true,
                request.windows_sandbox_private_desktop,
            )
            .await
        }
    }?;

    Ok(crate::stdio_bridge::forward_sandbox_session_stdio(spawned).await)
}

fn validate_windows_sandbox_wrapper_request(request: &WindowsSandboxWrapperRequest) -> Result<()> {
    if !request.codex_home.is_absolute() {
        bail!(
            "windows sandbox wrapper codex_home must be absolute: {}",
            request.codex_home.display()
        );
    }
    if request.command.is_empty() {
        bail!("missing sandboxed command in windows sandbox wrapper request");
    }
    Ok(())
}

fn parse_windows_sandbox_wrapper_args(args: Vec<String>) -> Result<PathBuf> {
    let mut args = args.into_iter();
    let Some(flag) = args.next() else {
        bail!("missing required argument {REQUEST_FILE_FLAG}");
    };
    if flag != REQUEST_FILE_FLAG {
        bail!("expected {REQUEST_FILE_FLAG}, got {flag}");
    }
    let request_file = PathBuf::from(next_flag_value(&mut args, REQUEST_FILE_FLAG)?);
    if !request_file.is_absolute() {
        bail!(
            "{REQUEST_FILE_FLAG} must be absolute: {}",
            request_file.display()
        );
    }
    if let Some(arg) = args.next() {
        bail!("unexpected windows sandbox wrapper argument: {arg}");
    }
    Ok(request_file)
}

fn next_flag_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String> {
    args.next()
        .ok_or_else(|| anyhow!("missing value for {flag}"))
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use codex_protocol::permissions::NetworkSandboxPolicy;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;
    use tokio::runtime::Builder;

    use super::*;

    static WRAPPER_PROCESS_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn current_thread_runtime() -> tokio::runtime::Runtime {
        Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime")
    }

    #[test]
    fn windows_wrapper_args_reference_request_file_only() -> Result<()> {
        let args = create_windows_sandbox_command_args_for_request_file(Path::new(
            r"C:\codex-home\.sandbox\request.json",
        ));

        assert_eq!(
            args,
            vec![
                CODEX_WINDOWS_SANDBOX_ARG1.to_string(),
                REQUEST_FILE_FLAG.to_string(),
                r"C:\codex-home\.sandbox\request.json".to_string(),
            ]
        );
        assert_eq!(
            parse_windows_sandbox_wrapper_args(args.into_iter().skip(1).collect())?,
            PathBuf::from(r"C:\codex-home\.sandbox\request.json")
        );
        Ok(())
    }

    #[test]
    fn windows_wrapper_request_round_trips() -> Result<()> {
        let permission_profile = PermissionProfile::External {
            network: NetworkSandboxPolicy::Restricted,
        };
        let request = create_windows_sandbox_wrapper_request_for_permission_profile(
            vec![
                "helper.exe".to_string(),
                "--codex-run-as-fs-helper".to_string(),
            ],
            AbsolutePathBuf::from_absolute_path(Path::new(r"C:\work"))?,
            HashMap::from([("PATH".to_string(), r"C:\Windows\System32".to_string())]),
            permission_profile.clone(),
            WindowsSandboxLevel::RestrictedToken,
            /*windows_sandbox_private_desktop*/ true,
            PathBuf::from(r"C:\codex-home"),
        );
        let request: WindowsSandboxWrapperRequest =
            serde_json::from_slice(&serde_json::to_vec(&request)?)?;

        assert_eq!(
            request.command,
            vec![
                "helper.exe".to_string(),
                "--codex-run-as-fs-helper".to_string()
            ]
        );
        assert_eq!(
            request.env_map,
            HashMap::from([("PATH".to_string(), r"C:\Windows\System32".to_string())])
        );
        assert_eq!(request.permission_profile, permission_profile);
        assert_eq!(
            request.windows_sandbox_level,
            WindowsSandboxLevel::RestrictedToken
        );
        assert_eq!(request.windows_sandbox_private_desktop, true);
        Ok(())
    }

    #[test]
    fn windows_wrapper_request_env_reaches_sandboxed_command() -> Result<()> {
        let _guard = WRAPPER_PROCESS_TEST_LOCK
            .lock()
            .expect("wrapper process test lock poisoned");
        let runtime = current_thread_runtime();
        runtime.block_on(async move {
            let workspace = TempDir::new()?;
            let workspace = AbsolutePathBuf::from_absolute_path(workspace.path())?;
            let codex_home = TempDir::new()?;
            let request = create_windows_sandbox_wrapper_request_for_permission_profile(
                vec![
                    r"C:\Windows\System32\cmd.exe".to_string(),
                    "/d".to_string(),
                    "/s".to_string(),
                    "/c".to_string(),
                    r#"if "%CODEX_WRAPPER_REQUEST_ENV%"=="from-request" (exit /b 0) else (exit /b 7)"#
                        .to_string(),
                ],
                workspace,
                HashMap::from([(
                    "CODEX_WRAPPER_REQUEST_ENV".to_string(),
                    "from-request".to_string(),
                )]),
                PermissionProfile::read_only(),
                WindowsSandboxLevel::RestrictedToken,
                /*windows_sandbox_private_desktop*/ true,
                codex_home.path().to_path_buf(),
            );

            let exit_code = run_windows_sandbox_wrapper_request(request).await?;

            assert_eq!(exit_code, 0);
            Ok(())
        })
    }
}
