use super::*;
use crate::config::Config;
use crate::config::ConfigOverrides;
use codex_config::config_toml::ConfigToml;
use codex_config::permissions_toml::FilesystemPermissionToml;
use codex_config::permissions_toml::FilesystemPermissionsToml;
use codex_config::permissions_toml::NetworkDomainPermissionToml;
use codex_config::permissions_toml::NetworkDomainPermissionsToml;
use codex_config::permissions_toml::NetworkToml;
use codex_config::permissions_toml::NetworkUnixSocketPermissionToml;
use codex_config::permissions_toml::NetworkUnixSocketPermissionsToml;
use codex_config::permissions_toml::PermissionProfileToml;
use codex_config::permissions_toml::PermissionsToml;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use tempfile::TempDir;

#[test]
fn normalize_absolute_path_for_platform_simplifies_windows_verbatim_paths() {
    let parsed = normalize_absolute_path_for_platform(
        r"\\?\D:\c\x\worktrees\2508\swift-base",
        /*is_windows*/ true,
    );
    assert_eq!(parsed, PathBuf::from(r"D:\c\x\worktrees\2508\swift-base"));
}

#[tokio::test]
async fn restricted_read_implicitly_allows_helper_executables() -> std::io::Result<()> {
    let temp_dir = TempDir::new()?;
    let cwd = temp_dir.path().join("workspace");
    let codex_home = temp_dir.path().join(".codex");
    let zsh_path = temp_dir.path().join("runtime").join("zsh");
    let arg0_root = codex_home.join("tmp").join("arg0");
    let allowed_arg0_dir = arg0_root.join("codex-arg0-session");
    let sibling_arg0_dir = arg0_root.join("codex-arg0-other-session");
    let execve_wrapper = allowed_arg0_dir.join("codex-execve-wrapper");
    std::fs::create_dir_all(&cwd)?;
    std::fs::create_dir_all(zsh_path.parent().expect("zsh path should have parent"))?;
    std::fs::create_dir_all(&allowed_arg0_dir)?;
    std::fs::create_dir_all(&sibling_arg0_dir)?;
    std::fs::write(&zsh_path, "")?;
    std::fs::write(&execve_wrapper, "")?;

    let config = Config::load_from_base_config_with_overrides(
        ConfigToml {
            default_permissions: Some("workspace".to_string()),
            permissions: Some(PermissionsToml {
                entries: BTreeMap::from([(
                    "workspace".to_string(),
                    PermissionProfileToml {
                        filesystem: Some(FilesystemPermissionsToml {
                            unreadable_glob_scan_max_depth: None,
                            entries: BTreeMap::new(),
                        }),
                        network: None,
                    },
                )]),
            }),
            ..Default::default()
        },
        ConfigOverrides {
            cwd: Some(cwd.clone()),
            zsh_path: Some(zsh_path.clone()),
            main_execve_wrapper_exe: Some(execve_wrapper),
            ..Default::default()
        },
        AbsolutePathBuf::from_absolute_path(&codex_home)?,
    )
    .await?;

    let expected_zsh = AbsolutePathBuf::try_from(zsh_path)?;
    let expected_allowed_arg0_dir = AbsolutePathBuf::try_from(allowed_arg0_dir)?;
    let expected_sibling_arg0_dir = AbsolutePathBuf::try_from(sibling_arg0_dir)?;
    let policy = &config.permissions.file_system_sandbox_policy;

    assert!(
        policy.can_read_path_with_cwd(expected_zsh.as_path(), &cwd),
        "expected zsh helper path to be readable, policy: {policy:?}"
    );
    assert!(
        policy.can_read_path_with_cwd(expected_allowed_arg0_dir.as_path(), &cwd),
        "expected active arg0 helper dir to be readable, policy: {policy:?}"
    );
    assert!(
        !policy.can_read_path_with_cwd(expected_sibling_arg0_dir.as_path(), &cwd),
        "expected sibling arg0 helper dir to remain unreadable, policy: {policy:?}"
    );

    Ok(())
}

#[test]
fn network_toml_ignores_legacy_network_list_keys() {
    let parsed = toml::from_str::<NetworkToml>(
        r#"
allowed_domains = ["openai.com"]
"#,
    )
    .expect("legacy network list keys should be ignored");

    assert_eq!(parsed, NetworkToml::default());
}

#[test]
fn network_permission_containers_project_allowed_and_denied_entries() {
    let domains = NetworkDomainPermissionsToml {
        entries: BTreeMap::from([
            (
                "*.openai.com".to_string(),
                NetworkDomainPermissionToml::Allow,
            ),
            (
                "api.example.com".to_string(),
                NetworkDomainPermissionToml::Allow,
            ),
            (
                "blocked.example.com".to_string(),
                NetworkDomainPermissionToml::Deny,
            ),
        ]),
    };
    let unix_sockets = NetworkUnixSocketPermissionsToml {
        entries: BTreeMap::from([
            (
                "/tmp/example.sock".to_string(),
                NetworkUnixSocketPermissionToml::Allow,
            ),
            (
                "/tmp/ignored.sock".to_string(),
                NetworkUnixSocketPermissionToml::None,
            ),
        ]),
    };

    assert_eq!(
        domains.allowed_domains(),
        Some(vec![
            "*.openai.com".to_string(),
            "api.example.com".to_string()
        ])
    );
    assert_eq!(
        domains.denied_domains(),
        Some(vec!["blocked.example.com".to_string()])
    );
    assert_eq!(
        NetworkDomainPermissionsToml {
            entries: BTreeMap::from([(
                "api.example.com".to_string(),
                NetworkDomainPermissionToml::Allow,
            )]),
        }
        .denied_domains(),
        None
    );
    assert_eq!(
        unix_sockets.allow_unix_sockets(),
        vec!["/tmp/example.sock".to_string()]
    );
}

#[test]
fn network_toml_overlays_unix_socket_permissions_by_path() {
    let mut config = NetworkProxyConfig::default();

    NetworkToml {
        unix_sockets: Some(NetworkUnixSocketPermissionsToml {
            entries: BTreeMap::from([
                (
                    "/tmp/base.sock".to_string(),
                    NetworkUnixSocketPermissionToml::Allow,
                ),
                (
                    "/tmp/override.sock".to_string(),
                    NetworkUnixSocketPermissionToml::Allow,
                ),
            ]),
        }),
        ..Default::default()
    }
    .apply_to_network_proxy_config(&mut config);

    NetworkToml {
        unix_sockets: Some(NetworkUnixSocketPermissionsToml {
            entries: BTreeMap::from([
                (
                    "/tmp/extra.sock".to_string(),
                    NetworkUnixSocketPermissionToml::Allow,
                ),
                (
                    "/tmp/override.sock".to_string(),
                    NetworkUnixSocketPermissionToml::None,
                ),
            ]),
        }),
        ..Default::default()
    }
    .apply_to_network_proxy_config(&mut config);

    assert_eq!(
        config.network.unix_sockets,
        Some(codex_network_proxy::NetworkUnixSocketPermissions {
            entries: BTreeMap::from([
                (
                    "/tmp/base.sock".to_string(),
                    ProxyNetworkUnixSocketPermission::Allow,
                ),
                (
                    "/tmp/extra.sock".to_string(),
                    ProxyNetworkUnixSocketPermission::Allow,
                ),
                (
                    "/tmp/override.sock".to_string(),
                    ProxyNetworkUnixSocketPermission::None,
                ),
            ]),
        })
    );
}

#[test]
fn read_write_glob_warnings_skip_supported_deny_read_globs_and_trailing_subpaths() {
    let filesystem = FilesystemPermissionsToml {
        unreadable_glob_scan_max_depth: None,
        entries: BTreeMap::from([
            (
                "/tmp/**/*.log".to_string(),
                FilesystemPermissionToml::Access(FileSystemAccessMode::Read),
            ),
            (
                "/tmp/cache/**".to_string(),
                FilesystemPermissionToml::Access(FileSystemAccessMode::Write),
            ),
            (
                ":project_roots".to_string(),
                FilesystemPermissionToml::Scoped(BTreeMap::from([
                    ("**/*.env".to_string(), FileSystemAccessMode::None),
                    ("docs/**".to_string(), FileSystemAccessMode::Read),
                    ("src/**/*.rs".to_string(), FileSystemAccessMode::Write),
                ])),
            ),
        ]),
    };

    assert_eq!(
        unsupported_read_write_glob_paths(&filesystem),
        vec![
            "/tmp/**/*.log".to_string(),
            ":project_roots/src/**/*.rs".to_string()
        ],
        "`none` glob patterns are supported as deny-read rules; only `read`/`write` globs should warn"
    );
}

#[test]
fn unreadable_globstar_warning_is_suppressed_when_scan_depth_is_configured() {
    let filesystem = FilesystemPermissionsToml {
        unreadable_glob_scan_max_depth: None,
        entries: BTreeMap::from([(
            ":project_roots".to_string(),
            FilesystemPermissionToml::Scoped(BTreeMap::from([
                ("**/*.env".to_string(), FileSystemAccessMode::None),
                ("*.pem".to_string(), FileSystemAccessMode::None),
            ])),
        )]),
    };

    assert_eq!(
        unbounded_unreadable_globstar_paths(&filesystem),
        vec![":project_roots/**/*.env".to_string()]
    );

    let configured_filesystem = FilesystemPermissionsToml {
        unreadable_glob_scan_max_depth: Some(2),
        ..filesystem
    };
    assert_eq!(
        unbounded_unreadable_globstar_paths(&configured_filesystem),
        Vec::<String>::new()
    );
}

#[test]
fn unreadable_glob_scan_max_depth_must_be_positive() {
    let err = validate_unreadable_glob_scan_max_depth(Some(0))
        .expect_err("zero depth would silently skip deny-read glob expansion");

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(
        err.to_string(),
        "unreadable_glob_scan_max_depth must be at least 1"
    );
    assert_eq!(
        validate_unreadable_glob_scan_max_depth(Some(2)).expect("depth should be valid"),
        Some(2)
    );
}

#[test]
fn read_write_trailing_glob_suffix_compiles_as_subpath() -> std::io::Result<()> {
    let cwd = TempDir::new()?;
    let mut startup_warnings = Vec::new();
    let (file_system_policy, _) = compile_permission_profile(
        &PermissionsToml {
            entries: BTreeMap::from([(
                "workspace".to_string(),
                PermissionProfileToml {
                    filesystem: Some(FilesystemPermissionsToml {
                        unreadable_glob_scan_max_depth: None,
                        entries: BTreeMap::from([(
                            ":project_roots".to_string(),
                            FilesystemPermissionToml::Scoped(BTreeMap::from([(
                                "docs/**".to_string(),
                                FileSystemAccessMode::Read,
                            )])),
                        )]),
                    }),
                    network: None,
                },
            )]),
        },
        "workspace",
        cwd.path(),
        &mut startup_warnings,
    )?;

    assert_eq!(
        file_system_policy,
        FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(Some("docs".into())),
            },
            access: FileSystemAccessMode::Read,
        }]),
        "trailing /** should compile as a subtree path instead of a glob pattern"
    );
    Ok(())
}

#[test]
fn read_write_glob_patterns_still_reject_non_subpath_globs() {
    let err = compile_read_write_glob_path("src/**/*.rs", FileSystemAccessMode::Read)
        .expect_err("non-subpath read/write glob should be rejected");

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(
        err.to_string()
            .contains("filesystem glob path `src/**/*.rs` only supports `none` access"),
        "{err}"
    );
}
