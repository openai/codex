use super::SandboxCommand;
use super::SandboxManager;
use super::SandboxTransformRequest;
use super::SandboxType;
use super::SandboxablePreference;
use super::get_platform_sandbox;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::NetworkPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::NetworkAccess;
use codex_protocol::protocol::ReadOnlyAccess;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use dunce::canonicalize;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use tempfile::TempDir;

#[test]
fn danger_full_access_defaults_to_no_sandbox_without_network_requirements() {
    let manager = SandboxManager::new();
    let sandbox = manager.select_initial(
        &FileSystemSandboxPolicy::unrestricted(),
        NetworkSandboxPolicy::Enabled,
        SandboxablePreference::Auto,
        WindowsSandboxLevel::Disabled,
        /*has_managed_network_requirements*/ false,
    );
    assert_eq!(sandbox, SandboxType::None);
}

#[test]
fn danger_full_access_uses_platform_sandbox_with_network_requirements() {
    let manager = SandboxManager::new();
    let expected =
        get_platform_sandbox(/*windows_sandbox_enabled*/ false).unwrap_or(SandboxType::None);
    let sandbox = manager.select_initial(
        &FileSystemSandboxPolicy::unrestricted(),
        NetworkSandboxPolicy::Enabled,
        SandboxablePreference::Auto,
        WindowsSandboxLevel::Disabled,
        /*has_managed_network_requirements*/ true,
    );
    assert_eq!(sandbox, expected);
}

#[test]
fn restricted_file_system_uses_platform_sandbox_without_managed_network() {
    let manager = SandboxManager::new();
    let expected =
        get_platform_sandbox(/*windows_sandbox_enabled*/ false).unwrap_or(SandboxType::None);
    let sandbox = manager.select_initial(
        &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Read,
        }]),
        NetworkSandboxPolicy::Enabled,
        SandboxablePreference::Auto,
        WindowsSandboxLevel::Disabled,
        /*has_managed_network_requirements*/ false,
    );
    assert_eq!(sandbox, expected);
}

#[test]
fn transform_preserves_unrestricted_file_system_policy_for_restricted_network() {
    let manager = SandboxManager::new();
    let cwd = AbsolutePathBuf::current_dir().expect("current dir");
    let exec_request = manager
        .transform(SandboxTransformRequest {
            command: SandboxCommand {
                program: "true".into(),
                args: Vec::new(),
                cwd: cwd.clone(),
                env: HashMap::new(),
                additional_permissions: None,
            },
            policy: &SandboxPolicy::ExternalSandbox {
                network_access: NetworkAccess::Restricted,
            },
            file_system_policy: &FileSystemSandboxPolicy::unrestricted(),
            network_policy: NetworkSandboxPolicy::Restricted,
            sandbox: SandboxType::None,
            enforce_managed_network: false,
            network: None,
            sandbox_policy_cwd: cwd.as_path(),
            codex_linux_sandbox_exe: None,
            use_legacy_landlock: false,
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            windows_sandbox_private_desktop: false,
        })
        .expect("transform");

    assert_eq!(
        exec_request.file_system_sandbox_policy,
        FileSystemSandboxPolicy::unrestricted()
    );
    assert_eq!(
        exec_request.network_sandbox_policy,
        NetworkSandboxPolicy::Restricted
    );
}

#[test]
fn transform_additional_permissions_enable_network_for_external_sandbox() {
    let manager = SandboxManager::new();
    let cwd = AbsolutePathBuf::current_dir().expect("current dir");
    let temp_dir = TempDir::new().expect("create temp dir");
    let path = AbsolutePathBuf::from_absolute_path(
        canonicalize(temp_dir.path()).expect("canonicalize temp dir"),
    )
    .expect("absolute temp dir");
    let exec_request = manager
        .transform(SandboxTransformRequest {
            command: SandboxCommand {
                program: "true".into(),
                args: Vec::new(),
                cwd: cwd.clone(),
                env: HashMap::new(),
                additional_permissions: Some(PermissionProfile {
                    network: Some(NetworkPermissions {
                        enabled: Some(true),
                    }),
                    file_system: Some(FileSystemPermissions {
                        read: Some(vec![path]),
                        write: Some(Vec::new()),
                    }),
                }),
            },
            policy: &SandboxPolicy::ExternalSandbox {
                network_access: NetworkAccess::Restricted,
            },
            file_system_policy: &FileSystemSandboxPolicy::unrestricted(),
            network_policy: NetworkSandboxPolicy::Restricted,
            sandbox: SandboxType::None,
            enforce_managed_network: false,
            network: None,
            sandbox_policy_cwd: cwd.as_path(),
            codex_linux_sandbox_exe: None,
            use_legacy_landlock: false,
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            windows_sandbox_private_desktop: false,
        })
        .expect("transform");

    assert_eq!(
        exec_request.sandbox_policy,
        SandboxPolicy::ExternalSandbox {
            network_access: NetworkAccess::Enabled,
        }
    );
    assert_eq!(
        exec_request.network_sandbox_policy,
        NetworkSandboxPolicy::Enabled
    );
}

#[test]
fn transform_additional_permissions_preserves_denied_entries() {
    let manager = SandboxManager::new();
    let cwd = AbsolutePathBuf::current_dir().expect("current dir");
    let temp_dir = TempDir::new().expect("create temp dir");
    let workspace_root = AbsolutePathBuf::from_absolute_path(
        canonicalize(temp_dir.path()).expect("canonicalize temp dir"),
    )
    .expect("absolute temp dir");
    let allowed_path = workspace_root.join("allowed");
    let denied_path = workspace_root.join("denied");
    let exec_request = manager
        .transform(SandboxTransformRequest {
            command: SandboxCommand {
                program: "true".into(),
                args: Vec::new(),
                cwd: cwd.clone(),
                env: HashMap::new(),
                additional_permissions: Some(PermissionProfile {
                    file_system: Some(FileSystemPermissions {
                        read: None,
                        write: Some(vec![allowed_path.clone()]),
                    }),
                    ..Default::default()
                }),
            },
            policy: &SandboxPolicy::ReadOnly {
                access: ReadOnlyAccess::FullAccess,
                network_access: false,
            },
            file_system_policy: &FileSystemSandboxPolicy::restricted(vec![
                FileSystemSandboxEntry {
                    path: FileSystemPath::Special {
                        value: FileSystemSpecialPath::Root,
                    },
                    access: FileSystemAccessMode::Read,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::Path {
                        path: denied_path.clone(),
                    },
                    access: FileSystemAccessMode::None,
                },
            ]),
            network_policy: NetworkSandboxPolicy::Restricted,
            sandbox: SandboxType::None,
            enforce_managed_network: false,
            network: None,
            sandbox_policy_cwd: cwd.as_path(),
            codex_linux_sandbox_exe: None,
            use_legacy_landlock: false,
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            windows_sandbox_private_desktop: false,
        })
        .expect("transform");

    assert_eq!(
        exec_request.file_system_sandbox_policy,
        FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Read,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path { path: denied_path },
                access: FileSystemAccessMode::None,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path { path: allowed_path },
                access: FileSystemAccessMode::Write,
            },
        ])
    );
    assert_eq!(
        exec_request.network_sandbox_policy,
        NetworkSandboxPolicy::Restricted
    );
}

#[cfg(target_os = "linux")]
fn transform_linux_seccomp_request(
    codex_linux_sandbox_exe: &std::path::Path,
) -> super::SandboxExecRequest {
    let manager = SandboxManager::new();
    let cwd = AbsolutePathBuf::current_dir().expect("current dir");
    manager
        .transform(SandboxTransformRequest {
            command: SandboxCommand {
                program: "true".into(),
                args: Vec::new(),
                cwd: cwd.clone(),
                env: HashMap::new(),
                additional_permissions: None,
            },
            policy: &SandboxPolicy::DangerFullAccess,
            file_system_policy: &FileSystemSandboxPolicy::unrestricted(),
            network_policy: NetworkSandboxPolicy::Enabled,
            sandbox: SandboxType::LinuxSeccomp,
            enforce_managed_network: false,
            network: None,
            sandbox_policy_cwd: cwd.as_path(),
            codex_linux_sandbox_exe: Some(codex_linux_sandbox_exe),
            use_legacy_landlock: false,
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            windows_sandbox_private_desktop: false,
        })
        .expect("transform")
}

#[cfg(target_os = "linux")]
#[test]
fn transform_linux_seccomp_preserves_helper_path_in_arg0_when_available() {
    let codex_linux_sandbox_exe = std::path::PathBuf::from("/tmp/codex-linux-sandbox");
    let exec_request = transform_linux_seccomp_request(&codex_linux_sandbox_exe);

    assert_eq!(
        exec_request.arg0,
        Some(codex_linux_sandbox_exe.to_string_lossy().into_owned())
    );
}

#[cfg(target_os = "linux")]
#[test]
fn transform_linux_seccomp_uses_helper_alias_when_launcher_is_not_helper_path() {
    let codex_linux_sandbox_exe = std::path::PathBuf::from("/tmp/codex");
    let exec_request = transform_linux_seccomp_request(&codex_linux_sandbox_exe);

    assert_eq!(exec_request.arg0, Some("codex-linux-sandbox".to_string()));
}
