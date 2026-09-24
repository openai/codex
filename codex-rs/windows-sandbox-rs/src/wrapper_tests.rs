use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
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
use super::PRIVATE_DESKTOP_NAME_FLAG;
use super::PROXY_ENFORCED_FLAG;
use super::READ_ROOTS_INCLUDE_PLATFORM_DEFAULTS_FLAG;
use super::READ_ROOTS_JSON_FLAG;
use super::SANDBOX_LEVEL_FLAG;
use super::WORKSPACE_ROOT_FLAG;
use super::WRITE_ROOTS_JSON_FLAG;
use super::create_windows_sandbox_command_args_for_permission_profile;
use super::parse_windows_sandbox_wrapper_args;

#[test]
fn large_deny_list_uses_environment_without_changing_the_request() {
    let temp = tempfile::tempdir().expect("tempdir");
    let cwd = AbsolutePathBuf::from_absolute_path(temp.path()).expect("absolute command cwd");
    let paths = (0..1000)
        .map(|index| {
            AbsolutePathBuf::from_absolute_path(format!(r"C:\private\nested\file-{index}.txt"))
                .unwrap()
        })
        .collect::<Vec<_>>();
    let expected_env = HashMap::from([("Path".to_string(), r"C:\Windows\System32".to_string())]);
    let mut env = expected_env.clone();
    crate::environment_transport::encode("must not reach workload", &mut env).unwrap();
    let profile = PermissionProfile::read_only();
    let command = vec![
        "codex.exe".to_string(),
        "--codex-run-as-fs-helper".to_string(),
    ];
    let args = create_windows_sandbox_command_args_for_permission_profile(
        command.clone(),
        &cwd,
        std::slice::from_ref(&cwd),
        &mut env,
        &profile,
        WindowsSandboxLevel::RestrictedToken,
        /*proxy_enforced*/ true,
        /*network_proxy_restricting_sid*/ Some("S-1-5-21-100-200-300-400"),
        crate::WindowsSandboxProxySettingsMode::Preserve,
        /*read_roots_override*/ None,
        /*read_roots_include_platform_defaults*/ true,
        /*write_roots_override*/ None,
        &paths,
        &paths,
        temp.path(),
    )
    .unwrap();
    assert_eq!(
        args,
        vec![CODEX_WINDOWS_SANDBOX_ARG1, crate::launch_environment::ARG]
    );
    let decoded = crate::environment_transport::decode(env).unwrap();
    let parsed =
        parse_windows_sandbox_wrapper_args(serde_json::from_str(&decoded).unwrap()).unwrap();
    let desktop = crate::desktop::LaunchDesktop::open_private(&parsed.private_desktop_name)
        .expect("wrapper must receive a live private desktop");
    drop(desktop);
    assert_eq!(parsed.deny_read_paths_override, paths);
    assert_eq!(parsed.deny_write_paths_override, paths);
    assert_eq!(parsed.env_map, expected_env);
    assert_eq!(parsed.permission_profile, profile);
    assert_eq!(parsed.command, command);
    assert_eq!(parsed.command_cwd, cwd);
    assert_eq!(parsed.workspace_roots, vec![cwd]);
    assert_eq!(parsed.proxy_enforced, true);
    assert_eq!(parsed.read_roots_include_platform_defaults, true);
    assert_eq!(
        parsed.network_proxy_restricting_sid.as_deref(),
        Some("S-1-5-21-100-200-300-400")
    );
    assert_eq!(
        parsed.proxy_settings_mode,
        crate::WindowsSandboxProxySettingsMode::Preserve
    );
}

#[test]
fn windows_wrapper_args_round_trip() {
    let temp = tempfile::tempdir().expect("tempdir");
    let command_cwd =
        AbsolutePathBuf::from_absolute_path(temp.path()).expect("absolute command cwd");
    let workspace_roots = vec![
        command_cwd.clone(),
        AbsolutePathBuf::from_absolute_path(Path::new(r"D:\other-workspace"))
            .expect("absolute workspace root"),
    ];
    let mut env = HashMap::from([("Path".to_string(), r"C:\Windows\System32".to_string())]);
    let permission_profile = PermissionProfile::read_only();
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
        &mut env,
        &permission_profile,
        WindowsSandboxLevel::RestrictedToken,
        /*proxy_enforced*/ true,
        /*network_proxy_restricting_sid*/ Some("S-1-5-21-100-200-300-400"),
        crate::WindowsSandboxProxySettingsMode::Preserve,
        Some(read_roots_override.as_slice()),
        /*read_roots_include_platform_defaults*/ true,
        Some(write_roots_override.as_slice()),
        deny_read_paths_override.as_slice(),
        deny_write_paths_override.as_slice(),
        temp.path(),
    )
    .expect("build wrapper args");

    assert_eq!(args[0], CODEX_WINDOWS_SANDBOX_ARG1);
    assert!(args.contains(&CODEX_HOME_FLAG.to_string()));
    assert!(args.contains(&COMMAND_CWD_FLAG.to_string()));
    assert!(args.contains(&WORKSPACE_ROOT_FLAG.to_string()));
    assert!(args.contains(&PERMISSION_PROFILE_FLAG.to_string()));
    assert!(args.contains(&ENV_JSON_FLAG.to_string()));
    assert!(args.contains(&SANDBOX_LEVEL_FLAG.to_string()));
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
    assert_eq!(
        parsed.windows_sandbox_level,
        WindowsSandboxLevel::RestrictedToken
    );
    let desktop = crate::desktop::LaunchDesktop::open_private(&parsed.private_desktop_name)
        .expect("wrapper must receive a live private desktop");
    drop(desktop);
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

    let mut missing_desktop_args = args[1..].to_vec();
    let name_index = missing_desktop_args
        .iter()
        .position(|arg| arg == PRIVATE_DESKTOP_NAME_FLAG)
        .expect("private desktop name argument");
    missing_desktop_args.drain(name_index..name_index + 2);
    assert!(parse_windows_sandbox_wrapper_args(missing_desktop_args).is_err());
}
