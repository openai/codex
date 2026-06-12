use std::collections::HashMap;
use std::path::Path;

use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;

use super::CODEX_HOME_FLAG;
use super::CODEX_WINDOWS_SANDBOX_ARG1;
use super::COMMAND_CWD_FLAG;
use super::ENV_JSON_FLAG;
use super::PERMISSION_PROFILE_FLAG;
use super::PRIVATE_DESKTOP_FLAG;
use super::SANDBOX_LEVEL_FLAG;
use super::WORKSPACE_ROOT_FLAG;
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
}
