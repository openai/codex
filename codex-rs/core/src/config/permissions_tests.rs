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
use codex_config::permissions_toml::WorkspaceRootsToml;
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

#[test]
fn windows_verbatim_path_prefix_does_not_count_as_glob_syntax() {
    assert!(!contains_glob_chars_for_platform(
        r"\\?\D:\c\x\worktrees\2508\swift-base",
        /*is_windows*/ true,
    ));
    assert!(contains_glob_chars_for_platform(
        r"\\?\D:\c\x\worktrees\2508\**\*.env",
        /*is_windows*/ true,
    ));
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
                        description: None,
                        extends: None,
                        workspace_roots: None,
                        filesystem: Some(FilesystemPermissionsToml {
                            glob_scan_max_depth: None,
                            allow_git: false,
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
            default_zsh_path: Some(AbsolutePathBuf::try_from(zsh_path.clone())?),
            main_execve_wrapper_exe: Some(execve_wrapper),
            ..Default::default()
        },
        AbsolutePathBuf::from_absolute_path(&codex_home)?,
    )
    .await?;

    let expected_zsh = AbsolutePathBuf::try_from(zsh_path)?;
    let expected_allowed_arg0_dir = AbsolutePathBuf::try_from(allowed_arg0_dir)?;
    let expected_sibling_arg0_dir = AbsolutePathBuf::try_from(sibling_arg0_dir)?;
    let policy = config.permissions.file_system_sandbox_policy();

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
                NetworkUnixSocketPermissionToml::Deny,
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
                    NetworkUnixSocketPermissionToml::Deny,
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
                    ProxyNetworkUnixSocketPermission::Deny,
                ),
            ]),
        })
    );
}

#[test]
fn permissions_profiles_resolve_extends_parent_first_with_child_overrides() {
    let permissions = toml::from_str::<PermissionsToml>(
        r#"
[base]
description = "Base profile"

[base.filesystem]
glob_scan_max_depth = 1
"/tmp/base" = "read"
"/tmp/shared" = "read"

[base.filesystem.":project_roots"]
"**/*.env" = "deny"
docs = "read"

[base.network]
enabled = true

[base.network.domains]
"base.example.com" = "allow"
"SHARED.EXAMPLE.COM." = "deny"

[base.network.unix_sockets]
"/tmp/base.sock" = "allow"
"/tmp/blocked.sock" = "deny"

[child]
extends = "base"

[child.filesystem]
glob_scan_max_depth = 3
"/tmp/shared" = "write"

[child.filesystem.":project_roots"]
docs = "write"
src = "read"

[child.network]
enabled = false
allow_local_binding = true

[child.network.domains]
"child.example.com" = "allow"
"shared.example.com" = "allow"

[child.network.unix_sockets]
"/tmp/child.sock" = "allow"
"#,
    )
    .expect("permissions should deserialize");

    let resolved = permissions
        .resolve_profile("child", |_| None)
        .expect("child profile should resolve");
    let expected_profile = toml::from_str::<PermissionProfileToml>(
        r#"
extends = "base"

[filesystem]
glob_scan_max_depth = 3
"/tmp/base" = "read"
"/tmp/shared" = "write"

[filesystem.":project_roots"]
"**/*.env" = "deny"
docs = "write"
src = "read"

[network]
enabled = false
allow_local_binding = true

[network.domains]
"base.example.com" = "allow"
"child.example.com" = "allow"
"shared.example.com" = "allow"

[network.unix_sockets]
"/tmp/base.sock" = "allow"
"/tmp/blocked.sock" = "deny"
"/tmp/child.sock" = "allow"
"#,
    )
    .expect("expected profile should deserialize");

    assert_eq!(resolved.profile, expected_profile);
    assert_eq!(resolved.inherited_profile_names, vec!["base".to_string()]);
}

#[test]
fn permissions_profiles_reject_undefined_extends_parent() {
    let permissions = toml::from_str::<PermissionsToml>(
        r#"
[child]
extends = "base"
"#,
    )
    .expect("permissions should deserialize");

    let err = permissions
        .resolve_profile("child", |_| None)
        .expect_err("missing parent should be rejected");

    assert_eq!(
        err.to_string(),
        "permissions profile `child` extends undefined profile `base`"
    );
}

#[test]
fn permissions_profiles_reject_unsupported_builtin_extends_parent() {
    let permissions = toml::from_str::<PermissionsToml>(
        r#"
[child]
extends = ":danger-full-access"
"#,
    )
    .expect("permissions should deserialize");

    let err = permissions
        .resolve_profile("child", |_| None)
        .expect_err("unsupported built-in parent should be rejected");

    assert_eq!(
        err.to_string(),
        "permissions profile `child` cannot extend unsupported built-in profile `:danger-full-access`"
    );
}

#[test]
fn permissions_profiles_reject_extends_cycles() {
    let permissions = toml::from_str::<PermissionsToml>(
        r#"
[alpha]
extends = "beta"

[beta]
extends = "alpha"
"#,
    )
    .expect("permissions should deserialize");

    let err = permissions
        .resolve_profile("alpha", |_| None)
        .expect_err("cycle should be rejected");

    assert_eq!(
        err.to_string(),
        "permissions profile inheritance cycle detected: alpha -> beta -> alpha"
    );
}

#[test]
fn profile_network_proxy_config_keeps_proxy_disabled_for_bare_network_access() {
    let config = network_proxy_config_from_profile_network(Some(&NetworkToml {
        enabled: Some(true),
        ..Default::default()
    }));

    assert!(!config.network.enabled);
}

#[test]
fn profile_network_proxy_config_keeps_proxy_disabled_for_proxy_policy() {
    let config = network_proxy_config_from_profile_network(Some(&NetworkToml {
        enabled: Some(true),
        proxy_url: Some("http://127.0.0.1:43128".to_string()),
        enable_socks5: Some(false),
        domains: Some(NetworkDomainPermissionsToml {
            entries: BTreeMap::from([(
                "openai.com".to_string(),
                NetworkDomainPermissionToml::Allow,
            )]),
        }),
        ..Default::default()
    }));

    assert!(!config.network.enabled);
    assert_eq!(config.network.proxy_url, "http://127.0.0.1:43128");
    assert!(!config.network.enable_socks5);
    assert_eq!(
        config.network.domains,
        Some(codex_network_proxy::NetworkDomainPermissions {
            entries: vec![codex_network_proxy::NetworkDomainPermissionEntry {
                pattern: "openai.com".to_string(),
                permission: codex_network_proxy::NetworkDomainPermission::Allow,
            }],
        })
    );
}

#[test]
fn compile_permission_profile_workspace_roots_resolves_enabled_entries() -> std::io::Result<()> {
    let cwd = TempDir::new()?;
    let workspace_roots = compile_permission_profile_workspace_roots(
        Some(&PermissionsToml {
            entries: BTreeMap::from([(
                "workspace".to_string(),
                PermissionProfileToml {
                    description: None,
                    extends: None,
                    workspace_roots: Some(WorkspaceRootsToml {
                        entries: BTreeMap::from([
                            ("backend".to_string(), true),
                            ("disabled".to_string(), false),
                        ]),
                    }),
                    filesystem: None,
                    network: None,
                },
            )]),
        }),
        "workspace",
        cwd.path(),
    )?;

    assert_eq!(
        workspace_roots,
        vec![AbsolutePathBuf::resolve_path_against_base(
            "backend",
            cwd.path()
        )]
    );
    Ok(())
}

#[test]
fn read_write_glob_warnings_skip_supported_deny_read_globs_and_trailing_subpaths() {
    let filesystem = FilesystemPermissionsToml {
        glob_scan_max_depth: None,
        allow_git: false,
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
                ":workspace_roots".to_string(),
                FilesystemPermissionToml::Scoped(BTreeMap::from([
                    ("**/*.env".to_string(), FileSystemAccessMode::Deny),
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
            ":workspace_roots/src/**/*.rs".to_string()
        ],
        "`deny` glob patterns are supported as deny-read rules; only `read`/`write` globs should warn"
    );
}

#[test]
fn unreadable_globstar_warning_is_suppressed_when_scan_depth_is_configured() {
    let filesystem = FilesystemPermissionsToml {
        glob_scan_max_depth: None,
        allow_git: false,
        entries: BTreeMap::from([(
            ":workspace_roots".to_string(),
            FilesystemPermissionToml::Scoped(BTreeMap::from([
                ("**/*.env".to_string(), FileSystemAccessMode::Deny),
                ("*.pem".to_string(), FileSystemAccessMode::Deny),
            ])),
        )]),
    };

    assert_eq!(
        unbounded_unreadable_globstar_paths(&filesystem),
        vec![":workspace_roots/**/*.env".to_string()]
    );

    let configured_filesystem = FilesystemPermissionsToml {
        glob_scan_max_depth: Some(2),
        ..filesystem
    };
    assert_eq!(
        unbounded_unreadable_globstar_paths(&configured_filesystem),
        Vec::<String>::new()
    );
}

#[test]
fn glob_scan_max_depth_must_be_positive() {
    let err = validate_glob_scan_max_depth(Some(0))
        .expect_err("zero depth would silently skip deny-read glob expansion");

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(err.to_string(), "glob_scan_max_depth must be at least 1");
    assert_eq!(
        validate_glob_scan_max_depth(Some(2)).expect("depth should be valid"),
        Some(2)
    );
}

#[test]
fn workspace_parent_uses_profile_limited_git_write_setting() -> std::io::Result<()> {
    let cwd = TempDir::new()?;
    std::fs::create_dir_all(cwd.path().join(".git").join("hooks"))?;
    let mut startup_warnings = Vec::new();
    let (file_system_policy, _) = compile_permission_profile(
        &PermissionsToml {
            entries: BTreeMap::from([(
                "workspace".to_string(),
                PermissionProfileToml {
                    description: None,
                    extends: Some(BUILT_IN_WORKSPACE_PROFILE.to_string()),
                    workspace_roots: None,
                    filesystem: Some(FilesystemPermissionsToml {
                        glob_scan_max_depth: None,
                        allow_git: true,
                        entries: BTreeMap::new(),
                    }),
                    network: None,
                },
            )]),
        },
        "workspace",
        cwd.path(),
        &mut startup_warnings,
    )?;

    assert!(
        file_system_policy
            .can_write_path_with_cwd(&cwd.path().join(".git").join("HEAD"), cwd.path())
    );
    assert!(
        !file_system_policy
            .can_write_path_with_cwd(&cwd.path().join(".git").join("config"), cwd.path())
    );
    assert!(!file_system_policy.can_write_path_with_cwd(
        &cwd.path().join(".git").join("hooks").join("pre-commit"),
        cwd.path()
    ));
    Ok(())
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
                    description: None,
                    extends: None,
                    workspace_roots: None,
                    filesystem: Some(FilesystemPermissionsToml {
                        glob_scan_max_depth: None,
                        allow_git: false,
                        entries: BTreeMap::from([(
                            ":workspace_roots".to_string(),
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
            .contains("filesystem glob path `src/**/*.rs` only supports `deny` access"),
        "{err}"
    );
}
