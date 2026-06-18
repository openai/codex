use super::SandboxCommand;
#[cfg(target_os = "windows")]
use super::SandboxDirectSpawnTransformRequest;
use super::SandboxManager;
use super::SandboxTransformRequest;
use super::SandboxType;
use super::SandboxablePreference;
use super::can_read_path_with_policy;
use super::get_platform_sandbox;
use super::read_deny_glob_matcher;
use super::with_managed_mitm_ca_proxy_dirs_denied;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use super::with_managed_mitm_ca_readable_roots;
#[cfg(target_os = "windows")]
use codex_network_proxy::NetworkProxy;
#[cfg(target_os = "windows")]
use codex_network_proxy::NetworkProxyConfig;
#[cfg(target_os = "windows")]
use codex_network_proxy::NetworkProxyConstraints;
#[cfg(target_os = "windows")]
use codex_network_proxy::build_config_state;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::NetworkPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use codex_protocol::permissions::ReadDenyMatcher;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use dunce::canonicalize;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
#[cfg(target_os = "windows")]
use std::sync::Arc;
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
fn unsandboxed_transform_preserves_foreign_cwd_and_unrestricted_file_system_policy() {
    let manager = SandboxManager::new();
    let cwd_uri = if cfg!(windows) {
        PathUri::parse("file:///workspace/remote").expect("POSIX path URI")
    } else {
        PathUri::parse("file:///C:/workspace/remote").expect("Windows path URI")
    };
    let permissions = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::unrestricted(),
        NetworkSandboxPolicy::Restricted,
    );
    let exec_request = manager
        .transform(SandboxTransformRequest {
            command: SandboxCommand {
                program: "true".into(),
                args: Vec::new(),
                cwd: cwd_uri.clone(),
                env: HashMap::new(),
                additional_permissions: None,
            },
            permissions: &permissions,
            sandbox: SandboxType::None,
            enforce_managed_network: false,
            environment_id: None,
            network: None,
            sandbox_policy_cwd: &cwd_uri,
            codex_linux_sandbox_exe: None,
            use_legacy_landlock: false,
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            windows_sandbox_private_desktop: false,
        })
        .expect("transform");

    assert_eq!(exec_request.cwd, cwd_uri);
    assert_eq!(exec_request.sandbox_policy_cwd, cwd_uri);
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
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let permissions = PermissionProfile::External {
        network: NetworkSandboxPolicy::Restricted,
    };
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
                cwd: cwd_uri.clone(),
                env: HashMap::new(),
                additional_permissions: Some(AdditionalPermissionProfile {
                    network: Some(NetworkPermissions {
                        enabled: Some(true),
                    }),
                    file_system: Some(FileSystemPermissions::from_read_write_roots(
                        Some(vec![path]),
                        Some(Vec::new()),
                    )),
                }),
            },
            permissions: &permissions,
            sandbox: SandboxType::None,
            enforce_managed_network: false,
            environment_id: None,
            network: None,
            sandbox_policy_cwd: &cwd_uri,
            codex_linux_sandbox_exe: None,
            use_legacy_landlock: false,
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            windows_sandbox_private_desktop: false,
        })
        .expect("transform");

    assert_eq!(
        exec_request.permission_profile,
        PermissionProfile::External {
            network: NetworkSandboxPolicy::Enabled,
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
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let temp_dir = TempDir::new().expect("create temp dir");
    let workspace_root = AbsolutePathBuf::from_absolute_path(
        canonicalize(temp_dir.path()).expect("canonicalize temp dir"),
    )
    .expect("absolute temp dir");
    let allowed_path = workspace_root.join("allowed");
    let denied_path = workspace_root.join("denied");
    let file_system_policy = FileSystemSandboxPolicy::restricted(vec![
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
            access: FileSystemAccessMode::Deny,
        },
    ]);
    let permissions = PermissionProfile::from_runtime_permissions(
        &file_system_policy,
        NetworkSandboxPolicy::Restricted,
    );
    let exec_request = manager
        .transform(SandboxTransformRequest {
            command: SandboxCommand {
                program: "true".into(),
                args: Vec::new(),
                cwd: cwd_uri.clone(),
                env: HashMap::new(),
                additional_permissions: Some(AdditionalPermissionProfile {
                    file_system: Some(FileSystemPermissions::from_read_write_roots(
                        /*read*/ None,
                        Some(vec![allowed_path.clone()]),
                    )),
                    ..Default::default()
                }),
            },
            permissions: &permissions,
            sandbox: SandboxType::None,
            enforce_managed_network: false,
            environment_id: None,
            network: None,
            sandbox_policy_cwd: &cwd_uri,
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
                access: FileSystemAccessMode::Deny,
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

#[test]
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn managed_mitm_ca_bundle_is_only_readable_carveback_in_proxy_dir() {
    let cwd = TempDir::new().expect("create cwd");
    let cwd =
        AbsolutePathBuf::from_absolute_path(canonicalize(cwd.path()).expect("canonicalize cwd"))
            .expect("absolute cwd");
    let managed_bundle_dir = TempDir::new().expect("create managed bundle dir");
    let managed_bundle_dir = AbsolutePathBuf::from_absolute_path(
        canonicalize(managed_bundle_dir.path()).expect("canonicalize managed bundle dir"),
    )
    .expect("absolute managed bundle dir");
    let managed_bundle_path = managed_bundle_dir.join("ca-bundle-active.pem");
    let previous_bundle_path = managed_bundle_dir.join("ca-bundle-previous.pem");
    let managed_ca_key_path = managed_bundle_dir.join("ca.key");
    for path in [
        &managed_bundle_path,
        &previous_bundle_path,
        &managed_ca_key_path,
    ] {
        std::fs::write(path.as_path(), "fixture").expect("write managed CA fixture");
    }
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Read,
        }]),
        NetworkSandboxPolicy::Restricted,
    );

    let permission_profile = with_managed_mitm_ca_proxy_dirs_denied(
        permission_profile,
        std::slice::from_ref(&managed_bundle_path),
        cwd.as_path(),
    )
    .expect("managed bundle directory should be outside writable roots");
    let (file_system_sandbox_policy, _) = permission_profile.to_runtime_permissions();
    let read_deny_matcher =
        ReadDenyMatcher::new(&file_system_sandbox_policy, cwd.as_path()).expect("deny matcher");
    for path in [
        &managed_bundle_path,
        &previous_bundle_path,
        &managed_ca_key_path,
    ] {
        assert!(!can_read_path_with_policy(
            &file_system_sandbox_policy,
            Some(&read_deny_matcher),
            path.as_path(),
            cwd.as_path(),
        ));
    }

    let permission_profile = with_managed_mitm_ca_readable_roots(
        permission_profile,
        std::slice::from_ref(&managed_bundle_path),
        cwd.as_path(),
    );
    let (file_system_sandbox_policy, _) = permission_profile.to_runtime_permissions();
    let read_deny_glob_matcher = read_deny_glob_matcher(&file_system_sandbox_policy, cwd.as_path());

    assert_eq!(
        file_system_sandbox_policy,
        FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Read,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: managed_bundle_dir,
                },
                access: FileSystemAccessMode::Deny,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: managed_bundle_path.clone(),
                },
                access: FileSystemAccessMode::Read,
            },
        ])
    );
    assert!(can_read_path_with_policy(
        &file_system_sandbox_policy,
        read_deny_glob_matcher.as_ref(),
        managed_bundle_path.as_path(),
        cwd.as_path(),
    ));
    assert!(
        !file_system_sandbox_policy
            .can_read_path_with_cwd(previous_bundle_path.as_path(), cwd.as_path(),)
    );
    assert!(
        !file_system_sandbox_policy
            .can_read_path_with_cwd(managed_ca_key_path.as_path(), cwd.as_path(),)
    );
}

#[cfg(unix)]
#[test]
fn managed_mitm_ca_materialization_checks_canonical_target_policy() {
    use std::os::unix::fs::symlink;

    let root = TempDir::new().expect("create policy root");
    let root = AbsolutePathBuf::from_absolute_path(
        canonicalize(root.path()).expect("canonicalize policy root"),
    )
    .expect("absolute policy root");
    let denied_dir = root.join("secrets");
    std::fs::create_dir(denied_dir.as_path()).expect("create denied dir");
    let denied_ca = denied_dir.join("ca.pem");
    std::fs::write(denied_ca.as_path(), "secret CA").expect("write denied CA");
    let readable_alias = root.join("readable-ca.pem");
    symlink(denied_ca.as_path(), readable_alias.as_path()).expect("create readable alias");
    let file_system_sandbox_policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Path { path: root.clone() },
            access: FileSystemAccessMode::Read,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path { path: denied_dir },
            access: FileSystemAccessMode::Deny,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: readable_alias.clone(),
            },
            access: FileSystemAccessMode::Read,
        },
    ]);

    assert!(
        file_system_sandbox_policy.can_read_path_with_cwd(readable_alias.as_path(), root.as_path()),
        "the lexical alias is explicitly readable"
    );
    assert!(!can_read_path_with_policy(
        &file_system_sandbox_policy,
        /*read_deny_glob_matcher*/ None,
        readable_alias.as_path(),
        root.as_path(),
    ));
}

#[test]
fn managed_mitm_ca_proxy_dir_deny_preserves_profiles_without_restricted_filesystem() {
    let managed_bundle_dir = TempDir::new().expect("create managed bundle dir");
    let managed_bundle_path =
        AbsolutePathBuf::from_absolute_path(managed_bundle_dir.path().join("ca-bundle.pem"))
            .expect("absolute managed bundle path");

    for permission_profile in [
        PermissionProfile::Disabled,
        PermissionProfile::from_runtime_permissions(
            &FileSystemSandboxPolicy::unrestricted(),
            NetworkSandboxPolicy::Restricted,
        ),
        PermissionProfile::External {
            network: NetworkSandboxPolicy::Restricted,
        },
    ] {
        assert_eq!(
            with_managed_mitm_ca_proxy_dirs_denied(
                permission_profile.clone(),
                std::slice::from_ref(&managed_bundle_path),
                managed_bundle_dir.path(),
            )
            .expect("profile should remain supported"),
            permission_profile,
        );
    }
}

#[test]
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn managed_mitm_ca_proxy_dir_rejects_writable_overlap() {
    let writable_root = TempDir::new().expect("create writable root");
    let proxy_dir = writable_root.path().join("proxy");
    std::fs::create_dir(&proxy_dir).expect("create proxy dir");
    let managed_bundle_path =
        AbsolutePathBuf::from_absolute_path(proxy_dir.join("ca-bundle-active.pem"))
            .expect("absolute managed bundle path");
    let writable_key = AbsolutePathBuf::from_absolute_path(proxy_dir.join("ca.key"))
        .expect("absolute writable key path");
    std::fs::write(writable_key.as_path(), "secret key").expect("write managed key fixture");
    let writable_root = AbsolutePathBuf::from_absolute_path(
        canonicalize(writable_root.path()).expect("canonicalize writable root"),
    )
    .expect("absolute writable root");
    let policies = [
        FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: writable_root.clone(),
            },
            access: FileSystemAccessMode::Write,
        }]),
        FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Read,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path { path: writable_key },
                access: FileSystemAccessMode::Write,
            },
        ]),
        FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Write,
        }]),
    ];

    for policy in policies {
        let permission_profile =
            PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted);
        assert!(matches!(
            with_managed_mitm_ca_proxy_dirs_denied(
                permission_profile,
                std::slice::from_ref(&managed_bundle_path),
                writable_root.as_path(),
            ),
            Err(super::SandboxTransformError::ManagedMitmCaPathUnderWritableRoot)
        ));
    }
}

#[test]
#[cfg(target_os = "macos")]
fn managed_mitm_ca_proxy_dir_rejects_platform_default_writable_ancestor() {
    let proxy_dir = tempfile::tempdir_in("/private/tmp").expect("create proxy dir");
    let managed_bundle_path =
        AbsolutePathBuf::from_absolute_path(proxy_dir.path().join("ca-bundle-active.pem"))
            .expect("absolute managed bundle path");
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Minimal,
            },
            access: FileSystemAccessMode::Read,
        }]),
        NetworkSandboxPolicy::Restricted,
    );

    assert!(matches!(
        with_managed_mitm_ca_proxy_dirs_denied(
            permission_profile,
            std::slice::from_ref(&managed_bundle_path),
            proxy_dir.path(),
        ),
        Err(super::SandboxTransformError::ManagedMitmCaPathUnderWritableRoot)
    ));
}

#[test]
fn managed_mitm_ca_materialization_rejects_glob_denied_paths_from_command_subdir() {
    let sandbox_policy_cwd = TempDir::new().expect("create cwd");
    let sandbox_policy_cwd = AbsolutePathBuf::from_absolute_path(
        canonicalize(sandbox_policy_cwd.path()).expect("canonicalize cwd"),
    )
    .expect("absolute cwd");
    let command_cwd = sandbox_policy_cwd.join("subdir");
    std::fs::create_dir(command_cwd.as_path()).expect("create command cwd");
    let ca_bundle_path = command_cwd.join("../secrets/blocked.pem");
    std::fs::create_dir(sandbox_policy_cwd.join("secrets").as_path()).expect("create secrets");
    std::fs::write(ca_bundle_path.as_path(), "secret").expect("write blocked CA bundle");
    let file_system_sandbox_policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: sandbox_policy_cwd.clone(),
            },
            access: FileSystemAccessMode::Read,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::GlobPattern {
                pattern: "secrets/**".to_string(),
            },
            access: FileSystemAccessMode::Deny,
        },
    ]);
    let read_deny_glob_matcher =
        read_deny_glob_matcher(&file_system_sandbox_policy, sandbox_policy_cwd.as_path())
            .expect("deny glob matcher");

    assert!(!can_read_path_with_policy(
        &file_system_sandbox_policy,
        Some(&read_deny_glob_matcher),
        ca_bundle_path.as_path(),
        sandbox_policy_cwd.as_path(),
    ));
}

#[cfg(target_os = "linux")]
fn transform_linux_seccomp_request(
    codex_linux_sandbox_exe: &std::path::Path,
) -> super::SandboxExecRequest {
    let manager = SandboxManager::new();
    let cwd = AbsolutePathBuf::current_dir().expect("current dir");
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let permissions = PermissionProfile::Disabled;
    manager
        .transform(SandboxTransformRequest {
            command: SandboxCommand {
                program: "true".into(),
                args: Vec::new(),
                cwd: cwd_uri.clone(),
                env: HashMap::new(),
                additional_permissions: None,
            },
            permissions: &permissions,
            sandbox: SandboxType::LinuxSeccomp,
            enforce_managed_network: false,
            environment_id: None,
            network: None,
            sandbox_policy_cwd: &cwd_uri,
            codex_linux_sandbox_exe: Some(codex_linux_sandbox_exe),
            use_legacy_landlock: false,
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            windows_sandbox_private_desktop: false,
        })
        .expect("transform")
}

#[cfg(target_os = "linux")]
#[test]
fn wsl1_rejects_linux_bubblewrap_path() {
    let restricted_policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: FileSystemPath::Special {
            value: FileSystemSpecialPath::Root,
        },
        access: FileSystemAccessMode::Read,
    }]);

    assert!(matches!(
        super::ensure_linux_bubblewrap_is_supported(
            &restricted_policy,
            /*use_legacy_landlock*/ false,
            /*allow_network_for_proxy*/ false,
            /*managed_mitm_ca_active*/ false,
            /*is_wsl1*/ true,
        ),
        Err(super::SandboxTransformError::Wsl1UnsupportedForBubblewrap)
    ));
    assert!(matches!(
        super::ensure_linux_bubblewrap_is_supported(
            &FileSystemSandboxPolicy::unrestricted(),
            /*use_legacy_landlock*/ false,
            /*allow_network_for_proxy*/ true,
            /*managed_mitm_ca_active*/ false,
            /*is_wsl1*/ true,
        ),
        Err(super::SandboxTransformError::Wsl1UnsupportedForBubblewrap)
    ));
}

#[cfg(target_os = "linux")]
#[test]
fn wsl1_allows_non_bubblewrap_linux_paths() {
    assert!(
        super::ensure_linux_bubblewrap_is_supported(
            &FileSystemSandboxPolicy::unrestricted(),
            /*use_legacy_landlock*/ false,
            /*allow_network_for_proxy*/ false,
            /*managed_mitm_ca_active*/ false,
            /*is_wsl1*/ true,
        )
        .is_ok()
    );

    let restricted_policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: FileSystemPath::Special {
            value: FileSystemSpecialPath::Root,
        },
        access: FileSystemAccessMode::Read,
    }]);
    assert!(
        super::ensure_linux_bubblewrap_is_supported(
            &restricted_policy,
            /*use_legacy_landlock*/ true,
            /*allow_network_for_proxy*/ false,
            /*managed_mitm_ca_active*/ false,
            /*is_wsl1*/ true,
        )
        .is_ok()
    );
}

#[cfg(target_os = "linux")]
#[test]
fn legacy_landlock_rejects_managed_mitm_ca_isolation_for_restricted_profiles() {
    let restricted_policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: FileSystemPath::Special {
            value: FileSystemSpecialPath::Root,
        },
        access: FileSystemAccessMode::Read,
    }]);

    assert!(matches!(
        super::ensure_linux_bubblewrap_is_supported(
            &restricted_policy,
            /*use_legacy_landlock*/ true,
            /*allow_network_for_proxy*/ false,
            /*managed_mitm_ca_active*/ true,
            /*is_wsl1*/ false,
        ),
        Err(super::SandboxTransformError::LegacyLandlockUnsupportedWithManagedMitm)
    ));
    assert!(
        super::ensure_linux_bubblewrap_is_supported(
            &restricted_policy,
            /*use_legacy_landlock*/ false,
            /*allow_network_for_proxy*/ false,
            /*managed_mitm_ca_active*/ true,
            /*is_wsl1*/ false,
        )
        .is_ok()
    );
    assert!(
        super::ensure_linux_bubblewrap_is_supported(
            &restricted_policy,
            /*use_legacy_landlock*/ true,
            /*allow_network_for_proxy*/ false,
            /*managed_mitm_ca_active*/ false,
            /*is_wsl1*/ false,
        )
        .is_ok()
    );
    assert!(
        super::ensure_linux_bubblewrap_is_supported(
            &FileSystemSandboxPolicy::unrestricted(),
            /*use_legacy_landlock*/ true,
            /*allow_network_for_proxy*/ false,
            /*managed_mitm_ca_active*/ true,
            /*is_wsl1*/ false,
        )
        .is_ok()
    );
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

#[cfg(target_os = "windows")]
#[test]
fn transform_for_direct_spawn_windows_preserves_only_wrapper_setup_identity() {
    let mut env = HashMap::from([
        ("Path".to_string(), r"C:\Windows\System32".to_string()),
        ("username".to_string(), "wrong-user".to_string()),
        ("UserProfile".to_string(), r"C:\wrong".to_string()),
    ]);

    super::add_windows_sandbox_wrapper_setup_env_from_vars(
        &mut env,
        [
            ("USERNAME", "alice"),
            ("USERPROFILE", r"C:\Users\alice"),
            ("OPENAI_API_KEY", "secret"),
        ]
        .map(|(key, value)| {
            (
                std::ffi::OsString::from(key),
                std::ffi::OsString::from(value),
            )
        }),
    );

    assert_eq!(
        env,
        HashMap::from([
            ("Path".to_string(), r"C:\Windows\System32".to_string()),
            ("USERNAME".to_string(), "alice".to_string()),
            ("USERPROFILE".to_string(), r"C:\Users\alice".to_string()),
        ])
    );
}

#[cfg(target_os = "windows")]
#[tokio::test]
async fn windows_restricted_transform_rejects_command_specific_ca() {
    let cwd = TempDir::new().expect("create cwd");
    let cwd = AbsolutePathBuf::from_absolute_path(cwd.path()).expect("absolute cwd");
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let mut config = NetworkProxyConfig::default();
    config.network.mitm = true;
    let state = Arc::new(
        build_config_state(config, NetworkProxyConstraints::default()).expect("build proxy state"),
    );
    let network = NetworkProxy::builder()
        .state(state)
        .managed_by_codex(/*managed_by_codex*/ false)
        .build()
        .await
        .expect("build proxy");
    let permissions = PermissionProfile::read_only();

    let err = SandboxManager::new()
        .transform(SandboxTransformRequest {
            command: SandboxCommand {
                program: "cmd.exe".into(),
                args: vec!["/c".to_string(), "exit 0".to_string()],
                cwd: cwd_uri.clone(),
                env: HashMap::from([(
                    "REQUESTS_CA_BUNDLE".to_string(),
                    r"C:\command-ca.pem".to_string(),
                )]),
                additional_permissions: None,
            },
            permissions: &permissions,
            sandbox: SandboxType::WindowsRestrictedToken,
            enforce_managed_network: true,
            environment_id: None,
            network: Some(&network),
            sandbox_policy_cwd: &cwd_uri,
            codex_linux_sandbox_exe: None,
            use_legacy_landlock: false,
            windows_sandbox_level: WindowsSandboxLevel::Elevated,
            windows_sandbox_private_desktop: false,
        })
        .expect_err("command-specific CA should be rejected");

    assert!(matches!(
        err,
        super::SandboxTransformError::ManagedMitmCustomCaUnsupportedOnWindows
    ));
}

#[cfg(target_os = "windows")]
#[test]
fn transform_for_direct_spawn_windows_materializes_inner_helper() {
    let codex_home = tempfile::TempDir::new().expect("codex home");
    let helper_dir = tempfile::TempDir::new().expect("helper dir");
    let configured_helper = helper_dir.path().join("configured-codex-helper.exe");
    std::fs::write(&configured_helper, b"helper").expect("write configured helper");
    let cwd = AbsolutePathBuf::from_absolute_path(helper_dir.path()).expect("absolute cwd");
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let blocked = cwd.join("blocked");
    std::fs::create_dir_all(blocked.as_path()).expect("create blocked path");
    let permissions = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Read,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                },
                access: FileSystemAccessMode::Write,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path { path: blocked },
                access: FileSystemAccessMode::Deny,
            },
        ]),
        NetworkSandboxPolicy::Restricted,
    );
    let other_workspace = tempfile::TempDir::new().expect("other workspace");
    let other_workspace_root = AbsolutePathBuf::from_absolute_path(other_workspace.path())
        .expect("absolute other workspace");
    let workspace_roots = vec![cwd, other_workspace_root];
    let manager = SandboxManager::new();
    let exec_request = manager
        .transform_for_direct_spawn_with_codex_home(
            SandboxDirectSpawnTransformRequest {
                workspace_roots: workspace_roots.as_slice(),
                transform: SandboxTransformRequest {
                    command: SandboxCommand {
                        program: configured_helper.as_os_str().to_owned(),
                        args: vec!["--codex-run-as-fs-helper".to_string()],
                        cwd: cwd_uri.clone(),
                        env: HashMap::from([(
                            "Path".to_string(),
                            r"C:\Windows\System32".to_string(),
                        )]),
                        additional_permissions: None,
                    },
                    permissions: &permissions,
                    sandbox: SandboxType::WindowsRestrictedToken,
                    enforce_managed_network: false,
                    environment_id: None,
                    network: None,
                    sandbox_policy_cwd: &cwd_uri,
                    codex_linux_sandbox_exe: None,
                    use_legacy_landlock: false,
                    windows_sandbox_level: WindowsSandboxLevel::Elevated,
                    windows_sandbox_private_desktop: false,
                },
            },
            codex_home.path(),
        )
        .expect("transform for direct spawn");

    let separator_index = exec_request
        .command
        .iter()
        .position(|arg| arg == "--")
        .expect("wrapper argv separator");
    let materialized_helper = std::path::PathBuf::from(&exec_request.command[separator_index + 1]);
    assert_eq!(exec_request.sandbox, SandboxType::None);
    assert_eq!(
        exec_request.command.first(),
        Some(&configured_helper.display().to_string())
    );
    assert!(
        exec_request
            .command
            .iter()
            .any(|arg| arg == "--run-as-windows-sandbox")
    );
    assert!(
        exec_request
            .command
            .iter()
            .any(|arg| arg == "--deny-read-paths-json")
    );
    assert_eq!(
        exec_request.command[separator_index + 2],
        "--codex-run-as-fs-helper"
    );
    assert_eq!(
        exec_request
            .command
            .windows(2)
            .filter_map(|args| {
                (args[0] == "--workspace-root").then_some(std::path::PathBuf::from(&args[1]))
            })
            .collect::<Vec<_>>(),
        workspace_roots
            .iter()
            .map(|root| root.as_path().to_path_buf())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        materialized_helper
            .parent()
            .and_then(std::path::Path::file_name),
        Some(std::ffi::OsStr::new(".sandbox-bin"))
    );
    assert!(materialized_helper.exists());
}
