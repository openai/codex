#[cfg(target_os = "macos")]
use super::EffectiveSandboxPermissions;
use super::effective_file_system_sandbox_policy;
#[cfg(target_os = "macos")]
use super::intersect_permission_profiles;
use super::merge_file_system_policy_with_additional_permissions;
use super::normalize_additional_permissions;
use super::sandbox_policy_with_additional_permissions;
use super::should_require_platform_sandbox;
use codex_protocol::models::FileSystemPermissions;
#[cfg(target_os = "macos")]
use codex_protocol::models::MacOsAutomationPermission;
#[cfg(target_os = "macos")]
use codex_protocol::models::MacOsContactsPermission;
#[cfg(target_os = "macos")]
use codex_protocol::models::MacOsPreferencesPermission;
#[cfg(target_os = "macos")]
use codex_protocol::models::MacOsSeatbeltProfileExtensions;
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
#[cfg(unix)]
use std::path::Path;
use tempfile::TempDir;

#[cfg(unix)]
fn symlink_dir(original: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(original, link)
}

#[test]
fn full_access_restricted_policy_skips_platform_sandbox_when_network_is_enabled() {
    let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: FileSystemPath::Special {
            value: FileSystemSpecialPath::Root,
        },
        access: FileSystemAccessMode::Write,
    }]);

    assert_eq!(
        should_require_platform_sandbox(&policy, NetworkSandboxPolicy::Enabled, false),
        false
    );
}

#[test]
fn root_write_policy_with_carveouts_still_uses_platform_sandbox() {
    let blocked = AbsolutePathBuf::resolve_path_against_base(
        "blocked",
        std::env::current_dir().expect("current dir"),
    )
    .expect("blocked path");
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Write,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path { path: blocked },
            access: FileSystemAccessMode::None,
        },
    ]);

    assert_eq!(
        should_require_platform_sandbox(&policy, NetworkSandboxPolicy::Enabled, false),
        true
    );
}

#[test]
fn full_access_restricted_policy_still_uses_platform_sandbox_for_restricted_network() {
    let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: FileSystemPath::Special {
            value: FileSystemSpecialPath::Root,
        },
        access: FileSystemAccessMode::Write,
    }]);

    assert_eq!(
        should_require_platform_sandbox(&policy, NetworkSandboxPolicy::Restricted, false),
        true
    );
}

#[test]
fn normalize_additional_permissions_preserves_network() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let path = AbsolutePathBuf::from_absolute_path(
        canonicalize(temp_dir.path()).expect("canonicalize temp dir"),
    )
    .expect("absolute temp dir");
    let permissions = normalize_additional_permissions(PermissionProfile {
        network: Some(NetworkPermissions {
            enabled: Some(true),
        }),
        file_system: Some(FileSystemPermissions {
            read: Some(vec![path.clone()]),
            write: Some(vec![path.clone()]),
        }),
        ..Default::default()
    })
    .expect("permissions");

    assert_eq!(
        permissions.network,
        Some(NetworkPermissions {
            enabled: Some(true),
        })
    );
    assert_eq!(
        permissions.file_system,
        Some(FileSystemPermissions {
            read: Some(vec![path.clone()]),
            write: Some(vec![path]),
        })
    );
}

#[cfg(unix)]
#[test]
fn normalize_additional_permissions_canonicalizes_symlinked_write_paths() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let real_root = temp_dir.path().join("real");
    let link_root = temp_dir.path().join("link");
    let write_dir = real_root.join("write");
    std::fs::create_dir_all(&write_dir).expect("create write dir");
    symlink_dir(&real_root, &link_root).expect("create symlinked root");

    let link_write_dir =
        AbsolutePathBuf::from_absolute_path(link_root.join("write")).expect("link write dir");
    let expected_write_dir = AbsolutePathBuf::from_absolute_path(
        write_dir.canonicalize().expect("canonicalize write dir"),
    )
    .expect("absolute canonical write dir");

    let permissions = normalize_additional_permissions(PermissionProfile {
        file_system: Some(FileSystemPermissions {
            read: Some(vec![]),
            write: Some(vec![link_write_dir]),
        }),
        ..Default::default()
    })
    .expect("permissions");

    assert_eq!(
        permissions.file_system,
        Some(FileSystemPermissions {
            read: Some(vec![]),
            write: Some(vec![expected_write_dir]),
        })
    );
}

#[test]
fn normalize_additional_permissions_drops_empty_nested_profiles() {
    let permissions = normalize_additional_permissions(PermissionProfile {
        network: Some(NetworkPermissions { enabled: None }),
        file_system: Some(FileSystemPermissions {
            read: None,
            write: None,
        }),
        macos: None,
    })
    .expect("permissions");

    assert_eq!(permissions, PermissionProfile::default());
}

#[cfg(target_os = "macos")]
#[test]
fn normalize_additional_permissions_preserves_default_macos_preferences_permission() {
    let permissions = normalize_additional_permissions(PermissionProfile {
        macos: Some(MacOsSeatbeltProfileExtensions::default()),
        ..Default::default()
    })
    .expect("permissions");

    assert_eq!(
        permissions,
        PermissionProfile {
            macos: Some(MacOsSeatbeltProfileExtensions::default()),
            ..Default::default()
        }
    );
}

#[cfg(target_os = "macos")]
#[test]
fn intersect_permission_profiles_preserves_default_macos_grants() {
    let requested = PermissionProfile {
        file_system: Some(FileSystemPermissions {
            read: Some(Vec::from(["/tmp/requested"
                .try_into()
                .expect("absolute path")])),
            write: None,
        }),
        macos: Some(MacOsSeatbeltProfileExtensions {
            macos_preferences: MacOsPreferencesPermission::ReadWrite,
            macos_automation: MacOsAutomationPermission::BundleIds(vec![
                "com.apple.Notes".to_string(),
            ]),
            macos_launch_services: false,
            macos_accessibility: true,
            macos_calendar: true,
            macos_reminders: false,
            macos_contacts: MacOsContactsPermission::None,
        }),
        ..Default::default()
    };
    let granted = PermissionProfile {
        file_system: Some(FileSystemPermissions {
            read: Some(Vec::new()),
            write: None,
        }),
        macos: Some(MacOsSeatbeltProfileExtensions::default()),
        ..Default::default()
    };

    assert_eq!(
        intersect_permission_profiles(requested, granted),
        PermissionProfile {
            macos: Some(MacOsSeatbeltProfileExtensions::default()),
            ..Default::default()
        }
    );
}

#[cfg(target_os = "macos")]
#[test]
fn normalize_additional_permissions_preserves_macos_permissions() {
    let permissions = normalize_additional_permissions(PermissionProfile {
        macos: Some(MacOsSeatbeltProfileExtensions {
            macos_preferences: MacOsPreferencesPermission::ReadWrite,
            macos_automation: MacOsAutomationPermission::BundleIds(vec![
                "com.apple.Notes".to_string(),
            ]),
            macos_launch_services: true,
            macos_accessibility: true,
            macos_calendar: true,
            macos_reminders: false,
            macos_contacts: MacOsContactsPermission::None,
        }),
        ..Default::default()
    })
    .expect("permissions");

    assert_eq!(
        permissions.macos,
        Some(MacOsSeatbeltProfileExtensions {
            macos_preferences: MacOsPreferencesPermission::ReadWrite,
            macos_automation: MacOsAutomationPermission::BundleIds(vec![
                "com.apple.Notes".to_string(),
            ]),
            macos_launch_services: true,
            macos_accessibility: true,
            macos_calendar: true,
            macos_reminders: false,
            macos_contacts: MacOsContactsPermission::None,
        })
    );
}

#[test]
fn read_only_additional_permissions_can_enable_network_without_writes() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let path = AbsolutePathBuf::from_absolute_path(
        canonicalize(temp_dir.path()).expect("canonicalize temp dir"),
    )
    .expect("absolute temp dir");
    let policy = sandbox_policy_with_additional_permissions(
        &SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::Restricted {
                include_platform_defaults: true,
                readable_roots: vec![path.clone()],
            },
            network_access: false,
        },
        &PermissionProfile {
            network: Some(NetworkPermissions {
                enabled: Some(true),
            }),
            file_system: Some(FileSystemPermissions {
                read: Some(vec![path.clone()]),
                write: Some(Vec::new()),
            }),
            ..Default::default()
        },
    );

    assert_eq!(
        policy,
        SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::Restricted {
                include_platform_defaults: true,
                readable_roots: vec![path],
            },
            network_access: true,
        }
    );
}

#[cfg(target_os = "macos")]
#[test]
fn effective_permissions_merge_macos_extensions_with_additional_permissions() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let path = AbsolutePathBuf::from_absolute_path(
        canonicalize(temp_dir.path()).expect("canonicalize temp dir"),
    )
    .expect("absolute temp dir");
    let effective_permissions = EffectiveSandboxPermissions::new(
        &SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::Restricted {
                include_platform_defaults: true,
                readable_roots: vec![path.clone()],
            },
            network_access: false,
        },
        Some(&MacOsSeatbeltProfileExtensions {
            macos_preferences: MacOsPreferencesPermission::ReadOnly,
            macos_automation: MacOsAutomationPermission::BundleIds(vec![
                "com.apple.Calendar".to_string(),
            ]),
            macos_launch_services: false,
            macos_accessibility: false,
            macos_calendar: false,
            macos_reminders: false,
            macos_contacts: MacOsContactsPermission::None,
        }),
        Some(&PermissionProfile {
            file_system: Some(FileSystemPermissions {
                read: Some(vec![path]),
                write: Some(Vec::new()),
            }),
            macos: Some(MacOsSeatbeltProfileExtensions {
                macos_preferences: MacOsPreferencesPermission::ReadWrite,
                macos_automation: MacOsAutomationPermission::BundleIds(vec![
                    "com.apple.Notes".to_string(),
                ]),
                macos_launch_services: true,
                macos_accessibility: true,
                macos_calendar: true,
                macos_reminders: false,
                macos_contacts: MacOsContactsPermission::None,
            }),
            ..Default::default()
        }),
    );

    assert_eq!(
        effective_permissions.macos_seatbelt_profile_extensions,
        Some(MacOsSeatbeltProfileExtensions {
            macos_preferences: MacOsPreferencesPermission::ReadWrite,
            macos_automation: MacOsAutomationPermission::BundleIds(vec![
                "com.apple.Calendar".to_string(),
                "com.apple.Notes".to_string(),
            ]),
            macos_launch_services: true,
            macos_accessibility: true,
            macos_calendar: true,
            macos_reminders: false,
            macos_contacts: MacOsContactsPermission::None,
        })
    );
}

#[test]
fn external_sandbox_additional_permissions_can_enable_network() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let path = AbsolutePathBuf::from_absolute_path(
        canonicalize(temp_dir.path()).expect("canonicalize temp dir"),
    )
    .expect("absolute temp dir");
    let policy = sandbox_policy_with_additional_permissions(
        &SandboxPolicy::ExternalSandbox {
            network_access: NetworkAccess::Restricted,
        },
        &PermissionProfile {
            network: Some(NetworkPermissions {
                enabled: Some(true),
            }),
            file_system: Some(FileSystemPermissions {
                read: Some(vec![path]),
                write: Some(Vec::new()),
            }),
            ..Default::default()
        },
    );

    assert_eq!(
        policy,
        SandboxPolicy::ExternalSandbox {
            network_access: NetworkAccess::Enabled,
        }
    );
}

#[test]
fn merge_file_system_policy_with_additional_permissions_preserves_unreadable_roots() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let cwd = AbsolutePathBuf::from_absolute_path(
        canonicalize(temp_dir.path()).expect("canonicalize temp dir"),
    )
    .expect("absolute temp dir");
    let allowed_path = cwd.join("allowed").expect("allowed path");
    let denied_path = cwd.join("denied").expect("denied path");
    let merged_policy = merge_file_system_policy_with_additional_permissions(
        &FileSystemSandboxPolicy::restricted(vec![
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
        vec![allowed_path.clone()],
        Vec::new(),
    );

    assert_eq!(
        merged_policy.entries.contains(&FileSystemSandboxEntry {
            path: FileSystemPath::Path { path: denied_path },
            access: FileSystemAccessMode::None,
        }),
        true
    );
    assert_eq!(
        merged_policy.entries.contains(&FileSystemSandboxEntry {
            path: FileSystemPath::Path { path: allowed_path },
            access: FileSystemAccessMode::Read,
        }),
        true
    );
}

#[test]
fn effective_file_system_sandbox_policy_returns_base_policy_without_additional_permissions() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let cwd = AbsolutePathBuf::from_absolute_path(
        canonicalize(temp_dir.path()).expect("canonicalize temp dir"),
    )
    .expect("absolute temp dir");
    let denied_path = cwd.join("denied").expect("denied path");
    let base_policy = FileSystemSandboxPolicy::restricted(vec![
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
    ]);

    let effective_policy = effective_file_system_sandbox_policy(&base_policy, None);

    assert_eq!(effective_policy, base_policy);
}

#[test]
fn effective_file_system_sandbox_policy_merges_additional_write_roots() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let cwd = AbsolutePathBuf::from_absolute_path(
        canonicalize(temp_dir.path()).expect("canonicalize temp dir"),
    )
    .expect("absolute temp dir");
    let allowed_path = cwd.join("allowed").expect("allowed path");
    let denied_path = cwd.join("denied").expect("denied path");
    let base_policy = FileSystemSandboxPolicy::restricted(vec![
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
    ]);
    let additional_permissions = PermissionProfile {
        file_system: Some(FileSystemPermissions {
            read: Some(vec![]),
            write: Some(vec![allowed_path.clone()]),
        }),
        ..Default::default()
    };

    let effective_policy =
        effective_file_system_sandbox_policy(&base_policy, Some(&additional_permissions));

    assert_eq!(
        effective_policy.entries.contains(&FileSystemSandboxEntry {
            path: FileSystemPath::Path { path: denied_path },
            access: FileSystemAccessMode::None,
        }),
        true
    );
    assert_eq!(
        effective_policy.entries.contains(&FileSystemSandboxEntry {
            path: FileSystemPath::Path { path: allowed_path },
            access: FileSystemAccessMode::Write,
        }),
        true
    );
}
