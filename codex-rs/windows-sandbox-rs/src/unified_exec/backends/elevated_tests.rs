use super::RunnerTransportRequest;
use super::spawn_runner_transport_with_retry;
use crate::WindowsSandboxProxySettingsMode;
use crate::identity::SandboxCreds;
use crate::ipc_framed::ErrorPayload;
use crate::ipc_framed::ErrorStage;
use crate::ipc_framed::SpawnRequest;
use crate::resolved_permissions::ResolvedWindowsSandboxPermissions;
use crate::runner_client::RunnerStartupError;
use codex_protocol::models::PermissionProfile;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use windows_sys::Win32::Foundation::ERROR_NO_SUCH_LOGON_SESSION;

#[derive(Debug, Clone, PartialEq, Eq)]
struct SpawnObservation {
    codex_home: PathBuf,
    cwd: PathBuf,
    username: String,
    password: String,
    logs_base_dir: Option<PathBuf>,
    spawn_request: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RefreshObservation {
    permissions: ResolvedWindowsSandboxPermissions,
    cwd: PathBuf,
    env_map: HashMap<String, String>,
    codex_home: PathBuf,
    read_roots_override: Option<Vec<PathBuf>>,
    read_roots_include_platform_defaults: bool,
    write_roots_override: Option<Vec<PathBuf>>,
    deny_read_paths_override: Vec<PathBuf>,
    deny_write_paths_override: Vec<PathBuf>,
    proxy_enforced: bool,
    proxy_settings_mode: WindowsSandboxProxySettingsMode,
}

#[test]
fn retry_uses_original_unified_exec_request_and_stops_after_second_failure() {
    let workspace_root = AbsolutePathBuf::from_absolute_path(PathBuf::from(r"C:\workspace"))
        .expect("absolute workspace root");
    let permission_profile = PermissionProfile::workspace_write();
    let permissions =
        ResolvedWindowsSandboxPermissions::try_from_permission_profile_for_workspace_roots(
            &permission_profile,
            std::slice::from_ref(&workspace_root),
        )
        .expect("resolved permissions");
    let env_map = HashMap::from([
        ("Path".to_string(), r"C:\tools".to_string()),
        (
            "HTTPS_PROXY".to_string(),
            "http://localhost:1234".to_string(),
        ),
    ]);
    let request = RunnerTransportRequest {
        permissions: permissions.clone(),
        codex_home: PathBuf::from(r"C:\Users\codex"),
        cwd: PathBuf::from(r"C:\workspace"),
        env_map: env_map.clone(),
        logs_base_dir: Some(PathBuf::from(r"C:\Users\codex\.sandbox")),
        spawn_request: SpawnRequest {
            command: vec!["pwsh.exe".to_string(), "-NoProfile".to_string()],
            cwd: PathBuf::from(r"C:\workspace"),
            env: env_map.clone(),
            permission_profile,
            workspace_roots: vec![workspace_root],
            codex_home: PathBuf::from(r"C:\Users\codex\.sandbox"),
            real_codex_home: PathBuf::from(r"C:\Users\codex"),
            cap_sids: vec!["S-1-15-3-1024-1".to_string()],
            network_proxy_restricting_sid: Some("S-1-5-21-100-200-300-400".to_string()),
            timeout_ms: Some(5_000),
            tty: true,
            stdin_open: true,
            use_private_desktop: true,
        },
        read_roots_override: Some(vec![PathBuf::from(r"C:\workspace\read")]),
        read_roots_include_platform_defaults: true,
        write_roots_override: Some(vec![PathBuf::from(r"C:\workspace\write")]),
        deny_read_paths_override: vec![PathBuf::from(r"C:\secrets")],
        deny_write_paths_override: vec![PathBuf::from(r"C:\workspace\.codex")],
        proxy_enforced: true,
        proxy_settings_mode: WindowsSandboxProxySettingsMode::Preserve,
    };
    let expected_spawn_request =
        serde_json::to_value(&request.spawn_request).expect("serialize spawn request");
    let expected_refresh = RefreshObservation {
        permissions,
        cwd: request.cwd.clone(),
        env_map,
        codex_home: request.codex_home.clone(),
        read_roots_override: request.read_roots_override.clone(),
        read_roots_include_platform_defaults: true,
        write_roots_override: request.write_roots_override.clone(),
        deny_read_paths_override: request.deny_read_paths_override.clone(),
        deny_write_paths_override: request.deny_write_paths_override.clone(),
        proxy_enforced: true,
        proxy_settings_mode: WindowsSandboxProxySettingsMode::Preserve,
    };
    let spawn_observations = RefCell::new(Vec::new());
    let refresh_observations = RefCell::new(Vec::new());
    let spawn_attempts = Cell::new(0);

    let result = spawn_runner_transport_with_retry(
        SandboxCreds {
            username: "stale".to_string(),
            password: "old".to_string(),
        },
        &request,
        |codex_home, cwd, sandbox_creds, logs_base_dir, spawn_request| {
            spawn_observations.borrow_mut().push(SpawnObservation {
                codex_home: codex_home.to_path_buf(),
                cwd: cwd.to_path_buf(),
                username: sandbox_creds.username.clone(),
                password: sandbox_creds.password.clone(),
                logs_base_dir: logs_base_dir.map(Path::to_path_buf),
                spawn_request: serde_json::to_value(spawn_request)
                    .expect("serialize spawn request"),
            });
            spawn_attempts.set(spawn_attempts.get() + 1);
            Err::<(), _>(anyhow::Error::new(RunnerStartupError::new(ErrorPayload {
                message: format!("spawn attempt {} failed", spawn_attempts.get()),
                stage: ErrorStage::SpawnChild,
                windows_error_code: Some(ERROR_NO_SUCH_LOGON_SESSION),
            })))
        },
        |permissions,
         cwd,
         env_map,
         codex_home,
         read_roots_override,
         read_roots_include_platform_defaults,
         write_roots_override,
         deny_read_paths_override,
         deny_write_paths_override,
         proxy_enforced,
         proxy_settings_mode| {
            refresh_observations.borrow_mut().push(RefreshObservation {
                permissions: permissions.clone(),
                cwd: cwd.to_path_buf(),
                env_map: env_map.clone(),
                codex_home: codex_home.to_path_buf(),
                read_roots_override: read_roots_override.map(<[PathBuf]>::to_vec),
                read_roots_include_platform_defaults,
                write_roots_override: write_roots_override.map(<[PathBuf]>::to_vec),
                deny_read_paths_override: deny_read_paths_override.to_vec(),
                deny_write_paths_override: deny_write_paths_override.to_vec(),
                proxy_enforced,
                proxy_settings_mode,
            });
            Ok(SandboxCreds {
                username: "refreshed".to_string(),
                password: "new".to_string(),
            })
        },
    );

    let err = result.expect_err("second spawn should fail");
    assert_eq!(
        "runner failed during SpawnChild: spawn attempt 2 failed (Windows error 1312)",
        err.to_string()
    );
    assert_eq!(
        vec![
            SpawnObservation {
                codex_home: request.codex_home.clone(),
                cwd: request.cwd.clone(),
                username: "stale".to_string(),
                password: "old".to_string(),
                logs_base_dir: request.logs_base_dir.clone(),
                spawn_request: expected_spawn_request.clone(),
            },
            SpawnObservation {
                codex_home: request.codex_home.clone(),
                cwd: request.cwd.clone(),
                username: "refreshed".to_string(),
                password: "new".to_string(),
                logs_base_dir: request.logs_base_dir,
                spawn_request: expected_spawn_request,
            },
        ],
        *spawn_observations.borrow()
    );
    assert_eq!(vec![expected_refresh], *refresh_observations.borrow());
}
