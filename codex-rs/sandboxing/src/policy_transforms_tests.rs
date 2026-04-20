use super::effective_file_system_sandbox_policy;
use super::intersect_permission_profiles;
use super::merge_file_system_policy_with_additional_permissions;
use super::normalize_additional_permissions;
use super::sandbox_policy_with_additional_permissions;
use super::should_require_platform_sandbox;
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
        should_require_platform_sandbox(
            &policy,
            NetworkSandboxPolicy::Enabled,
            /*has_managed_network_requirements*/ false
        ),
        false
    );
}

#[test]
fn root_write_policy_with_carveouts_still_uses_platform_sandbox() {
    let blocked = AbsolutePathBuf::resolve_path_against_base(
        "blocked",
        std::env::current_dir().expect("current dir"),
    );
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
        should_require_platform_sandbox(
            &policy,
            NetworkSandboxPolicy::Enabled,
            /*has_managed_network_requirements*/ false
        ),
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
        should_require_platform_sandbox(
            &policy,
            NetworkSandboxPolicy::Restricted,
            /*has_managed_network_requirements*/ false
        ),
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
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![path.clone()]),
            Some(vec![path.clone()]),
        )),
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
        Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![path.clone()]),
            Some(vec![path]),
        ))
    );
}

#[cfg(unix)]
#[test]
fn normalize_additional_permissions_preserves_symlinked_write_paths() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let real_root = temp_dir.path().join("real");
    let link_root = temp_dir.path().join("link");
    let write_dir = real_root.join("write");
    std::fs::create_dir_all(&write_dir).expect("create write dir");
    symlink_dir(&real_root, &link_root).expect("create symlinked root");

    let link_write_dir =
        AbsolutePathBuf::from_absolute_path(link_root.join("write")).expect("link write dir");
    let permissions = normalize_additional_permissions(PermissionProfile {
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![]),
            Some(vec![link_write_dir]),
        )),
        ..Default::default()
    })
    .expect("permissions");

    assert_eq!(
        permissions.file_system,
        Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![]),
            Some(vec![
                AbsolutePathBuf::from_absolute_path(link_root.join("write"))
                    .expect("link write dir"),
            ]),
        ))
    );
}

#[test]
fn normalize_additional_permissions_rejects_glob_read_grants() {
    let err = normalize_additional_permissions(PermissionProfile {
        file_system: Some(FileSystemPermissions {
            entries: vec![FileSystemSandboxEntry {
                path: FileSystemPath::GlobPattern {
                    pattern: "**/*.env".to_string(),
                },
                access: FileSystemAccessMode::Read,
            }],
            glob_scan_max_depth: None,
        }),
        ..Default::default()
    })
    .expect_err("read glob permissions are unsupported");

    assert_eq!(
        err,
        "glob file system permissions only support deny-read entries"
    );
}

#[test]
fn normalize_additional_permissions_preserves_deny_globs() {
    let permissions = normalize_additional_permissions(PermissionProfile {
        file_system: Some(FileSystemPermissions {
            entries: vec![FileSystemSandboxEntry {
                path: FileSystemPath::GlobPattern {
                    pattern: "**/*.env".to_string(),
                },
                access: FileSystemAccessMode::None,
            }],
            glob_scan_max_depth: Some(2),
        }),
        ..Default::default()
    })
    .expect("deny glob permissions are supported");

    assert_eq!(
        permissions,
        PermissionProfile {
            file_system: Some(FileSystemPermissions {
                entries: vec![FileSystemSandboxEntry {
                    path: FileSystemPath::GlobPattern {
                        pattern: "**/*.env".to_string(),
                    },
                    access: FileSystemAccessMode::None,
                }],
                glob_scan_max_depth: Some(2),
            }),
            ..Default::default()
        }
    );
}

#[test]
fn normalize_additional_permissions_drops_empty_nested_profiles() {
    let permissions = normalize_additional_permissions(PermissionProfile {
        network: Some(NetworkPermissions { enabled: None }),
        file_system: Some(FileSystemPermissions::default()),
    })
    .expect("permissions");

    assert_eq!(permissions, PermissionProfile::default());
}

#[test]
fn intersect_permission_profiles_preserves_explicit_empty_requested_reads() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let path = AbsolutePathBuf::from_absolute_path(
        canonicalize(temp_dir.path()).expect("canonicalize temp dir"),
    )
    .expect("absolute temp dir");
    let requested = PermissionProfile {
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![]),
            Some(vec![path]),
        )),
        ..Default::default()
    };
    let granted = requested.clone();

    assert_eq!(
        intersect_permission_profiles(requested.clone(), granted, temp_dir.path()),
        requested
    );
}

#[test]
fn intersect_permission_profiles_drops_ungranted_nonempty_path_requests() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let path = AbsolutePathBuf::from_absolute_path(
        canonicalize(temp_dir.path()).expect("canonicalize temp dir"),
    )
    .expect("absolute temp dir");
    let requested = PermissionProfile {
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![path]),
            /*write*/ None,
        )),
        ..Default::default()
    };

    assert_eq!(
        intersect_permission_profiles(requested, PermissionProfile::default(), temp_dir.path()),
        PermissionProfile::default()
    );
}

#[test]
fn intersect_permission_profiles_drops_explicit_empty_reads_without_grant() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let path = AbsolutePathBuf::from_absolute_path(
        canonicalize(temp_dir.path()).expect("canonicalize temp dir"),
    )
    .expect("absolute temp dir");
    let requested = PermissionProfile {
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![]),
            Some(vec![path]),
        )),
        ..Default::default()
    };

    assert_eq!(
        intersect_permission_profiles(requested, PermissionProfile::default(), temp_dir.path()),
        PermissionProfile::default()
    );
}

#[test]
fn intersect_permission_profiles_accepts_child_path_granted_for_requested_cwd() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let cwd = AbsolutePathBuf::from_absolute_path(
        canonicalize(temp_dir.path()).expect("canonicalize temp dir"),
    )
    .expect("absolute temp dir");
    let child = cwd.join("child");
    let requested = PermissionProfile {
        file_system: Some(FileSystemPermissions {
            entries: vec![FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::CurrentWorkingDirectory,
                },
                access: FileSystemAccessMode::Write,
            }],
            glob_scan_max_depth: None,
        }),
        ..Default::default()
    };
    let granted = PermissionProfile {
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            None,
            Some(vec![child.clone()]),
        )),
        ..Default::default()
    };

    assert_eq!(
        intersect_permission_profiles(requested, granted.clone(), cwd.as_path()),
        granted
    );
}

#[test]
fn intersect_permission_profiles_drops_broader_cwd_grant_for_requested_child_path() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let cwd = AbsolutePathBuf::from_absolute_path(
        canonicalize(temp_dir.path()).expect("canonicalize temp dir"),
    )
    .expect("absolute temp dir");
    let child = cwd.join("child");
    let requested = PermissionProfile {
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            None,
            Some(vec![child]),
        )),
        ..Default::default()
    };
    let granted = PermissionProfile {
        file_system: Some(FileSystemPermissions {
            entries: vec![FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::CurrentWorkingDirectory,
                },
                access: FileSystemAccessMode::Write,
            }],
            glob_scan_max_depth: None,
        }),
        ..Default::default()
    };

    assert_eq!(
        intersect_permission_profiles(requested, granted, cwd.as_path()),
        PermissionProfile::default()
    );
}

#[test]
fn intersect_permission_profiles_uses_granted_bounded_glob_scan_depth() {
    let cwd = std::env::current_dir().expect("current dir");
    let deny_env_files = FileSystemSandboxEntry {
        path: FileSystemPath::GlobPattern {
            pattern: "**/*.env".to_string(),
        },
        access: FileSystemAccessMode::None,
    };
    let requested = PermissionProfile {
        file_system: Some(FileSystemPermissions {
            entries: vec![deny_env_files.clone()],
            glob_scan_max_depth: Some(2),
        }),
        ..Default::default()
    };
    let granted = PermissionProfile {
        file_system: Some(FileSystemPermissions {
            entries: vec![deny_env_files.clone()],
            glob_scan_max_depth: Some(4),
        }),
        ..Default::default()
    };

    assert_eq!(
        intersect_permission_profiles(requested, granted, cwd.as_path()),
        PermissionProfile {
            file_system: Some(FileSystemPermissions {
                entries: vec![deny_env_files],
                glob_scan_max_depth: Some(4),
            }),
            ..Default::default()
        }
    );
}

#[test]
fn intersect_permission_profiles_uses_granted_unbounded_glob_scan_depth() {
    let cwd = std::env::current_dir().expect("current dir");
    let deny_env_files = FileSystemSandboxEntry {
        path: FileSystemPath::GlobPattern {
            pattern: "**/*.env".to_string(),
        },
        access: FileSystemAccessMode::None,
    };
    let requested = PermissionProfile {
        file_system: Some(FileSystemPermissions {
            entries: vec![deny_env_files.clone()],
            glob_scan_max_depth: Some(2),
        }),
        ..Default::default()
    };
    let granted = PermissionProfile {
        file_system: Some(FileSystemPermissions {
            entries: vec![deny_env_files.clone()],
            glob_scan_max_depth: None,
        }),
        ..Default::default()
    };

    assert_eq!(
        intersect_permission_profiles(requested, granted, cwd.as_path()),
        PermissionProfile {
            file_system: Some(FileSystemPermissions {
                entries: vec![deny_env_files],
                glob_scan_max_depth: None,
            }),
            ..Default::default()
        }
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
            file_system: Some(FileSystemPermissions::from_read_write_roots(
                Some(vec![path.clone()]),
                Some(Vec::new()),
            )),
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
            file_system: Some(FileSystemPermissions::from_read_write_roots(
                Some(vec![path]),
                Some(Vec::new()),
            )),
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
    let allowed_path = cwd.join("allowed");
    let denied_path = cwd.join("denied");
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
        &FileSystemPermissions::from_read_write_roots(
            Some(vec![allowed_path.clone()]),
            Some(Vec::new()),
        ),
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
fn merge_file_system_policy_with_additional_permissions_carries_bounded_glob_scan_depth() {
    let deny_env_files = FileSystemSandboxEntry {
        path: FileSystemPath::GlobPattern {
            pattern: "**/*.env".to_string(),
        },
        access: FileSystemAccessMode::None,
    };
    let merged_policy = merge_file_system_policy_with_additional_permissions(
        &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Write,
        }]),
        &FileSystemPermissions {
            entries: vec![deny_env_files.clone()],
            glob_scan_max_depth: Some(2),
        },
    );

    assert_eq!(merged_policy, {
        let mut policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Write,
            },
            deny_env_files,
        ]);
        policy.glob_scan_max_depth = Some(2);
        policy
    });
}

#[test]
fn effective_file_system_sandbox_policy_returns_base_policy_without_additional_permissions() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let cwd = AbsolutePathBuf::from_absolute_path(
        canonicalize(temp_dir.path()).expect("canonicalize temp dir"),
    )
    .expect("absolute temp dir");
    let denied_path = cwd.join("denied");
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

    let effective_policy =
        effective_file_system_sandbox_policy(&base_policy, /*additional_permissions*/ None);

    assert_eq!(effective_policy, base_policy);
}

#[test]
fn effective_file_system_sandbox_policy_merges_additional_write_roots() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let cwd = AbsolutePathBuf::from_absolute_path(
        canonicalize(temp_dir.path()).expect("canonicalize temp dir"),
    )
    .expect("absolute temp dir");
    let allowed_path = cwd.join("allowed");
    let denied_path = cwd.join("denied");
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
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![]),
            Some(vec![allowed_path.clone()]),
        )),
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
