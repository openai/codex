use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;

use super::CODEX_HOME_FLAG;
use super::CODEX_WINDOWS_SANDBOX_ARG1;
use super::COMMAND_CWD_FLAG;
use super::DENY_READ_PATHS_JSON_FLAG;
use super::DENY_WRITE_PATHS_JSON_FLAG;
use super::ENV_JSON_FLAG;
use super::NETWORK_PROXY_RESTRICTING_SID_FLAG;
use super::PERMISSION_PROFILE_FLAG;
use super::PRESERVE_PROXY_SETTINGS_FLAG;
use super::PRIVATE_DESKTOP_FLAG;
use super::PROXY_ENFORCED_FLAG;
use super::READ_ROOTS_INCLUDE_PLATFORM_DEFAULTS_FLAG;
use super::READ_ROOTS_JSON_FLAG;
use super::SANDBOX_LEVEL_FLAG;
use super::WORKSPACE_ROOT_FLAG;
use super::WRITE_ROOTS_JSON_FLAG;
use super::create_windows_sandbox_command_args_for_permission_profile;
use super::parse_windows_sandbox_wrapper_args;

#[test]
fn windows_wrapper_args_round_trip() {
    let command_cwd = AbsolutePathBuf::from_absolute_path(Path::new(r"C:\workspace"))
        .expect("absolute command cwd");
    let workspace_roots = vec![
        command_cwd.clone(),
        AbsolutePathBuf::from_absolute_path(Path::new(r"D:\other-workspace"))
            .expect("absolute workspace root"),
    ];
    let env = HashMap::from([("Path".to_string(), r"C:\Windows\System32".to_string())]);
    let permission_profile = PermissionProfile::External {
        network: NetworkSandboxPolicy::Restricted,
    };
    let read_roots_override = vec![PathBuf::from(r"C:\read")];
    let write_roots_override = vec![PathBuf::from(r"C:\write")];
    let deny_read_paths_override = vec![
        AbsolutePathBuf::from_absolute_path(Path::new(r"C:\blocked-read"))
            .expect("absolute deny-read"),
    ];
    let deny_write_paths_override = vec![
        AbsolutePathBuf::from_absolute_path(Path::new(r"C:\blocked-write"))
            .expect("absolute deny-write"),
    ];

    let args = create_windows_sandbox_command_args_for_permission_profile(
        vec![
            "codex.exe".to_string(),
            "--codex-run-as-fs-helper".to_string(),
        ],
        &command_cwd,
        workspace_roots.as_slice(),
        &env,
        &permission_profile,
        WindowsSandboxLevel::Elevated,
        /*windows_sandbox_private_desktop*/ true,
        /*proxy_enforced*/ true,
        /*network_proxy_restricting_sid*/ Some("S-1-5-21-100-200-300-400"),
        crate::WindowsSandboxProxySettingsMode::Preserve,
        Some(read_roots_override.as_slice()),
        /*read_roots_include_platform_defaults*/ true,
        Some(write_roots_override.as_slice()),
        deny_read_paths_override.as_slice(),
        deny_write_paths_override.as_slice(),
        Path::new(r"C:\Users\me\.codex"),
    );

    assert_eq!(args[0], CODEX_WINDOWS_SANDBOX_ARG1);
    assert!(args.contains(&CODEX_HOME_FLAG.to_string()));
    assert!(args.contains(&COMMAND_CWD_FLAG.to_string()));
    assert!(args.contains(&WORKSPACE_ROOT_FLAG.to_string()));
    assert!(args.contains(&PERMISSION_PROFILE_FLAG.to_string()));
    assert!(args.contains(&ENV_JSON_FLAG.to_string()));
    assert!(args.contains(&SANDBOX_LEVEL_FLAG.to_string()));
    assert!(args.contains(&PRIVATE_DESKTOP_FLAG.to_string()));
    assert!(args.contains(&PROXY_ENFORCED_FLAG.to_string()));
    assert!(args.contains(&NETWORK_PROXY_RESTRICTING_SID_FLAG.to_string()));
    assert!(args.contains(&PRESERVE_PROXY_SETTINGS_FLAG.to_string()));
    assert!(args.contains(&READ_ROOTS_JSON_FLAG.to_string()));
    assert!(args.contains(&READ_ROOTS_INCLUDE_PLATFORM_DEFAULTS_FLAG.to_string()));
    assert!(args.contains(&WRITE_ROOTS_JSON_FLAG.to_string()));
    assert!(args.contains(&DENY_READ_PATHS_JSON_FLAG.to_string()));
    assert!(args.contains(&DENY_WRITE_PATHS_JSON_FLAG.to_string()));

    let parsed =
        parse_windows_sandbox_wrapper_args(args[1..].to_vec()).expect("parse wrapper args");

    assert_eq!(
        parsed.command,
        vec!["codex.exe", "--codex-run-as-fs-helper"]
    );
    assert_eq!(parsed.command_cwd, command_cwd);
    assert_eq!(parsed.workspace_roots, workspace_roots);
    assert_eq!(parsed.env_map, env);
    assert_eq!(parsed.permission_profile, permission_profile);
    assert_eq!(parsed.windows_sandbox_level, WindowsSandboxLevel::Elevated);
    assert_eq!(parsed.windows_sandbox_private_desktop, true);
    assert_eq!(parsed.proxy_enforced, true);
    assert_eq!(
        parsed.network_proxy_restricting_sid.as_deref(),
        Some("S-1-5-21-100-200-300-400")
    );
    assert_eq!(
        parsed.proxy_settings_mode,
        crate::WindowsSandboxProxySettingsMode::Preserve
    );
    assert_eq!(parsed.read_roots_override, Some(read_roots_override));
    assert_eq!(parsed.read_roots_include_platform_defaults, true);
    assert_eq!(parsed.write_roots_override, Some(write_roots_override));
    assert_eq!(parsed.deny_read_paths_override, deny_read_paths_override);
    assert_eq!(parsed.deny_write_paths_override, deny_write_paths_override);
}
