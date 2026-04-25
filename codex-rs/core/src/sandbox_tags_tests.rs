use super::permission_profile_policy_tag;
use super::permission_profile_sandbox_tag;
use super::sandbox_tag;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::ManagedFileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxKind;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::NetworkAccess;
use codex_protocol::protocol::SandboxPolicy;
use codex_sandboxing::SandboxType;
use codex_sandboxing::get_platform_sandbox;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::path::Path;

#[test]
fn danger_full_access_is_untagged_even_when_linux_sandbox_defaults_apply() {
    let actual = sandbox_tag(
        &SandboxPolicy::DangerFullAccess,
        WindowsSandboxLevel::Disabled,
    );
    assert_eq!(actual, "none");
}

#[test]
fn external_sandbox_keeps_external_tag_when_linux_sandbox_defaults_apply() {
    let actual = sandbox_tag(
        &SandboxPolicy::ExternalSandbox {
            network_access: NetworkAccess::Enabled,
        },
        WindowsSandboxLevel::Disabled,
    );
    assert_eq!(actual, "external");
}

#[test]
fn default_linux_sandbox_uses_platform_sandbox_tag() {
    let actual = sandbox_tag(
        &SandboxPolicy::new_read_only_policy(),
        WindowsSandboxLevel::Disabled,
    );
    let expected = get_platform_sandbox(/*windows_sandbox_enabled*/ false)
        .map(SandboxType::as_metric_tag)
        .unwrap_or("none");
    assert_eq!(actual, expected);
}

#[test]
fn profile_sandbox_tag_distinguishes_disabled_from_external() {
    assert_eq!(
        permission_profile_sandbox_tag(&PermissionProfile::Disabled, WindowsSandboxLevel::Disabled),
        "none"
    );
    assert_eq!(
        permission_profile_sandbox_tag(
            &PermissionProfile::External {
                network: NetworkSandboxPolicy::Restricted,
            },
            WindowsSandboxLevel::Disabled,
        ),
        "external"
    );
}

#[test]
fn unrestricted_managed_profile_with_enabled_network_is_untagged() {
    let profile = PermissionProfile::Managed {
        file_system: ManagedFileSystemPermissions::Unrestricted,
        network: NetworkSandboxPolicy::Enabled,
    };

    assert_eq!(
        permission_profile_sandbox_tag(&profile, WindowsSandboxLevel::Disabled),
        "none"
    );
}

#[test]
fn profile_policy_tag_reports_closest_legacy_mode() {
    let cwd = AbsolutePathBuf::from_absolute_path(Path::new("/tmp/codex")).expect("absolute cwd");
    let writable_root = AbsolutePathBuf::from_absolute_path(Path::new("/tmp/codex/work"))
        .expect("absolute writable root");
    let profile = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy {
            kind: FileSystemSandboxKind::Restricted,
            glob_scan_max_depth: None,
            entries: vec![FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: writable_root,
                },
                access: FileSystemAccessMode::Write,
            }],
        },
        NetworkSandboxPolicy::Restricted,
    );

    assert_eq!(
        permission_profile_policy_tag(&profile, cwd.as_path()),
        "workspace-write"
    );
}
