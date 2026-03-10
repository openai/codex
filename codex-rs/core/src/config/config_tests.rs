use crate::config::edit::ConfigEdit;
use crate::config::edit::ConfigEditsBuilder;
use crate::config::edit::apply_blocking;
use crate::config::types::BundledSkillsConfig;
use crate::config::types::ApprovalReviewPolicy;
use crate::config::types::BundledSkillsConfig;
use crate::config::types::FeedbackConfigToml;
use crate::config::types::HistoryPersistence;
use crate::config::types::McpServerTransportConfig;
use crate::config::types::MemoriesConfig;
use crate::config::types::MemoriesToml;
use crate::config::types::ModelAvailabilityNuxConfig;
use crate::config::types::NotificationMethod;
use crate::config::types::Notifications;
use crate::config_loader::RequirementSource;
use crate::features::Feature;
use assert_matches::assert_matches;
use codex_config::CONFIG_TOML_FILE;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use serde::Deserialize;
use tempfile::tempdir;

use super::*;
use core_test_support::test_absolute_path;
use pretty_assertions::assert_eq;

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::time::Duration;
use tempfile::TempDir;

fn stdio_mcp(command: &str) -> McpServerConfig {
    McpServerConfig {
        transport: McpServerTransportConfig::Stdio {
            command: command.to_string(),
            args: Vec::new(),
            env: None,
            env_vars: Vec::new(),
            cwd: None,
        },
        enabled: true,
        required: false,
        disabled_reason: None,
        startup_timeout_sec: None,
        tool_timeout_sec: None,
        enabled_tools: None,
        disabled_tools: None,
        scopes: None,
        oauth_resource: None,
    }
}

fn http_mcp(url: &str) -> McpServerConfig {
    McpServerConfig {
        transport: McpServerTransportConfig::StreamableHttp {
            url: url.to_string(),
            bearer_token_env_var: None,
            http_headers: None,
            env_http_headers: None,
        },
        enabled: true,
        required: false,
        disabled_reason: None,
        startup_timeout_sec: None,
        tool_timeout_sec: None,
        enabled_tools: None,
        disabled_tools: None,
        scopes: None,
        oauth_resource: None,
    }
}

#[test]
fn test_toml_parsing() {
    let history_with_persistence = r#"
[history]
persistence = "save-all"
"#;
    let history_with_persistence_cfg = toml::from_str::<ConfigToml>(history_with_persistence)
        .expect("TOML deserialization should succeed");
    assert_eq!(
        Some(History {
            persistence: HistoryPersistence::SaveAll,
            max_bytes: None,
        }),
        history_with_persistence_cfg.history
    );

    let history_no_persistence = r#"
[history]
persistence = "none"
"#;

    let history_no_persistence_cfg = toml::from_str::<ConfigToml>(history_no_persistence)
        .expect("TOML deserialization should succeed");
    assert_eq!(
        Some(History {
            persistence: HistoryPersistence::None,
            max_bytes: None,
        }),
        history_no_persistence_cfg.history
    );

    let memories = r#"
[memories]
no_memories_if_mcp_or_web_search = true
generate_memories = false
use_memories = false
max_raw_memories_for_consolidation = 512
max_unused_days = 21
max_rollout_age_days = 42
max_rollouts_per_startup = 9
min_rollout_idle_hours = 24
extract_model = "gpt-5-mini"
consolidation_model = "gpt-5"
"#;
    let memories_cfg =
        toml::from_str::<ConfigToml>(memories).expect("TOML deserialization should succeed");
    assert_eq!(
        Some(MemoriesToml {
            no_memories_if_mcp_or_web_search: Some(true),
            generate_memories: Some(false),
            use_memories: Some(false),
            max_raw_memories_for_consolidation: Some(512),
            max_unused_days: Some(21),
            max_rollout_age_days: Some(42),
            max_rollouts_per_startup: Some(9),
            min_rollout_idle_hours: Some(24),
            extract_model: Some("gpt-5-mini".to_string()),
            consolidation_model: Some("gpt-5".to_string()),
        }),
        memories_cfg.memories
    );

    let config = Config::load_from_base_config_with_overrides(
        memories_cfg,
        ConfigOverrides::default(),
        tempdir().expect("tempdir").path().to_path_buf(),
    )
    .expect("load config from memories settings");
    assert_eq!(
        config.memories,
        MemoriesConfig {
            no_memories_if_mcp_or_web_search: true,
            generate_memories: false,
            use_memories: false,
            max_raw_memories_for_consolidation: 512,
            max_unused_days: 21,
            max_rollout_age_days: 42,
            max_rollouts_per_startup: 9,
            min_rollout_idle_hours: 24,
            extract_model: Some("gpt-5-mini".to_string()),
            consolidation_model: Some("gpt-5".to_string()),
        }
    );
}

#[test]
fn parses_bundled_skills_config() {
    let cfg: ConfigToml = toml::from_str(
        r#"
[skills.bundled]
enabled = false
"#,
    )
    .expect("TOML deserialization should succeed");

    assert_eq!(
        cfg.skills,
        Some(SkillsConfig {
            bundled: Some(BundledSkillsConfig { enabled: false }),
            config: Vec::new(),
        })
    );
}

#[test]
fn tools_web_search_true_deserializes_to_none() {
    let cfg: ConfigToml = toml::from_str(
        r#"
[tools]
web_search = true
"#,
    )
    .expect("TOML deserialization should succeed");

    assert_eq!(
        cfg.tools,
        Some(ToolsToml {
            web_search: None,
            view_image: None,
        })
    );
}

#[test]
fn tools_web_search_false_deserializes_to_none() {
    let cfg: ConfigToml = toml::from_str(
        r#"
[tools]
web_search = false
"#,
    )
    .expect("TOML deserialization should succeed");

    assert_eq!(
        cfg.tools,
        Some(ToolsToml {
            web_search: None,
            view_image: None,
        })
    );
}

#[test]
fn config_toml_deserializes_model_availability_nux() {
    let toml = r#"
[tui.model_availability_nux]
"gpt-foo" = 2
"gpt-bar" = 4
"#;
    let cfg: ConfigToml =
        toml::from_str(toml).expect("TOML deserialization should succeed for TUI NUX");

    assert_eq!(
        cfg.tui.expect("tui config should deserialize"),
        Tui {
            notifications: Notifications::default(),
            notification_method: NotificationMethod::default(),
            animations: true,
            show_tooltips: true,
            alternate_screen: AltScreenMode::default(),
            status_line: None,
            theme: None,
            model_availability_nux: ModelAvailabilityNuxConfig {
                shown_count: HashMap::from([
                    ("gpt-bar".to_string(), 4),
                    ("gpt-foo".to_string(), 2),
                ]),
            },
        }
    );
}

#[test]
fn runtime_config_defaults_model_availability_nux() {
    let cfg = Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        tempdir().expect("tempdir").path().to_path_buf(),
    )
    .expect("load config");

    assert_eq!(
        cfg.model_availability_nux,
        ModelAvailabilityNuxConfig::default()
    );
}

#[test]
fn config_toml_deserializes_permission_profiles() {
    let toml = r#"
default_permissions = "workspace"

[permissions.workspace.filesystem]
":minimal" = "read"

[permissions.workspace.filesystem.":project_roots"]
"." = "write"
"docs" = "read"

[permissions.workspace.network]
enabled = true
proxy_url = "http://127.0.0.1:43128"
enable_socks5 = false
allow_upstream_proxy = false
allowed_domains = ["openai.com"]
"#;
    let cfg: ConfigToml =
        toml::from_str(toml).expect("TOML deserialization should succeed for permissions profiles");

    assert_eq!(cfg.default_permissions.as_deref(), Some("workspace"));
    assert_eq!(
        cfg.permissions.expect("[permissions] should deserialize"),
        PermissionsToml {
            entries: BTreeMap::from([(
                "workspace".to_string(),
                PermissionProfileToml {
                    filesystem: Some(FilesystemPermissionsToml {
                        entries: BTreeMap::from([
                            (
                                ":minimal".to_string(),
                                FilesystemPermissionToml::Access(FileSystemAccessMode::Read),
                            ),
                            (
                                ":project_roots".to_string(),
                                FilesystemPermissionToml::Scoped(BTreeMap::from([
                                    (".".to_string(), FileSystemAccessMode::Write),
                                    ("docs".to_string(), FileSystemAccessMode::Read),
                                ])),
                            ),
                        ]),
                    }),
                    network: Some(NetworkToml {
                        enabled: Some(true),
                        proxy_url: Some("http://127.0.0.1:43128".to_string()),
                        enable_socks5: Some(false),
                        socks_url: None,
                        enable_socks5_udp: None,
                        allow_upstream_proxy: Some(false),
                        dangerously_allow_non_loopback_proxy: None,
                        dangerously_allow_all_unix_sockets: None,
                        mode: None,
                        allowed_domains: Some(vec!["openai.com".to_string()]),
                        denied_domains: None,
                        allow_unix_sockets: None,
                        allow_local_binding: None,
                    }),
                },
            )]),
        }
    );
}

#[test]
fn permissions_profiles_network_populates_runtime_network_proxy_spec() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let cwd = TempDir::new()?;
    std::fs::write(cwd.path().join(".git"), "gitdir: nowhere")?;

    let config = Config::load_from_base_config_with_overrides(
        ConfigToml {
            default_permissions: Some("workspace".to_string()),
            permissions: Some(PermissionsToml {
                entries: BTreeMap::from([(
                    "workspace".to_string(),
                    PermissionProfileToml {
                        filesystem: Some(FilesystemPermissionsToml {
                            entries: BTreeMap::from([(
                                ":minimal".to_string(),
                                FilesystemPermissionToml::Access(FileSystemAccessMode::Read),
                            )]),
                        }),
                        network: Some(NetworkToml {
                            enabled: Some(true),
                            proxy_url: Some("http://127.0.0.1:43128".to_string()),
                            enable_socks5: Some(false),
                            ..Default::default()
                        }),
                    },
                )]),
            }),
            ..Default::default()
        },
        ConfigOverrides {
            cwd: Some(cwd.path().to_path_buf()),
            ..Default::default()
        },
        codex_home.path().to_path_buf(),
    )?;
    let network = config
        .permissions
        .network
        .as_ref()
        .expect("enabled profile network should produce a NetworkProxySpec");

    assert_eq!(network.proxy_host_and_port(), "127.0.0.1:43128");
    assert!(!network.socks_enabled());
    Ok(())
}

#[test]
fn permissions_profiles_network_disabled_by_default_does_not_start_proxy() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let cwd = TempDir::new()?;
    std::fs::write(cwd.path().join(".git"), "gitdir: nowhere")?;

    let config = Config::load_from_base_config_with_overrides(
        ConfigToml {
            default_permissions: Some("workspace".to_string()),
            permissions: Some(PermissionsToml {
                entries: BTreeMap::from([(
                    "workspace".to_string(),
                    PermissionProfileToml {
                        filesystem: Some(FilesystemPermissionsToml {
                            entries: BTreeMap::from([(
                                ":minimal".to_string(),
                                FilesystemPermissionToml::Access(FileSystemAccessMode::Read),
                            )]),
                        }),
                        network: Some(NetworkToml {
                            allowed_domains: Some(vec!["openai.com".to_string()]),
                            ..Default::default()
                        }),
                    },
                )]),
            }),
            ..Default::default()
        },
        ConfigOverrides {
            cwd: Some(cwd.path().to_path_buf()),
            ..Default::default()
        },
        codex_home.path().to_path_buf(),
    )?;

    assert!(config.permissions.network.is_none());
    Ok(())
}

#[test]
fn default_permissions_profile_populates_runtime_sandbox_policy() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let cwd = TempDir::new()?;
    std::fs::create_dir_all(cwd.path().join("docs"))?;
    std::fs::write(cwd.path().join(".git"), "gitdir: nowhere")?;

    let cfg = ConfigToml {
        default_permissions: Some("workspace".to_string()),
        permissions: Some(PermissionsToml {
            entries: BTreeMap::from([(
                "workspace".to_string(),
                PermissionProfileToml {
                    filesystem: Some(FilesystemPermissionsToml {
                        entries: BTreeMap::from([
                            (
                                ":minimal".to_string(),
                                FilesystemPermissionToml::Access(FileSystemAccessMode::Read),
                            ),
                            (
                                ":project_roots".to_string(),
                                FilesystemPermissionToml::Scoped(BTreeMap::from([
                                    (".".to_string(), FileSystemAccessMode::Write),
                                    ("docs".to_string(), FileSystemAccessMode::Read),
                                ])),
                            ),
                        ]),
                    }),
                    network: None,
                },
            )]),
        }),
        ..Default::default()
    };

    let config = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides {
            cwd: Some(cwd.path().to_path_buf()),
            ..Default::default()
        },
        codex_home.path().to_path_buf(),
    )?;

    let memories_root = AbsolutePathBuf::try_from(codex_home.path().join("memories")).unwrap();
    assert_eq!(
        config.permissions.file_system_sandbox_policy,
        FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Minimal,
                },
                access: FileSystemAccessMode::Read,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(None),
                },
                access: FileSystemAccessMode::Write,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(Some("docs".into())),
                },
                access: FileSystemAccessMode::Read,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: memories_root.clone(),
                },
                access: FileSystemAccessMode::Write,
            },
        ]),
    );
    assert_eq!(
        config.permissions.sandbox_policy.get(),
        &SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![memories_root],
            read_only_access: ReadOnlyAccess::Restricted {
                include_platform_defaults: true,
                readable_roots: vec![
                    AbsolutePathBuf::try_from(cwd.path().join("docs")).expect("absolute docs path"),
                ],
            },
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        }
    );
    assert_eq!(
        config.permissions.network_sandbox_policy,
        NetworkSandboxPolicy::Restricted
    );
    Ok(())
}

#[test]
fn permissions_profiles_require_default_permissions() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let cwd = TempDir::new()?;
    std::fs::write(cwd.path().join(".git"), "gitdir: nowhere")?;

    let err = Config::load_from_base_config_with_overrides(
        ConfigToml {
            permissions: Some(PermissionsToml {
                entries: BTreeMap::from([(
                    "workspace".to_string(),
                    PermissionProfileToml {
                        filesystem: Some(FilesystemPermissionsToml {
                            entries: BTreeMap::from([(
                                ":minimal".to_string(),
                                FilesystemPermissionToml::Access(FileSystemAccessMode::Read),
                            )]),
                        }),
                        network: None,
                    },
                )]),
            }),
            ..Default::default()
        },
        ConfigOverrides {
            cwd: Some(cwd.path().to_path_buf()),
            ..Default::default()
        },
        codex_home.path().to_path_buf(),
    )
    .expect_err("missing default_permissions should be rejected");

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(
        err.to_string(),
        "config defines `[permissions]` profiles but does not set `default_permissions`"
    );
    Ok(())
}

#[test]
fn permissions_profiles_reject_writes_outside_workspace_root() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let cwd = TempDir::new()?;
    std::fs::write(cwd.path().join(".git"), "gitdir: nowhere")?;
    let external_write_path = if cfg!(windows) { r"C:\temp" } else { "/tmp" };

    let err = Config::load_from_base_config_with_overrides(
        ConfigToml {
            default_permissions: Some("workspace".to_string()),
            permissions: Some(PermissionsToml {
                entries: BTreeMap::from([(
                    "workspace".to_string(),
                    PermissionProfileToml {
                        filesystem: Some(FilesystemPermissionsToml {
                            entries: BTreeMap::from([(
                                external_write_path.to_string(),
                                FilesystemPermissionToml::Access(FileSystemAccessMode::Write),
                            )]),
                        }),
                        network: None,
                    },
                )]),
            }),
            ..Default::default()
        },
        ConfigOverrides {
            cwd: Some(cwd.path().to_path_buf()),
            ..Default::default()
        },
        codex_home.path().to_path_buf(),
    )
    .expect_err("writes outside the workspace root should be rejected");

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(
        err.to_string()
            .contains("filesystem writes outside the workspace root"),
        "{err}"
    );
    Ok(())
}

#[test]
fn permissions_profiles_reject_nested_entries_for_non_project_roots() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let cwd = TempDir::new()?;
    std::fs::write(cwd.path().join(".git"), "gitdir: nowhere")?;

    let err = Config::load_from_base_config_with_overrides(
        ConfigToml {
            default_permissions: Some("workspace".to_string()),
            permissions: Some(PermissionsToml {
                entries: BTreeMap::from([(
                    "workspace".to_string(),
                    PermissionProfileToml {
                        filesystem: Some(FilesystemPermissionsToml {
                            entries: BTreeMap::from([(
                                ":minimal".to_string(),
                                FilesystemPermissionToml::Scoped(BTreeMap::from([(
                                    "docs".to_string(),
                                    FileSystemAccessMode::Read,
                                )])),
                            )]),
                        }),
                        network: None,
                    },
                )]),
            }),
            ..Default::default()
        },
        ConfigOverrides {
            cwd: Some(cwd.path().to_path_buf()),
            ..Default::default()
        },
        codex_home.path().to_path_buf(),
    )
    .expect_err("nested entries outside :project_roots should be rejected");

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(
        err.to_string(),
        "filesystem path `:minimal` does not support nested entries"
    );
    Ok(())
}

fn load_workspace_permission_profile(profile: PermissionProfileToml) -> std::io::Result<Config> {
    let codex_home = TempDir::new()?;
    let cwd = TempDir::new()?;
    std::fs::write(cwd.path().join(".git"), "gitdir: nowhere")?;

    Config::load_from_base_config_with_overrides(
        ConfigToml {
            default_permissions: Some("workspace".to_string()),
            permissions: Some(PermissionsToml {
                entries: BTreeMap::from([("workspace".to_string(), profile)]),
            }),
            ..Default::default()
        },
        ConfigOverrides {
            cwd: Some(cwd.path().to_path_buf()),
            ..Default::default()
        },
        codex_home.path().to_path_buf(),
    )
}

#[test]
fn permissions_profiles_allow_unknown_special_paths() -> std::io::Result<()> {
    let config = load_workspace_permission_profile(PermissionProfileToml {
        filesystem: Some(FilesystemPermissionsToml {
            entries: BTreeMap::from([(
                ":future_special_path".to_string(),
                FilesystemPermissionToml::Access(FileSystemAccessMode::Read),
            )]),
        }),
        network: None,
    })?;

    assert_eq!(
        config.permissions.file_system_sandbox_policy,
        FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::unknown(":future_special_path", None),
            },
            access: FileSystemAccessMode::Read,
        }]),
    );
    assert_eq!(
        config.permissions.sandbox_policy.get(),
        &SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::Restricted {
                include_platform_defaults: false,
                readable_roots: Vec::new(),
            },
            network_access: false,
        }
    );
    assert!(
        config.startup_warnings.iter().any(|warning| warning.contains(
            "Configured filesystem path `:future_special_path` is not recognized by this version of Codex and will be ignored."
        )),
        "{:?}",
        config.startup_warnings
    );
    Ok(())
}

#[test]
fn permissions_profiles_allow_unknown_special_paths_with_nested_entries() -> std::io::Result<()> {
    let config = load_workspace_permission_profile(PermissionProfileToml {
        filesystem: Some(FilesystemPermissionsToml {
            entries: BTreeMap::from([(
                ":future_special_path".to_string(),
                FilesystemPermissionToml::Scoped(BTreeMap::from([(
                    "docs".to_string(),
                    FileSystemAccessMode::Read,
                )])),
            )]),
        }),
        network: None,
    })?;

    assert_eq!(
        config.permissions.file_system_sandbox_policy,
        FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::unknown(":future_special_path", Some("docs".into())),
            },
            access: FileSystemAccessMode::Read,
        }]),
    );
    assert!(
        config.startup_warnings.iter().any(|warning| warning.contains(
            "Configured filesystem path `:future_special_path` with nested entry `docs` is not recognized by this version of Codex and will be ignored."
        )),
        "{:?}",
        config.startup_warnings
    );
    Ok(())
}

#[test]
fn permissions_profiles_allow_missing_filesystem_with_warning() -> std::io::Result<()> {
    let config = load_workspace_permission_profile(PermissionProfileToml {
        filesystem: None,
        network: None,
    })?;

    assert_eq!(
        config.permissions.file_system_sandbox_policy,
        FileSystemSandboxPolicy::restricted(Vec::new())
    );
    assert_eq!(
        config.permissions.sandbox_policy.get(),
        &SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::Restricted {
                include_platform_defaults: false,
                readable_roots: Vec::new(),
            },
            network_access: false,
        }
    );
    assert!(
        config.startup_warnings.iter().any(|warning| warning.contains(
            "Permissions profile `workspace` does not define any recognized filesystem entries for this version of Codex."
        )),
        "{:?}",
        config.startup_warnings
    );
    Ok(())
}

#[test]
fn permissions_profiles_allow_empty_filesystem_with_warning() -> std::io::Result<()> {
    let config = load_workspace_permission_profile(PermissionProfileToml {
        filesystem: Some(FilesystemPermissionsToml {
            entries: BTreeMap::new(),
        }),
        network: None,
    })?;

    assert_eq!(
        config.permissions.file_system_sandbox_policy,
        FileSystemSandboxPolicy::restricted(Vec::new())
    );
    assert!(
        config.startup_warnings.iter().any(|warning| warning.contains(
            "Permissions profile `workspace` does not define any recognized filesystem entries for this version of Codex."
        )),
        "{:?}",
        config.startup_warnings
    );
    Ok(())
}

#[test]
fn permissions_profiles_reject_project_root_parent_traversal() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let cwd = TempDir::new()?;
    std::fs::write(cwd.path().join(".git"), "gitdir: nowhere")?;

    let err = Config::load_from_base_config_with_overrides(
        ConfigToml {
            default_permissions: Some("workspace".to_string()),
            permissions: Some(PermissionsToml {
                entries: BTreeMap::from([(
                    "workspace".to_string(),
                    PermissionProfileToml {
                        filesystem: Some(FilesystemPermissionsToml {
                            entries: BTreeMap::from([(
                                ":project_roots".to_string(),
                                FilesystemPermissionToml::Scoped(BTreeMap::from([(
                                    "../sibling".to_string(),
                                    FileSystemAccessMode::Read,
                                )])),
                            )]),
                        }),
                        network: None,
                    },
                )]),
            }),
            ..Default::default()
        },
        ConfigOverrides {
            cwd: Some(cwd.path().to_path_buf()),
            ..Default::default()
        },
        codex_home.path().to_path_buf(),
    )
    .expect_err("parent traversal should be rejected for project root subpaths");

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(
        err.to_string(),
        "filesystem subpath `../sibling` must be a descendant path without `.` or `..` components"
    );
    Ok(())
}

#[test]
fn permissions_profiles_allow_network_enablement() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let cwd = TempDir::new()?;
    std::fs::write(cwd.path().join(".git"), "gitdir: nowhere")?;

    let config = Config::load_from_base_config_with_overrides(
        ConfigToml {
            default_permissions: Some("workspace".to_string()),
            permissions: Some(PermissionsToml {
                entries: BTreeMap::from([(
                    "workspace".to_string(),
                    PermissionProfileToml {
                        filesystem: Some(FilesystemPermissionsToml {
                            entries: BTreeMap::from([(
                                ":minimal".to_string(),
                                FilesystemPermissionToml::Access(FileSystemAccessMode::Read),
                            )]),
                        }),
                        network: Some(NetworkToml {
                            enabled: Some(true),
                            ..Default::default()
                        }),
                    },
                )]),
            }),
            ..Default::default()
        },
        ConfigOverrides {
            cwd: Some(cwd.path().to_path_buf()),
            ..Default::default()
        },
        codex_home.path().to_path_buf(),
    )?;

    assert!(
        config.permissions.network_sandbox_policy.is_enabled(),
        "expected network sandbox policy to be enabled",
    );
    assert!(
        config
            .permissions
            .sandbox_policy
            .get()
            .has_full_network_access()
    );
    Ok(())
}

#[test]
fn tui_theme_deserializes_from_toml() {
    let cfg = r#"
[tui]
theme = "dracula"
"#;
    let parsed = toml::from_str::<ConfigToml>(cfg).expect("TOML deserialization should succeed");
    assert_eq!(
        parsed.tui.as_ref().and_then(|t| t.theme.as_deref()),
        Some("dracula"),
    );
}

#[test]
fn tui_theme_defaults_to_none() {
    let cfg = r#"
[tui]
"#;
    let parsed = toml::from_str::<ConfigToml>(cfg).expect("TOML deserialization should succeed");
    assert_eq!(parsed.tui.as_ref().and_then(|t| t.theme.as_deref()), None);
}

#[test]
fn tui_config_missing_notifications_field_defaults_to_enabled() {
    let cfg = r#"
[tui]
"#;

    let parsed =
        toml::from_str::<ConfigToml>(cfg).expect("TUI config without notifications should succeed");
    let tui = parsed.tui.expect("config should include tui section");

    assert_eq!(
        tui,
        Tui {
            notifications: Notifications::Enabled(true),
            notification_method: NotificationMethod::Auto,
            animations: true,
            show_tooltips: true,
            alternate_screen: AltScreenMode::Auto,
            status_line: None,
            theme: None,
            model_availability_nux: ModelAvailabilityNuxConfig::default(),
        }
    );
}

#[test]
fn test_sandbox_config_parsing() {
    let sandbox_full_access = r#"
sandbox_mode = "danger-full-access"

[sandbox_workspace_write]
network_access = false  # This should be ignored.
"#;
    let sandbox_full_access_cfg = toml::from_str::<ConfigToml>(sandbox_full_access)
        .expect("TOML deserialization should succeed");
    let sandbox_mode_override = None;
    let resolution = sandbox_full_access_cfg.derive_sandbox_policy(
        sandbox_mode_override,
        None,
        WindowsSandboxLevel::Disabled,
        &PathBuf::from("/tmp/test"),
        None,
    );
    assert_eq!(resolution, SandboxPolicy::DangerFullAccess);

    let sandbox_read_only = r#"
sandbox_mode = "read-only"

[sandbox_workspace_write]
network_access = true  # This should be ignored.
"#;

    let sandbox_read_only_cfg = toml::from_str::<ConfigToml>(sandbox_read_only)
        .expect("TOML deserialization should succeed");
    let sandbox_mode_override = None;
    let resolution = sandbox_read_only_cfg.derive_sandbox_policy(
        sandbox_mode_override,
        None,
        WindowsSandboxLevel::Disabled,
        &PathBuf::from("/tmp/test"),
        None,
    );
    assert_eq!(resolution, SandboxPolicy::new_read_only_policy());

    let writable_root = test_absolute_path("/my/workspace");
    let sandbox_workspace_write = format!(
        r#"
sandbox_mode = "workspace-write"

[sandbox_workspace_write]
writable_roots = [
    {},
]
exclude_tmpdir_env_var = true
exclude_slash_tmp = true
"#,
        serde_json::json!(writable_root)
    );

    let sandbox_workspace_write_cfg = toml::from_str::<ConfigToml>(&sandbox_workspace_write)
        .expect("TOML deserialization should succeed");
    let sandbox_mode_override = None;
    let resolution = sandbox_workspace_write_cfg.derive_sandbox_policy(
        sandbox_mode_override,
        None,
        WindowsSandboxLevel::Disabled,
        &PathBuf::from("/tmp/test"),
        None,
    );
    if cfg!(target_os = "windows") {
        assert_eq!(resolution, SandboxPolicy::new_read_only_policy());
    } else {
        assert_eq!(
            resolution,
            SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![writable_root.clone()],
                read_only_access: ReadOnlyAccess::FullAccess,
                network_access: false,
                exclude_tmpdir_env_var: true,
                exclude_slash_tmp: true,
            }
        );
    }

    let sandbox_workspace_write = format!(
        r#"
sandbox_mode = "workspace-write"

[sandbox_workspace_write]
writable_roots = [
    {},
]
exclude_tmpdir_env_var = true
exclude_slash_tmp = true

[projects."/tmp/test"]
trust_level = "trusted"
"#,
        serde_json::json!(writable_root)
    );

    let sandbox_workspace_write_cfg = toml::from_str::<ConfigToml>(&sandbox_workspace_write)
        .expect("TOML deserialization should succeed");
    let sandbox_mode_override = None;
    let resolution = sandbox_workspace_write_cfg.derive_sandbox_policy(
        sandbox_mode_override,
        None,
        WindowsSandboxLevel::Disabled,
        &PathBuf::from("/tmp/test"),
        None,
    );
    if cfg!(target_os = "windows") {
        assert_eq!(resolution, SandboxPolicy::new_read_only_policy());
    } else {
        assert_eq!(
            resolution,
            SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![writable_root],
                read_only_access: ReadOnlyAccess::FullAccess,
                network_access: false,
                exclude_tmpdir_env_var: true,
                exclude_slash_tmp: true,
            }
        );
    }
}

#[test]
fn legacy_sandbox_mode_config_builds_split_policies_without_drift() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let cwd = TempDir::new()?;
    let extra_root = test_absolute_path("/tmp/legacy-extra-root");
    let cases = vec![
        (
            "danger-full-access".to_string(),
            r#"sandbox_mode = "danger-full-access"
"#
            .to_string(),
        ),
        (
            "read-only".to_string(),
            r#"sandbox_mode = "read-only"
"#
            .to_string(),
        ),
        (
            "workspace-write".to_string(),
            format!(
                r#"sandbox_mode = "workspace-write"

[sandbox_workspace_write]
writable_roots = [{}]
exclude_tmpdir_env_var = true
exclude_slash_tmp = true
"#,
                serde_json::json!(extra_root)
            ),
        ),
    ];

    for (name, config_toml) in cases {
        let cfg = toml::from_str::<ConfigToml>(&config_toml)
            .unwrap_or_else(|err| panic!("case `{name}` should parse: {err}"));
        let config = Config::load_from_base_config_with_overrides(
            cfg,
            ConfigOverrides {
                cwd: Some(cwd.path().to_path_buf()),
                ..Default::default()
            },
            codex_home.path().to_path_buf(),
        )?;

        let sandbox_policy = config.permissions.sandbox_policy.get();
        assert_eq!(
            config.permissions.file_system_sandbox_policy,
            FileSystemSandboxPolicy::from_legacy_sandbox_policy(sandbox_policy, cwd.path()),
            "case `{name}` should preserve filesystem semantics from legacy config"
        );
        assert_eq!(
            config.permissions.network_sandbox_policy,
            NetworkSandboxPolicy::from(sandbox_policy),
            "case `{name}` should preserve network semantics from legacy config"
        );
        assert_eq!(
            config
                .permissions
                .file_system_sandbox_policy
                .to_legacy_sandbox_policy(config.permissions.network_sandbox_policy, cwd.path())
                .unwrap_or_else(|err| panic!("case `{name}` should round-trip: {err}")),
            sandbox_policy.clone(),
            "case `{name}` should round-trip through split policies without drift"
        );
    }

    Ok(())
}

#[test]
fn filter_mcp_servers_by_allowlist_enforces_identity_rules() {
    const MISMATCHED_COMMAND_SERVER: &str = "mismatched-command-should-disable";
    const MISMATCHED_URL_SERVER: &str = "mismatched-url-should-disable";
    const MATCHED_COMMAND_SERVER: &str = "matched-command-should-allow";
    const MATCHED_URL_SERVER: &str = "matched-url-should-allow";
    const DIFFERENT_NAME_SERVER: &str = "different-name-should-disable";

    const GOOD_CMD: &str = "good-cmd";
    const GOOD_URL: &str = "https://example.com/good";

    let mut servers = HashMap::from([
        (MISMATCHED_COMMAND_SERVER.to_string(), stdio_mcp("docs-cmd")),
        (
            MISMATCHED_URL_SERVER.to_string(),
            http_mcp("https://example.com/mcp"),
        ),
        (MATCHED_COMMAND_SERVER.to_string(), stdio_mcp(GOOD_CMD)),
        (MATCHED_URL_SERVER.to_string(), http_mcp(GOOD_URL)),
        (DIFFERENT_NAME_SERVER.to_string(), stdio_mcp("same-cmd")),
    ]);
    let source = RequirementSource::LegacyManagedConfigTomlFromMdm;
    let requirements = Sourced::new(
        BTreeMap::from([
            (
                MISMATCHED_URL_SERVER.to_string(),
                McpServerRequirement {
                    identity: McpServerIdentity::Url {
                        url: "https://example.com/other".to_string(),
                    },
                },
            ),
            (
                MISMATCHED_COMMAND_SERVER.to_string(),
                McpServerRequirement {
                    identity: McpServerIdentity::Command {
                        command: "other-cmd".to_string(),
                    },
                },
            ),
            (
                MATCHED_URL_SERVER.to_string(),
                McpServerRequirement {
                    identity: McpServerIdentity::Url {
                        url: GOOD_URL.to_string(),
                    },
                },
            ),
            (
                MATCHED_COMMAND_SERVER.to_string(),
                McpServerRequirement {
                    identity: McpServerIdentity::Command {
                        command: GOOD_CMD.to_string(),
                    },
                },
            ),
        ]),
        source.clone(),
    );
    filter_mcp_servers_by_requirements(&mut servers, Some(&requirements));

    let reason = Some(McpServerDisabledReason::Requirements { source });
    assert_eq!(
        servers
            .iter()
            .map(|(name, server)| (
                name.clone(),
                (server.enabled, server.disabled_reason.clone())
            ))
            .collect::<HashMap<String, (bool, Option<McpServerDisabledReason>)>>(),
        HashMap::from([
            (MISMATCHED_URL_SERVER.to_string(), (false, reason.clone())),
            (
                MISMATCHED_COMMAND_SERVER.to_string(),
                (false, reason.clone()),
            ),
            (MATCHED_URL_SERVER.to_string(), (true, None)),
            (MATCHED_COMMAND_SERVER.to_string(), (true, None)),
            (DIFFERENT_NAME_SERVER.to_string(), (false, reason)),
        ])
    );
}

#[test]
fn filter_mcp_servers_by_allowlist_allows_all_when_unset() {
    let mut servers = HashMap::from([
        ("server-a".to_string(), stdio_mcp("cmd-a")),
        ("server-b".to_string(), http_mcp("https://example.com/b")),
    ]);

    filter_mcp_servers_by_requirements(&mut servers, None);

    assert_eq!(
        servers
            .iter()
            .map(|(name, server)| (
                name.clone(),
                (server.enabled, server.disabled_reason.clone())
            ))
            .collect::<HashMap<String, (bool, Option<McpServerDisabledReason>)>>(),
        HashMap::from([
            ("server-a".to_string(), (true, None)),
            ("server-b".to_string(), (true, None)),
        ])
    );
}

#[test]
fn filter_mcp_servers_by_allowlist_blocks_all_when_empty() {
    let mut servers = HashMap::from([
        ("server-a".to_string(), stdio_mcp("cmd-a")),
        ("server-b".to_string(), http_mcp("https://example.com/b")),
    ]);

    let source = RequirementSource::LegacyManagedConfigTomlFromMdm;
    let requirements = Sourced::new(BTreeMap::new(), source.clone());
    filter_mcp_servers_by_requirements(&mut servers, Some(&requirements));

    let reason = Some(McpServerDisabledReason::Requirements { source });
    assert_eq!(
        servers
            .iter()
            .map(|(name, server)| (
                name.clone(),
                (server.enabled, server.disabled_reason.clone())
            ))
            .collect::<HashMap<String, (bool, Option<McpServerDisabledReason>)>>(),
        HashMap::from([
            ("server-a".to_string(), (false, reason.clone())),
            ("server-b".to_string(), (false, reason)),
        ])
    );
}

#[test]
fn add_dir_override_extends_workspace_writable_roots() -> std::io::Result<()> {
    let temp_dir = TempDir::new()?;
    let frontend = temp_dir.path().join("frontend");
    let backend = temp_dir.path().join("backend");
    std::fs::create_dir_all(&frontend)?;
    std::fs::create_dir_all(&backend)?;

    let overrides = ConfigOverrides {
        cwd: Some(frontend),
        sandbox_mode: Some(SandboxMode::WorkspaceWrite),
        additional_writable_roots: vec![PathBuf::from("../backend"), backend.clone()],
        ..Default::default()
    };

    let config = Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        overrides,
        temp_dir.path().to_path_buf(),
    )?;

    let expected_backend = AbsolutePathBuf::try_from(backend).unwrap();
    if cfg!(target_os = "windows") {
        match config.permissions.sandbox_policy.get() {
            SandboxPolicy::ReadOnly { .. } => {}
            other => panic!("expected read-only policy on Windows, got {other:?}"),
        }
    } else {
        match config.permissions.sandbox_policy.get() {
            SandboxPolicy::WorkspaceWrite { writable_roots, .. } => {
                assert_eq!(
                    writable_roots
                        .iter()
                        .filter(|root| **root == expected_backend)
                        .count(),
                    1,
                    "expected single writable root entry for {}",
                    expected_backend.display()
                );
            }
            other => panic!("expected workspace-write policy, got {other:?}"),
        }
    }

    Ok(())
}

#[test]
fn sqlite_home_defaults_to_codex_home_for_workspace_write() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let config = Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides {
            sandbox_mode: Some(SandboxMode::WorkspaceWrite),
            ..Default::default()
        },
        codex_home.path().to_path_buf(),
    )?;

    assert_eq!(config.sqlite_home, codex_home.path().to_path_buf());

    Ok(())
}

#[test]
fn workspace_write_always_includes_memories_root_once() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let memories_root = codex_home.path().join("memories");
    let config = Config::load_from_base_config_with_overrides(
        ConfigToml {
            sandbox_workspace_write: Some(SandboxWorkspaceWrite {
                writable_roots: vec![AbsolutePathBuf::from_absolute_path(&memories_root)?],
                ..Default::default()
            }),
            ..Default::default()
        },
        ConfigOverrides {
            sandbox_mode: Some(SandboxMode::WorkspaceWrite),
            ..Default::default()
        },
        codex_home.path().to_path_buf(),
    )?;

    if cfg!(target_os = "windows") {
        match config.permissions.sandbox_policy.get() {
            SandboxPolicy::ReadOnly { .. } => {}
            other => panic!("expected read-only policy on Windows, got {other:?}"),
        }
    } else {
        assert!(
            memories_root.is_dir(),
            "expected memories root directory to exist at {}",
            memories_root.display()
        );
        let expected_memories_root = AbsolutePathBuf::from_absolute_path(&memories_root)?;
        match config.permissions.sandbox_policy.get() {
            SandboxPolicy::WorkspaceWrite { writable_roots, .. } => {
                assert_eq!(
                    writable_roots
                        .iter()
                        .filter(|root| **root == expected_memories_root)
                        .count(),
                    1,
                    "expected single writable root entry for {}",
                    expected_memories_root.display()
                );
            }
            other => panic!("expected workspace-write policy, got {other:?}"),
        }
    }

    Ok(())
}

#[test]
fn config_defaults_to_file_cli_auth_store_mode() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let cfg = ConfigToml::default();

    let config = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    )?;

    assert_eq!(
        config.cli_auth_credentials_store_mode,
        AuthCredentialsStoreMode::File,
    );

    Ok(())
}

#[test]
fn config_honors_explicit_keyring_auth_store_mode() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let cfg = ConfigToml {
        cli_auth_credentials_store: Some(AuthCredentialsStoreMode::Keyring),
        ..Default::default()
    };

    let config = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    )?;

    assert_eq!(
        config.cli_auth_credentials_store_mode,
        AuthCredentialsStoreMode::Keyring,
    );

    Ok(())
}

#[test]
fn config_defaults_to_auto_oauth_store_mode() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let cfg = ConfigToml::default();

    let config = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    )?;

    assert_eq!(
        config.mcp_oauth_credentials_store_mode,
        OAuthCredentialsStoreMode::Auto,
    );

    Ok(())
}

#[test]
fn feedback_enabled_defaults_to_true() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let cfg = ConfigToml {
        feedback: Some(FeedbackConfigToml::default()),
        ..Default::default()
    };

    let config = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    )?;

    assert_eq!(config.feedback_enabled, true);

    Ok(())
}

#[test]
fn web_search_mode_defaults_to_none_if_unset() {
    let cfg = ConfigToml::default();
    let profile = ConfigProfile::default();
    let features = Features::with_defaults();

    assert_eq!(resolve_web_search_mode(&cfg, &profile, &features), None);
}

#[test]
fn web_search_mode_prefers_profile_over_legacy_flags() {
    let cfg = ConfigToml::default();
    let profile = ConfigProfile {
        web_search: Some(WebSearchMode::Live),
        ..Default::default()
    };
    let mut features = Features::with_defaults();
    features.enable(Feature::WebSearchCached);

    assert_eq!(
        resolve_web_search_mode(&cfg, &profile, &features),
        Some(WebSearchMode::Live)
    );
}

#[test]
fn web_search_mode_disabled_overrides_legacy_request() {
    let cfg = ConfigToml {
        web_search: Some(WebSearchMode::Disabled),
        ..Default::default()
    };
    let profile = ConfigProfile::default();
    let mut features = Features::with_defaults();
    features.enable(Feature::WebSearchRequest);

    assert_eq!(
        resolve_web_search_mode(&cfg, &profile, &features),
        Some(WebSearchMode::Disabled)
    );
}

#[test]
fn web_search_mode_for_turn_uses_preference_for_read_only() {
    let web_search_mode = Constrained::allow_any(WebSearchMode::Cached);
    let mode =
        resolve_web_search_mode_for_turn(&web_search_mode, &SandboxPolicy::new_read_only_policy());

    assert_eq!(mode, WebSearchMode::Cached);
}

#[test]
fn web_search_mode_for_turn_prefers_live_for_danger_full_access() {
    let web_search_mode = Constrained::allow_any(WebSearchMode::Cached);
    let mode = resolve_web_search_mode_for_turn(&web_search_mode, &SandboxPolicy::DangerFullAccess);

    assert_eq!(mode, WebSearchMode::Live);
}

#[test]
fn web_search_mode_for_turn_respects_disabled_for_danger_full_access() {
    let web_search_mode = Constrained::allow_any(WebSearchMode::Disabled);
    let mode = resolve_web_search_mode_for_turn(&web_search_mode, &SandboxPolicy::DangerFullAccess);

    assert_eq!(mode, WebSearchMode::Disabled);
}

#[test]
fn web_search_mode_for_turn_falls_back_when_live_is_disallowed() -> anyhow::Result<()> {
    let allowed = [WebSearchMode::Disabled, WebSearchMode::Cached];
    let web_search_mode = Constrained::new(WebSearchMode::Cached, move |candidate| {
        if allowed.contains(candidate) {
            Ok(())
        } else {
            Err(ConstraintError::InvalidValue {
                field_name: "web_search_mode",
                candidate: format!("{candidate:?}"),
                allowed: format!("{allowed:?}"),
                requirement_source: RequirementSource::Unknown,
            })
        }
    })?;
    let mode = resolve_web_search_mode_for_turn(&web_search_mode, &SandboxPolicy::DangerFullAccess);

    assert_eq!(mode, WebSearchMode::Cached);
    Ok(())
}

#[tokio::test]
async fn project_profile_overrides_user_profile() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let workspace = TempDir::new()?;
    let workspace_key = workspace.path().to_string_lossy().replace('\\', "\\\\");
    std::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        format!(
            r#"
profile = "global"

[profiles.global]
model = "gpt-global"

[profiles.project]
model = "gpt-project"

[projects."{workspace_key}"]
trust_level = "trusted"
"#,
        ),
    )?;
    let project_config_dir = workspace.path().join(".codex");
    std::fs::create_dir_all(&project_config_dir)?;
    std::fs::write(
        project_config_dir.join(CONFIG_TOML_FILE),
        r#"
profile = "project"
"#,
    )?;

    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .harness_overrides(ConfigOverrides {
            cwd: Some(workspace.path().to_path_buf()),
            ..Default::default()
        })
        .build()
        .await?;

    assert_eq!(config.active_profile.as_deref(), Some("project"));
    assert_eq!(config.model.as_deref(), Some("gpt-project"));

    Ok(())
}

#[test]
fn profile_sandbox_mode_overrides_base() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let mut profiles = HashMap::new();
    profiles.insert(
        "work".to_string(),
        ConfigProfile {
            sandbox_mode: Some(SandboxMode::DangerFullAccess),
            ..Default::default()
        },
    );
    let cfg = ConfigToml {
        profiles,
        profile: Some("work".to_string()),
        sandbox_mode: Some(SandboxMode::ReadOnly),
        ..Default::default()
    };

    let config = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    )?;

    assert!(matches!(
        config.permissions.sandbox_policy.get(),
        &SandboxPolicy::DangerFullAccess
    ));

    Ok(())
}

#[test]
fn cli_override_takes_precedence_over_profile_sandbox_mode() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let mut profiles = HashMap::new();
    profiles.insert(
        "work".to_string(),
        ConfigProfile {
            sandbox_mode: Some(SandboxMode::DangerFullAccess),
            ..Default::default()
        },
    );
    let cfg = ConfigToml {
        profiles,
        profile: Some("work".to_string()),
        ..Default::default()
    };

    let overrides = ConfigOverrides {
        sandbox_mode: Some(SandboxMode::WorkspaceWrite),
        ..Default::default()
    };

    let config = Config::load_from_base_config_with_overrides(
        cfg,
        overrides,
        codex_home.path().to_path_buf(),
    )?;

    if cfg!(target_os = "windows") {
        assert!(matches!(
            config.permissions.sandbox_policy.get(),
            SandboxPolicy::ReadOnly { .. }
        ));
    } else {
        assert!(matches!(
            config.permissions.sandbox_policy.get(),
            SandboxPolicy::WorkspaceWrite { .. }
        ));
    }

    Ok(())
}

#[test]
fn feature_table_overrides_legacy_flags() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let mut entries = BTreeMap::new();
    entries.insert("apply_patch_freeform".to_string(), false);
    let cfg = ConfigToml {
        features: Some(crate::features::FeaturesToml { entries }),
        ..Default::default()
    };

    let config = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    )?;

    assert!(!config.features.enabled(Feature::ApplyPatchFreeform));
    assert!(!config.include_apply_patch_tool);

    Ok(())
}

#[test]
fn legacy_toggles_map_to_features() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let cfg = ConfigToml {
        experimental_use_unified_exec_tool: Some(true),
        experimental_use_freeform_apply_patch: Some(true),
        ..Default::default()
    };

    let config = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    )?;

    assert!(config.features.enabled(Feature::ApplyPatchFreeform));
    assert!(config.features.enabled(Feature::UnifiedExec));

    assert!(config.include_apply_patch_tool);

    assert!(config.use_experimental_unified_exec_tool);

    Ok(())
}

#[test]
fn responses_websocket_features_do_not_change_wire_api() -> std::io::Result<()> {
    for feature_key in ["responses_websockets", "responses_websockets_v2"] {
        let codex_home = TempDir::new()?;
        let mut entries = BTreeMap::new();
        entries.insert(feature_key.to_string(), true);
        let cfg = ConfigToml {
            features: Some(crate::features::FeaturesToml { entries }),
            ..Default::default()
        };

        let config = Config::load_from_base_config_with_overrides(
            cfg,
            ConfigOverrides::default(),
            codex_home.path().to_path_buf(),
        )?;

        assert_eq!(
            config.model_provider.wire_api,
            crate::model_provider_info::WireApi::Responses
        );
    }

    Ok(())
}

#[test]
fn config_honors_explicit_file_oauth_store_mode() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let cfg = ConfigToml {
        mcp_oauth_credentials_store: Some(OAuthCredentialsStoreMode::File),
        ..Default::default()
    };

    let config = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    )?;

    assert_eq!(
        config.mcp_oauth_credentials_store_mode,
        OAuthCredentialsStoreMode::File,
    );

    Ok(())
}

#[tokio::test]
async fn managed_config_overrides_oauth_store_mode() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;
    let managed_path = codex_home.path().join("managed_config.toml");
    let config_path = codex_home.path().join(CONFIG_TOML_FILE);

    std::fs::write(&config_path, "mcp_oauth_credentials_store = \"file\"\n")?;
    std::fs::write(&managed_path, "mcp_oauth_credentials_store = \"keyring\"\n")?;

    let overrides = LoaderOverrides {
        managed_config_path: Some(managed_path.clone()),
        #[cfg(target_os = "macos")]
        managed_preferences_base64: None,
        macos_managed_config_requirements_base64: None,
    };

    let cwd = AbsolutePathBuf::try_from(codex_home.path())?;
    let config_layer_stack = load_config_layers_state(
        codex_home.path(),
        Some(cwd),
        &Vec::new(),
        overrides,
        CloudRequirementsLoader::default(),
    )
    .await?;
    let cfg =
        deserialize_config_toml_with_base(config_layer_stack.effective_config(), codex_home.path())
            .map_err(|e| {
                tracing::error!("Failed to deserialize overridden config: {e}");
                e
            })?;
    assert_eq!(
        cfg.mcp_oauth_credentials_store,
        Some(OAuthCredentialsStoreMode::Keyring),
    );

    let final_config = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    )?;
    assert_eq!(
        final_config.mcp_oauth_credentials_store_mode,
        OAuthCredentialsStoreMode::Keyring,
    );

    Ok(())
}

#[tokio::test]
async fn load_global_mcp_servers_returns_empty_if_missing() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;

    let servers = load_global_mcp_servers(codex_home.path()).await?;
    assert!(servers.is_empty());

    Ok(())
}

#[tokio::test]
async fn replace_mcp_servers_round_trips_entries() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;

    let mut servers = BTreeMap::new();
    servers.insert(
        "docs".to_string(),
        McpServerConfig {
            transport: McpServerTransportConfig::Stdio {
                command: "echo".to_string(),
                args: vec!["hello".to_string()],
                env: None,
                env_vars: Vec::new(),
                cwd: None,
            },
            enabled: true,
            required: false,
            disabled_reason: None,
            startup_timeout_sec: Some(Duration::from_secs(3)),
            tool_timeout_sec: Some(Duration::from_secs(5)),
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth_resource: None,
        },
    );

    apply_blocking(
        codex_home.path(),
        None,
        &[ConfigEdit::ReplaceMcpServers(servers.clone())],
    )?;

    let loaded = load_global_mcp_servers(codex_home.path()).await?;
    assert_eq!(loaded.len(), 1);
    let docs = loaded.get("docs").expect("docs entry");
    match &docs.transport {
        McpServerTransportConfig::Stdio {
            command,
            args,
            env,
            env_vars,
            cwd,
        } => {
            assert_eq!(command, "echo");
            assert_eq!(args, &vec!["hello".to_string()]);
            assert!(env.is_none());
            assert!(env_vars.is_empty());
            assert!(cwd.is_none());
        }
        other => panic!("unexpected transport {other:?}"),
    }
    assert_eq!(docs.startup_timeout_sec, Some(Duration::from_secs(3)));
    assert_eq!(docs.tool_timeout_sec, Some(Duration::from_secs(5)));
    assert!(docs.enabled);

    let empty = BTreeMap::new();
    apply_blocking(
        codex_home.path(),
        None,
        &[ConfigEdit::ReplaceMcpServers(empty.clone())],
    )?;
    let loaded = load_global_mcp_servers(codex_home.path()).await?;
    assert!(loaded.is_empty());

    Ok(())
}

#[tokio::test]
async fn managed_config_wins_over_cli_overrides() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;
    let managed_path = codex_home.path().join("managed_config.toml");

    std::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        "model = \"base\"\n",
    )?;
    std::fs::write(&managed_path, "model = \"managed_config\"\n")?;

    let overrides = LoaderOverrides {
        managed_config_path: Some(managed_path),
        #[cfg(target_os = "macos")]
        managed_preferences_base64: None,
        macos_managed_config_requirements_base64: None,
    };

    let cwd = AbsolutePathBuf::try_from(codex_home.path())?;
    let config_layer_stack = load_config_layers_state(
        codex_home.path(),
        Some(cwd),
        &[("model".to_string(), TomlValue::String("cli".to_string()))],
        overrides,
        CloudRequirementsLoader::default(),
    )
    .await?;

    let cfg =
        deserialize_config_toml_with_base(config_layer_stack.effective_config(), codex_home.path())
            .map_err(|e| {
                tracing::error!("Failed to deserialize overridden config: {e}");
                e
            })?;

    assert_eq!(cfg.model.as_deref(), Some("managed_config"));
    Ok(())
}

#[tokio::test]
async fn load_global_mcp_servers_accepts_legacy_ms_field() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;
    let config_path = codex_home.path().join(CONFIG_TOML_FILE);

    std::fs::write(
        &config_path,
        r#"
[mcp_servers]
[mcp_servers.docs]
command = "echo"
startup_timeout_ms = 2500
"#,
    )?;

    let servers = load_global_mcp_servers(codex_home.path()).await?;
    let docs = servers.get("docs").expect("docs entry");
    assert_eq!(docs.startup_timeout_sec, Some(Duration::from_millis(2500)));

    Ok(())
}

#[tokio::test]
async fn load_global_mcp_servers_rejects_inline_bearer_token() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;
    let config_path = codex_home.path().join(CONFIG_TOML_FILE);

    std::fs::write(
        &config_path,
        r#"
[mcp_servers.docs]
url = "https://example.com/mcp"
bearer_token = "secret"
"#,
    )?;

    let err = load_global_mcp_servers(codex_home.path())
        .await
        .expect_err("bearer_token entries should be rejected");

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    assert!(err.to_string().contains("bearer_token"));
    assert!(err.to_string().contains("bearer_token_env_var"));

    Ok(())
}

#[tokio::test]
async fn replace_mcp_servers_serializes_env_sorted() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;

    let servers = BTreeMap::from([(
        "docs".to_string(),
        McpServerConfig {
            transport: McpServerTransportConfig::Stdio {
                command: "docs-server".to_string(),
                args: vec!["--verbose".to_string()],
                env: Some(HashMap::from([
                    ("ZIG_VAR".to_string(), "3".to_string()),
                    ("ALPHA_VAR".to_string(), "1".to_string()),
                ])),
                env_vars: Vec::new(),
                cwd: None,
            },
            enabled: true,
            required: false,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth_resource: None,
        },
    )]);

    apply_blocking(
        codex_home.path(),
        None,
        &[ConfigEdit::ReplaceMcpServers(servers.clone())],
    )?;

    let config_path = codex_home.path().join(CONFIG_TOML_FILE);
    let serialized = std::fs::read_to_string(&config_path)?;
    assert_eq!(
        serialized,
        r#"[mcp_servers.docs]
command = "docs-server"
args = ["--verbose"]

[mcp_servers.docs.env]
ALPHA_VAR = "1"
ZIG_VAR = "3"
"#
    );

    let loaded = load_global_mcp_servers(codex_home.path()).await?;
    let docs = loaded.get("docs").expect("docs entry");
    match &docs.transport {
        McpServerTransportConfig::Stdio {
            command,
            args,
            env,
            env_vars,
            cwd,
        } => {
            assert_eq!(command, "docs-server");
            assert_eq!(args, &vec!["--verbose".to_string()]);
            let env = env
                .as_ref()
                .expect("env should be preserved for stdio transport");
            assert_eq!(env.get("ALPHA_VAR"), Some(&"1".to_string()));
            assert_eq!(env.get("ZIG_VAR"), Some(&"3".to_string()));
            assert!(env_vars.is_empty());
            assert!(cwd.is_none());
        }
        other => panic!("unexpected transport {other:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn replace_mcp_servers_serializes_env_vars() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;

    let servers = BTreeMap::from([(
        "docs".to_string(),
        McpServerConfig {
            transport: McpServerTransportConfig::Stdio {
                command: "docs-server".to_string(),
                args: Vec::new(),
                env: None,
                env_vars: vec!["ALPHA".to_string(), "BETA".to_string()],
                cwd: None,
            },
            enabled: true,
            required: false,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth_resource: None,
        },
    )]);

    apply_blocking(
        codex_home.path(),
        None,
        &[ConfigEdit::ReplaceMcpServers(servers.clone())],
    )?;

    let config_path = codex_home.path().join(CONFIG_TOML_FILE);
    let serialized = std::fs::read_to_string(&config_path)?;
    assert!(
        serialized.contains(r#"env_vars = ["ALPHA", "BETA"]"#),
        "serialized config missing env_vars field:\n{serialized}"
    );

    let loaded = load_global_mcp_servers(codex_home.path()).await?;
    let docs = loaded.get("docs").expect("docs entry");
    match &docs.transport {
        McpServerTransportConfig::Stdio { env_vars, .. } => {
            assert_eq!(env_vars, &vec!["ALPHA".to_string(), "BETA".to_string()]);
        }
        other => panic!("unexpected transport {other:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn replace_mcp_servers_serializes_cwd() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;

    let cwd_path = PathBuf::from("/tmp/codex-mcp");
    let servers = BTreeMap::from([(
        "docs".to_string(),
        McpServerConfig {
            transport: McpServerTransportConfig::Stdio {
                command: "docs-server".to_string(),
                args: Vec::new(),
                env: None,
                env_vars: Vec::new(),
                cwd: Some(cwd_path.clone()),
            },
            enabled: true,
            required: false,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth_resource: None,
        },
    )]);

    apply_blocking(
        codex_home.path(),
        None,
        &[ConfigEdit::ReplaceMcpServers(servers.clone())],
    )?;

    let config_path = codex_home.path().join(CONFIG_TOML_FILE);
    let serialized = std::fs::read_to_string(&config_path)?;
    assert!(
        serialized.contains(r#"cwd = "/tmp/codex-mcp""#),
        "serialized config missing cwd field:\n{serialized}"
    );

    let loaded = load_global_mcp_servers(codex_home.path()).await?;
    let docs = loaded.get("docs").expect("docs entry");
    match &docs.transport {
        McpServerTransportConfig::Stdio { cwd, .. } => {
            assert_eq!(cwd.as_deref(), Some(Path::new("/tmp/codex-mcp")));
        }
        other => panic!("unexpected transport {other:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn replace_mcp_servers_streamable_http_serializes_bearer_token() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;

    let servers = BTreeMap::from([(
        "docs".to_string(),
        McpServerConfig {
            transport: McpServerTransportConfig::StreamableHttp {
                url: "https://example.com/mcp".to_string(),
                bearer_token_env_var: Some("MCP_TOKEN".to_string()),
                http_headers: None,
                env_http_headers: None,
            },
            enabled: true,
            required: false,
            disabled_reason: None,
            startup_timeout_sec: Some(Duration::from_secs(2)),
            tool_timeout_sec: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth_resource: None,
        },
    )]);

    apply_blocking(
        codex_home.path(),
        None,
        &[ConfigEdit::ReplaceMcpServers(servers.clone())],
    )?;

    let config_path = codex_home.path().join(CONFIG_TOML_FILE);
    let serialized = std::fs::read_to_string(&config_path)?;
    assert_eq!(
        serialized,
        r#"[mcp_servers.docs]
url = "https://example.com/mcp"
bearer_token_env_var = "MCP_TOKEN"
startup_timeout_sec = 2.0
"#
    );

    let loaded = load_global_mcp_servers(codex_home.path()).await?;
    let docs = loaded.get("docs").expect("docs entry");
    match &docs.transport {
        McpServerTransportConfig::StreamableHttp {
            url,
            bearer_token_env_var,
            http_headers,
            env_http_headers,
        } => {
            assert_eq!(url, "https://example.com/mcp");
            assert_eq!(bearer_token_env_var.as_deref(), Some("MCP_TOKEN"));
            assert!(http_headers.is_none());
            assert!(env_http_headers.is_none());
        }
        other => panic!("unexpected transport {other:?}"),
    }
    assert_eq!(docs.startup_timeout_sec, Some(Duration::from_secs(2)));

    Ok(())
}

#[tokio::test]
async fn replace_mcp_servers_streamable_http_serializes_custom_headers() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;

    let servers = BTreeMap::from([(
        "docs".to_string(),
        McpServerConfig {
            transport: McpServerTransportConfig::StreamableHttp {
                url: "https://example.com/mcp".to_string(),
                bearer_token_env_var: Some("MCP_TOKEN".to_string()),
                http_headers: Some(HashMap::from([("X-Doc".to_string(), "42".to_string())])),
                env_http_headers: Some(HashMap::from([(
                    "X-Auth".to_string(),
                    "DOCS_AUTH".to_string(),
                )])),
            },
            enabled: true,
            required: false,
            disabled_reason: None,
            startup_timeout_sec: Some(Duration::from_secs(2)),
            tool_timeout_sec: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth_resource: None,
        },
    )]);
    apply_blocking(
        codex_home.path(),
        None,
        &[ConfigEdit::ReplaceMcpServers(servers.clone())],
    )?;

    let config_path = codex_home.path().join(CONFIG_TOML_FILE);
    let serialized = std::fs::read_to_string(&config_path)?;
    assert_eq!(
        serialized,
        r#"[mcp_servers.docs]
url = "https://example.com/mcp"
bearer_token_env_var = "MCP_TOKEN"
startup_timeout_sec = 2.0

[mcp_servers.docs.http_headers]
X-Doc = "42"

[mcp_servers.docs.env_http_headers]
X-Auth = "DOCS_AUTH"
"#
    );

    let loaded = load_global_mcp_servers(codex_home.path()).await?;
    let docs = loaded.get("docs").expect("docs entry");
    match &docs.transport {
        McpServerTransportConfig::StreamableHttp {
            http_headers,
            env_http_headers,
            ..
        } => {
            assert_eq!(
                http_headers,
                &Some(HashMap::from([("X-Doc".to_string(), "42".to_string())]))
            );
            assert_eq!(
                env_http_headers,
                &Some(HashMap::from([(
                    "X-Auth".to_string(),
                    "DOCS_AUTH".to_string()
                )]))
            );
        }
        other => panic!("unexpected transport {other:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn replace_mcp_servers_streamable_http_removes_optional_sections() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;

    let config_path = codex_home.path().join(CONFIG_TOML_FILE);

    let mut servers = BTreeMap::from([(
        "docs".to_string(),
        McpServerConfig {
            transport: McpServerTransportConfig::StreamableHttp {
                url: "https://example.com/mcp".to_string(),
                bearer_token_env_var: Some("MCP_TOKEN".to_string()),
                http_headers: Some(HashMap::from([("X-Doc".to_string(), "42".to_string())])),
                env_http_headers: Some(HashMap::from([(
                    "X-Auth".to_string(),
                    "DOCS_AUTH".to_string(),
                )])),
            },
            enabled: true,
            required: false,
            disabled_reason: None,
            startup_timeout_sec: Some(Duration::from_secs(2)),
            tool_timeout_sec: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth_resource: None,
        },
    )]);

    apply_blocking(
        codex_home.path(),
        None,
        &[ConfigEdit::ReplaceMcpServers(servers.clone())],
    )?;
    let serialized_with_optional = std::fs::read_to_string(&config_path)?;
    assert!(serialized_with_optional.contains("bearer_token_env_var = \"MCP_TOKEN\""));
    assert!(serialized_with_optional.contains("[mcp_servers.docs.http_headers]"));
    assert!(serialized_with_optional.contains("[mcp_servers.docs.env_http_headers]"));

    servers.insert(
        "docs".to_string(),
        McpServerConfig {
            transport: McpServerTransportConfig::StreamableHttp {
                url: "https://example.com/mcp".to_string(),
                bearer_token_env_var: None,
                http_headers: None,
                env_http_headers: None,
            },
            enabled: true,
            required: false,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth_resource: None,
        },
    );
    apply_blocking(
        codex_home.path(),
        None,
        &[ConfigEdit::ReplaceMcpServers(servers.clone())],
    )?;

    let serialized = std::fs::read_to_string(&config_path)?;
    assert_eq!(
        serialized,
        r#"[mcp_servers.docs]
url = "https://example.com/mcp"
"#
    );

    let loaded = load_global_mcp_servers(codex_home.path()).await?;
    let docs = loaded.get("docs").expect("docs entry");
    match &docs.transport {
        McpServerTransportConfig::StreamableHttp {
            url,
            bearer_token_env_var,
            http_headers,
            env_http_headers,
        } => {
            assert_eq!(url, "https://example.com/mcp");
            assert!(bearer_token_env_var.is_none());
            assert!(http_headers.is_none());
            assert!(env_http_headers.is_none());
        }
        other => panic!("unexpected transport {other:?}"),
    }

    assert!(docs.startup_timeout_sec.is_none());

    Ok(())
}

#[tokio::test]
async fn replace_mcp_servers_streamable_http_isolates_headers_between_servers() -> anyhow::Result<()>
{
    let codex_home = TempDir::new()?;
    let config_path = codex_home.path().join(CONFIG_TOML_FILE);

    let servers = BTreeMap::from([
        (
            "docs".to_string(),
            McpServerConfig {
                transport: McpServerTransportConfig::StreamableHttp {
                    url: "https://example.com/mcp".to_string(),
                    bearer_token_env_var: Some("MCP_TOKEN".to_string()),
                    http_headers: Some(HashMap::from([("X-Doc".to_string(), "42".to_string())])),
                    env_http_headers: Some(HashMap::from([(
                        "X-Auth".to_string(),
                        "DOCS_AUTH".to_string(),
                    )])),
                },
                enabled: true,
                required: false,
                disabled_reason: None,
                startup_timeout_sec: Some(Duration::from_secs(2)),
                tool_timeout_sec: None,
                enabled_tools: None,
                disabled_tools: None,
                scopes: None,
                oauth_resource: None,
            },
        ),
        (
            "logs".to_string(),
            McpServerConfig {
                transport: McpServerTransportConfig::Stdio {
                    command: "logs-server".to_string(),
                    args: vec!["--follow".to_string()],
                    env: None,
                    env_vars: Vec::new(),
                    cwd: None,
                },
                enabled: true,
                required: false,
                disabled_reason: None,
                startup_timeout_sec: None,
                tool_timeout_sec: None,
                enabled_tools: None,
                disabled_tools: None,
                scopes: None,
                oauth_resource: None,
            },
        ),
    ]);

    apply_blocking(
        codex_home.path(),
        None,
        &[ConfigEdit::ReplaceMcpServers(servers.clone())],
    )?;

    let serialized = std::fs::read_to_string(&config_path)?;
    assert!(
        serialized.contains("[mcp_servers.docs.http_headers]"),
        "serialized config missing docs headers section:\n{serialized}"
    );
    assert!(
        !serialized.contains("[mcp_servers.logs.http_headers]"),
        "serialized config should not add logs headers section:\n{serialized}"
    );
    assert!(
        !serialized.contains("[mcp_servers.logs.env_http_headers]"),
        "serialized config should not add logs env headers section:\n{serialized}"
    );
    assert!(
        !serialized.contains("mcp_servers.logs.bearer_token_env_var"),
        "serialized config should not add bearer token to logs:\n{serialized}"
    );

    let loaded = load_global_mcp_servers(codex_home.path()).await?;
    let docs = loaded.get("docs").expect("docs entry");
    match &docs.transport {
        McpServerTransportConfig::StreamableHttp {
            http_headers,
            env_http_headers,
            ..
        } => {
            assert_eq!(
                http_headers,
                &Some(HashMap::from([("X-Doc".to_string(), "42".to_string())]))
            );
            assert_eq!(
                env_http_headers,
                &Some(HashMap::from([(
                    "X-Auth".to_string(),
                    "DOCS_AUTH".to_string()
                )]))
            );
        }
        other => panic!("unexpected transport {other:?}"),
    }
    let logs = loaded.get("logs").expect("logs entry");
    match &logs.transport {
        McpServerTransportConfig::Stdio { env, .. } => {
            assert!(env.is_none());
        }
        other => panic!("unexpected transport {other:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn replace_mcp_servers_serializes_disabled_flag() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;

    let servers = BTreeMap::from([(
        "docs".to_string(),
        McpServerConfig {
            transport: McpServerTransportConfig::Stdio {
                command: "docs-server".to_string(),
                args: Vec::new(),
                env: None,
                env_vars: Vec::new(),
                cwd: None,
            },
            enabled: false,
            required: false,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth_resource: None,
        },
    )]);

    apply_blocking(
        codex_home.path(),
        None,
        &[ConfigEdit::ReplaceMcpServers(servers.clone())],
    )?;

    let config_path = codex_home.path().join(CONFIG_TOML_FILE);
    let serialized = std::fs::read_to_string(&config_path)?;
    assert!(
        serialized.contains("enabled = false"),
        "serialized config missing disabled flag:\n{serialized}"
    );

    let loaded = load_global_mcp_servers(codex_home.path()).await?;
    let docs = loaded.get("docs").expect("docs entry");
    assert!(!docs.enabled);

    Ok(())
}

#[tokio::test]
async fn replace_mcp_servers_serializes_required_flag() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;

    let servers = BTreeMap::from([(
        "docs".to_string(),
        McpServerConfig {
            transport: McpServerTransportConfig::Stdio {
                command: "docs-server".to_string(),
                args: Vec::new(),
                env: None,
                env_vars: Vec::new(),
                cwd: None,
            },
            enabled: true,
            required: true,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth_resource: None,
        },
    )]);

    apply_blocking(
        codex_home.path(),
        None,
        &[ConfigEdit::ReplaceMcpServers(servers.clone())],
    )?;

    let config_path = codex_home.path().join(CONFIG_TOML_FILE);
    let serialized = std::fs::read_to_string(&config_path)?;
    assert!(
        serialized.contains("required = true"),
        "serialized config missing required flag:\n{serialized}"
    );

    let loaded = load_global_mcp_servers(codex_home.path()).await?;
    let docs = loaded.get("docs").expect("docs entry");
    assert!(docs.required);

    Ok(())
}

#[tokio::test]
async fn replace_mcp_servers_serializes_tool_filters() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;

    let servers = BTreeMap::from([(
        "docs".to_string(),
        McpServerConfig {
            transport: McpServerTransportConfig::Stdio {
                command: "docs-server".to_string(),
                args: Vec::new(),
                env: None,
                env_vars: Vec::new(),
                cwd: None,
            },
            enabled: true,
            required: false,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled_tools: Some(vec!["allowed".to_string()]),
            disabled_tools: Some(vec!["blocked".to_string()]),
            scopes: None,
            oauth_resource: None,
        },
    )]);

    apply_blocking(
        codex_home.path(),
        None,
        &[ConfigEdit::ReplaceMcpServers(servers.clone())],
    )?;

    let config_path = codex_home.path().join(CONFIG_TOML_FILE);
    let serialized = std::fs::read_to_string(&config_path)?;
    assert!(serialized.contains(r#"enabled_tools = ["allowed"]"#));
    assert!(serialized.contains(r#"disabled_tools = ["blocked"]"#));

    let loaded = load_global_mcp_servers(codex_home.path()).await?;
    let docs = loaded.get("docs").expect("docs entry");
    assert_eq!(
        docs.enabled_tools.as_ref(),
        Some(&vec!["allowed".to_string()])
    );
    assert_eq!(
        docs.disabled_tools.as_ref(),
        Some(&vec!["blocked".to_string()])
    );

    Ok(())
}

#[tokio::test]
async fn replace_mcp_servers_streamable_http_serializes_oauth_resource() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;

    let servers = BTreeMap::from([(
        "docs".to_string(),
        McpServerConfig {
            transport: McpServerTransportConfig::StreamableHttp {
                url: "https://example.com/mcp".to_string(),
                bearer_token_env_var: None,
                http_headers: None,
                env_http_headers: None,
            },
            enabled: true,
            required: false,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth_resource: Some("https://resource.example.com".to_string()),
        },
    )]);

    apply_blocking(
        codex_home.path(),
        None,
        &[ConfigEdit::ReplaceMcpServers(servers.clone())],
    )?;

    let config_path = codex_home.path().join(CONFIG_TOML_FILE);
    let serialized = std::fs::read_to_string(&config_path)?;
    assert!(serialized.contains(r#"oauth_resource = "https://resource.example.com""#));

    let loaded = load_global_mcp_servers(codex_home.path()).await?;
    let docs = loaded.get("docs").expect("docs entry");
    assert_eq!(
        docs.oauth_resource.as_deref(),
        Some("https://resource.example.com")
    );

    Ok(())
}

#[tokio::test]
async fn set_model_updates_defaults() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;

    ConfigEditsBuilder::new(codex_home.path())
        .set_model(Some("gpt-5.1-codex"), Some(ReasoningEffort::High))
        .apply()
        .await?;

    let serialized = tokio::fs::read_to_string(codex_home.path().join(CONFIG_TOML_FILE)).await?;
    let parsed: ConfigToml = toml::from_str(&serialized)?;

    assert_eq!(parsed.model.as_deref(), Some("gpt-5.1-codex"));
    assert_eq!(parsed.model_reasoning_effort, Some(ReasoningEffort::High));

    Ok(())
}

#[tokio::test]
async fn set_model_overwrites_existing_model() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;
    let config_path = codex_home.path().join(CONFIG_TOML_FILE);

    tokio::fs::write(
        &config_path,
        r#"
model = "gpt-5.1-codex"
model_reasoning_effort = "medium"

[profiles.dev]
model = "gpt-4.1"
"#,
    )
    .await?;

    ConfigEditsBuilder::new(codex_home.path())
        .set_model(Some("o4-mini"), Some(ReasoningEffort::High))
        .apply()
        .await?;

    let serialized = tokio::fs::read_to_string(config_path).await?;
    let parsed: ConfigToml = toml::from_str(&serialized)?;

    assert_eq!(parsed.model.as_deref(), Some("o4-mini"));
    assert_eq!(parsed.model_reasoning_effort, Some(ReasoningEffort::High));
    assert_eq!(
        parsed
            .profiles
            .get("dev")
            .and_then(|profile| profile.model.as_deref()),
        Some("gpt-4.1"),
    );

    Ok(())
}

#[tokio::test]
async fn set_model_updates_profile() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;

    ConfigEditsBuilder::new(codex_home.path())
        .with_profile(Some("dev"))
        .set_model(Some("gpt-5.1-codex"), Some(ReasoningEffort::Medium))
        .apply()
        .await?;

    let serialized = tokio::fs::read_to_string(codex_home.path().join(CONFIG_TOML_FILE)).await?;
    let parsed: ConfigToml = toml::from_str(&serialized)?;
    let profile = parsed
        .profiles
        .get("dev")
        .expect("profile should be created");

    assert_eq!(profile.model.as_deref(), Some("gpt-5.1-codex"));
    assert_eq!(
        profile.model_reasoning_effort,
        Some(ReasoningEffort::Medium)
    );

    Ok(())
}

#[tokio::test]
async fn set_model_updates_existing_profile() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;
    let config_path = codex_home.path().join(CONFIG_TOML_FILE);

    tokio::fs::write(
        &config_path,
        r#"
[profiles.dev]
model = "gpt-4"
model_reasoning_effort = "medium"

[profiles.prod]
model = "gpt-5.1-codex"
"#,
    )
    .await?;

    ConfigEditsBuilder::new(codex_home.path())
        .with_profile(Some("dev"))
        .set_model(Some("o4-high"), Some(ReasoningEffort::Medium))
        .apply()
        .await?;

    let serialized = tokio::fs::read_to_string(config_path).await?;
    let parsed: ConfigToml = toml::from_str(&serialized)?;

    let dev_profile = parsed
        .profiles
        .get("dev")
        .expect("dev profile should survive updates");
    assert_eq!(dev_profile.model.as_deref(), Some("o4-high"));
    assert_eq!(
        dev_profile.model_reasoning_effort,
        Some(ReasoningEffort::Medium)
    );

    assert_eq!(
        parsed
            .profiles
            .get("prod")
            .and_then(|profile| profile.model.as_deref()),
        Some("gpt-5.1-codex"),
    );

    Ok(())
}

#[tokio::test]
async fn set_feature_enabled_updates_profile() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;

    ConfigEditsBuilder::new(codex_home.path())
        .with_profile(Some("dev"))
        .set_feature_enabled("guardian_approval", true)
        .apply()
        .await?;

    let serialized = tokio::fs::read_to_string(codex_home.path().join(CONFIG_TOML_FILE)).await?;
    let parsed: ConfigToml = toml::from_str(&serialized)?;
    let profile = parsed
        .profiles
        .get("dev")
        .expect("profile should be created");

    assert_eq!(
        profile
            .features
            .as_ref()
            .and_then(|features| features.entries.get("guardian_approval")),
        Some(&true),
    );
    assert_eq!(
        parsed
            .features
            .as_ref()
            .and_then(|features| features.entries.get("guardian_approval")),
        None,
    );

    Ok(())
}

struct PrecedenceTestFixture {
    cwd: TempDir,
    codex_home: TempDir,
    cfg: ConfigToml,
    model_provider_map: HashMap<String, ModelProviderInfo>,
    openai_provider: ModelProviderInfo,
    openai_custom_provider: ModelProviderInfo,
}

impl PrecedenceTestFixture {
    fn cwd(&self) -> PathBuf {
        self.cwd.path().to_path_buf()
    }

    fn codex_home(&self) -> PathBuf {
        self.codex_home.path().to_path_buf()
    }
}

#[test]
fn cli_override_sets_compact_prompt() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let overrides = ConfigOverrides {
        compact_prompt: Some("Use the compact override".to_string()),
        ..Default::default()
    };

    let config = Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        overrides,
        codex_home.path().to_path_buf(),
    )?;

    assert_eq!(
        config.compact_prompt.as_deref(),
        Some("Use the compact override")
    );

    Ok(())
}

#[test]
fn loads_compact_prompt_from_file() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let workspace = codex_home.path().join("workspace");
    std::fs::create_dir_all(&workspace)?;

    let prompt_path = workspace.join("compact_prompt.txt");
    std::fs::write(&prompt_path, "  summarize differently  ")?;

    let cfg = ConfigToml {
        experimental_compact_prompt_file: Some(AbsolutePathBuf::from_absolute_path(prompt_path)?),
        ..Default::default()
    };

    let overrides = ConfigOverrides {
        cwd: Some(workspace),
        ..Default::default()
    };

    let config = Config::load_from_base_config_with_overrides(
        cfg,
        overrides,
        codex_home.path().to_path_buf(),
    )?;

    assert_eq!(
        config.compact_prompt.as_deref(),
        Some("summarize differently")
    );

    Ok(())
}

#[test]
fn load_config_rejects_missing_agent_role_config_file() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let missing_path = codex_home.path().join("agents").join("researcher.toml");
    let cfg = ConfigToml {
        agents: Some(AgentsToml {
            max_threads: None,
            max_depth: None,
            job_max_runtime_seconds: None,
            roles: BTreeMap::from([(
                "researcher".to_string(),
                AgentRoleToml {
                    description: Some("Research role".to_string()),
                    config_file: Some(AbsolutePathBuf::from_absolute_path(missing_path)?),
                    nickname_candidates: None,
                },
            )]),
        }),
        ..Default::default()
    };

    let result = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    );
    let err = result.expect_err("missing role config file should be rejected");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    let message = err.to_string();
    assert!(message.contains("agents.researcher.config_file"));
    assert!(message.contains("must point to an existing file"));

    Ok(())
}

#[tokio::test]
async fn agent_role_relative_config_file_resolves_against_config_toml() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let role_config_path = codex_home.path().join("agents").join("researcher.toml");
    tokio::fs::create_dir_all(
        role_config_path
            .parent()
            .expect("role config should have a parent directory"),
    )
    .await?;
    tokio::fs::write(
        &role_config_path,
        "developer_instructions = \"Research carefully\"\nmodel = \"gpt-5\"",
    )
    .await?;
    tokio::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        r#"[agents.researcher]
description = "Research role"
config_file = "./agents/researcher.toml"
nickname_candidates = ["Hypatia", "Noether"]
"#,
    )
    .await?;

    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .build()
        .await?;
    assert_eq!(
        config
            .agent_roles
            .get("researcher")
            .and_then(|role| role.config_file.as_ref()),
        Some(&role_config_path)
    );
    assert_eq!(
        config
            .agent_roles
            .get("researcher")
            .and_then(|role| role.nickname_candidates.as_ref())
            .map(|candidates| candidates.iter().map(String::as_str).collect::<Vec<_>>()),
        Some(vec!["Hypatia", "Noether"])
    );

    Ok(())
}

#[tokio::test]
async fn agent_role_file_metadata_overrides_config_toml_metadata() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let role_config_path = codex_home.path().join("agents").join("researcher.toml");
    tokio::fs::create_dir_all(
        role_config_path
            .parent()
            .expect("role config should have a parent directory"),
    )
    .await?;
    tokio::fs::write(
        &role_config_path,
        r#"
description = "Role metadata from file"
nickname_candidates = ["Hypatia"]
developer_instructions = "Research carefully"
model = "gpt-5"
"#,
    )
    .await?;
    tokio::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        r#"[agents.researcher]
description = "Research role from config"
config_file = "./agents/researcher.toml"
nickname_candidates = ["Noether"]
"#,
    )
    .await?;

    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .build()
        .await?;
    let role = config
        .agent_roles
        .get("researcher")
        .expect("researcher role should load");
    assert_eq!(role.description.as_deref(), Some("Role metadata from file"));
    assert_eq!(role.config_file.as_ref(), Some(&role_config_path));
    assert_eq!(
        role.nickname_candidates
            .as_ref()
            .map(|candidates| candidates.iter().map(String::as_str).collect::<Vec<_>>()),
        Some(vec!["Hypatia"])
    );

    Ok(())
}

#[tokio::test]
async fn agent_role_file_without_developer_instructions_is_dropped_with_warning()
-> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let repo_root = TempDir::new()?;
    let nested_cwd = repo_root.path().join("packages").join("app");
    std::fs::create_dir_all(repo_root.path().join(".git"))?;
    std::fs::create_dir_all(&nested_cwd)?;

    let workspace_key = repo_root.path().to_string_lossy().replace('\\', "\\\\");
    tokio::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        format!(
            r#"[projects."{workspace_key}"]
trust_level = "trusted"
"#
        ),
    )
    .await?;

    let standalone_agents_dir = repo_root.path().join(".codex").join("agents");
    tokio::fs::create_dir_all(&standalone_agents_dir).await?;
    tokio::fs::write(
        standalone_agents_dir.join("researcher.toml"),
        r#"
name = "researcher"
description = "Role metadata from file"
model = "gpt-5"
"#,
    )
    .await?;
    tokio::fs::write(
        standalone_agents_dir.join("reviewer.toml"),
        r#"
name = "reviewer"
description = "Review role"
developer_instructions = "Review carefully"
model = "gpt-5"
"#,
    )
    .await?;

    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .harness_overrides(ConfigOverrides {
            cwd: Some(nested_cwd),
            ..Default::default()
        })
        .build()
        .await?;
    assert!(!config.agent_roles.contains_key("researcher"));
    assert_eq!(
        config
            .agent_roles
            .get("reviewer")
            .and_then(|role| role.description.as_deref()),
        Some("Review role")
    );
    assert!(
        config
            .startup_warnings
            .iter()
            .any(|warning| warning.contains("must define `developer_instructions`"))
    );

    Ok(())
}

#[tokio::test]
async fn legacy_agent_role_config_file_allows_missing_developer_instructions() -> std::io::Result<()>
{
    let codex_home = TempDir::new()?;
    let role_config_path = codex_home.path().join("agents").join("researcher.toml");
    tokio::fs::create_dir_all(
        role_config_path
            .parent()
            .expect("role config should have a parent directory"),
    )
    .await?;
    tokio::fs::write(
        &role_config_path,
        r#"
model = "gpt-5"
model_reasoning_effort = "high"
"#,
    )
    .await?;
    tokio::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        r#"[agents.researcher]
description = "Research role from config"
config_file = "./agents/researcher.toml"
"#,
    )
    .await?;

    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .build()
        .await?;
    assert_eq!(
        config
            .agent_roles
            .get("researcher")
            .and_then(|role| role.description.as_deref()),
        Some("Research role from config")
    );
    assert_eq!(
        config
            .agent_roles
            .get("researcher")
            .and_then(|role| role.config_file.as_ref()),
        Some(&role_config_path)
    );

    Ok(())
}

#[tokio::test]
async fn agent_role_without_description_after_merge_is_dropped_with_warning() -> std::io::Result<()>
{
    let codex_home = TempDir::new()?;
    let role_config_path = codex_home.path().join("agents").join("researcher.toml");
    tokio::fs::create_dir_all(
        role_config_path
            .parent()
            .expect("role config should have a parent directory"),
    )
    .await?;
    tokio::fs::write(
        &role_config_path,
        r#"
developer_instructions = "Research carefully"
model = "gpt-5"
"#,
    )
    .await?;
    tokio::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        r#"[agents.researcher]
config_file = "./agents/researcher.toml"

[agents.reviewer]
description = "Review role"
"#,
    )
    .await?;

    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .build()
        .await?;
    assert!(!config.agent_roles.contains_key("researcher"));
    assert_eq!(
        config
            .agent_roles
            .get("reviewer")
            .and_then(|role| role.description.as_deref()),
        Some("Review role")
    );
    assert!(
        config
            .startup_warnings
            .iter()
            .any(|warning| warning.contains("agent role `researcher` must define a description"))
    );

    Ok(())
}

#[tokio::test]
async fn discovered_agent_role_file_without_name_is_dropped_with_warning() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let repo_root = TempDir::new()?;
    let nested_cwd = repo_root.path().join("packages").join("app");
    std::fs::create_dir_all(repo_root.path().join(".git"))?;
    std::fs::create_dir_all(&nested_cwd)?;

    let workspace_key = repo_root.path().to_string_lossy().replace('\\', "\\\\");
    tokio::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        format!(
            r#"[projects."{workspace_key}"]
trust_level = "trusted"
"#
        ),
    )
    .await?;

    let standalone_agents_dir = repo_root.path().join(".codex").join("agents");
    tokio::fs::create_dir_all(&standalone_agents_dir).await?;
    tokio::fs::write(
        standalone_agents_dir.join("researcher.toml"),
        r#"
description = "Role metadata from file"
developer_instructions = "Research carefully"
"#,
    )
    .await?;
    tokio::fs::write(
        standalone_agents_dir.join("reviewer.toml"),
        r#"
name = "reviewer"
description = "Review role"
developer_instructions = "Review carefully"
"#,
    )
    .await?;

    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .harness_overrides(ConfigOverrides {
            cwd: Some(nested_cwd),
            ..Default::default()
        })
        .build()
        .await?;
    assert!(!config.agent_roles.contains_key("researcher"));
    assert_eq!(
        config
            .agent_roles
            .get("reviewer")
            .and_then(|role| role.description.as_deref()),
        Some("Review role")
    );
    assert!(
        config
            .startup_warnings
            .iter()
            .any(|warning| warning.contains("must define a non-empty `name`"))
    );

    Ok(())
}

#[tokio::test]
async fn agent_role_file_name_takes_precedence_over_config_key() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let role_config_path = codex_home.path().join("agents").join("researcher.toml");
    tokio::fs::create_dir_all(
        role_config_path
            .parent()
            .expect("role config should have a parent directory"),
    )
    .await?;
    tokio::fs::write(
        &role_config_path,
        r#"
name = "archivist"
description = "Role metadata from file"
developer_instructions = "Research carefully"
model = "gpt-5"
"#,
    )
    .await?;
    tokio::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        r#"[agents.researcher]
description = "Research role from config"
config_file = "./agents/researcher.toml"
"#,
    )
    .await?;

    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .build()
        .await?;
    assert_eq!(config.agent_roles.contains_key("researcher"), false);
    let role = config
        .agent_roles
        .get("archivist")
        .expect("role should use file-provided name");
    assert_eq!(role.description.as_deref(), Some("Role metadata from file"));
    assert_eq!(role.config_file.as_ref(), Some(&role_config_path));

    Ok(())
}

#[tokio::test]
async fn loads_legacy_split_agent_roles_from_config_toml() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let researcher_path = codex_home.path().join("agents").join("researcher.toml");
    let reviewer_path = codex_home.path().join("agents").join("reviewer.toml");
    tokio::fs::create_dir_all(
        researcher_path
            .parent()
            .expect("role config should have a parent directory"),
    )
    .await?;
    tokio::fs::write(
        &researcher_path,
        "developer_instructions = \"Research carefully\"\nmodel = \"gpt-5\"",
    )
    .await?;
    tokio::fs::write(
        &reviewer_path,
        "developer_instructions = \"Review carefully\"\nmodel = \"gpt-4.1\"",
    )
    .await?;
    tokio::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        r#"[agents.researcher]
description = "Research role"
config_file = "./agents/researcher.toml"
nickname_candidates = ["Hypatia", "Noether"]

[agents.reviewer]
description = "Review role"
config_file = "./agents/reviewer.toml"
nickname_candidates = ["Atlas"]
"#,
    )
    .await?;

    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .build()
        .await?;

    assert_eq!(
        config
            .agent_roles
            .get("researcher")
            .and_then(|role| role.description.as_deref()),
        Some("Research role")
    );
    assert_eq!(
        config
            .agent_roles
            .get("researcher")
            .and_then(|role| role.config_file.as_ref()),
        Some(&researcher_path)
    );
    assert_eq!(
        config
            .agent_roles
            .get("researcher")
            .and_then(|role| role.nickname_candidates.as_ref())
            .map(|candidates| candidates.iter().map(String::as_str).collect::<Vec<_>>()),
        Some(vec!["Hypatia", "Noether"])
    );
    assert_eq!(
        config
            .agent_roles
            .get("reviewer")
            .and_then(|role| role.description.as_deref()),
        Some("Review role")
    );
    assert_eq!(
        config
            .agent_roles
            .get("reviewer")
            .and_then(|role| role.config_file.as_ref()),
        Some(&reviewer_path)
    );
    assert_eq!(
        config
            .agent_roles
            .get("reviewer")
            .and_then(|role| role.nickname_candidates.as_ref())
            .map(|candidates| candidates.iter().map(String::as_str).collect::<Vec<_>>()),
        Some(vec!["Atlas"])
    );

    Ok(())
}

#[tokio::test]
async fn discovers_multiple_standalone_agent_role_files() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let repo_root = TempDir::new()?;
    let nested_cwd = repo_root.path().join("packages").join("app");
    std::fs::create_dir_all(repo_root.path().join(".git"))?;
    std::fs::create_dir_all(&nested_cwd)?;

    let workspace_key = repo_root.path().to_string_lossy().replace('\\', "\\\\");
    std::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        format!(
            r#"[projects."{workspace_key}"]
trust_level = "trusted"
"#
        ),
    )?;

    let root_agent = repo_root
        .path()
        .join(".codex")
        .join("agents")
        .join("root.toml");
    std::fs::create_dir_all(
        root_agent
            .parent()
            .expect("root agent should have a parent directory"),
    )?;
    std::fs::write(
        &root_agent,
        r#"
name = "researcher"
description = "from root"
developer_instructions = "Research carefully"
"#,
    )?;

    let nested_agent = repo_root
        .path()
        .join("packages")
        .join(".codex")
        .join("agents")
        .join("review")
        .join("nested.toml");
    std::fs::create_dir_all(
        nested_agent
            .parent()
            .expect("nested agent should have a parent directory"),
    )?;
    std::fs::write(
        &nested_agent,
        r#"
name = "reviewer"
description = "from nested"
nickname_candidates = ["Atlas"]
developer_instructions = "Review carefully"
"#,
    )?;

    let sibling_agent = repo_root
        .path()
        .join("packages")
        .join(".codex")
        .join("agents")
        .join("writer.toml");
    std::fs::create_dir_all(
        sibling_agent
            .parent()
            .expect("sibling agent should have a parent directory"),
    )?;
    std::fs::write(
        &sibling_agent,
        r#"
name = "writer"
description = "from sibling"
nickname_candidates = ["Sagan"]
developer_instructions = "Write carefully"
"#,
    )?;

    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .harness_overrides(ConfigOverrides {
            cwd: Some(nested_cwd),
            ..Default::default()
        })
        .build()
        .await?;

    assert_eq!(
        config
            .agent_roles
            .get("researcher")
            .and_then(|role| role.description.as_deref()),
        Some("from root")
    );
    assert_eq!(
        config
            .agent_roles
            .get("reviewer")
            .and_then(|role| role.description.as_deref()),
        Some("from nested")
    );
    assert_eq!(
        config
            .agent_roles
            .get("reviewer")
            .and_then(|role| role.nickname_candidates.as_ref())
            .map(|candidates| candidates.iter().map(String::as_str).collect::<Vec<_>>()),
        Some(vec!["Atlas"])
    );
    assert_eq!(
        config
            .agent_roles
            .get("writer")
            .and_then(|role| role.description.as_deref()),
        Some("from sibling")
    );
    assert_eq!(
        config
            .agent_roles
            .get("writer")
            .and_then(|role| role.nickname_candidates.as_ref())
            .map(|candidates| candidates.iter().map(String::as_str).collect::<Vec<_>>()),
        Some(vec!["Sagan"])
    );

    Ok(())
}

#[tokio::test]
async fn mixed_legacy_and_standalone_agent_role_sources_merge_with_precedence()
-> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let repo_root = TempDir::new()?;
    let nested_cwd = repo_root.path().join("packages").join("app");
    std::fs::create_dir_all(repo_root.path().join(".git"))?;
    std::fs::create_dir_all(&nested_cwd)?;

    let workspace_key = repo_root.path().to_string_lossy().replace('\\', "\\\\");
    tokio::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        format!(
            r#"[projects."{workspace_key}"]
trust_level = "trusted"

[agents.researcher]
description = "Research role from config"
config_file = "./agents/researcher.toml"
nickname_candidates = ["Noether"]

[agents.critic]
description = "Critic role from config"
config_file = "./agents/critic.toml"
nickname_candidates = ["Ada"]
"#
        ),
    )
    .await?;

    let home_agents_dir = codex_home.path().join("agents");
    tokio::fs::create_dir_all(&home_agents_dir).await?;
    tokio::fs::write(
        home_agents_dir.join("researcher.toml"),
        r#"
developer_instructions = "Research carefully"
model = "gpt-5"
"#,
    )
    .await?;
    tokio::fs::write(
        home_agents_dir.join("critic.toml"),
        r#"
developer_instructions = "Critique carefully"
model = "gpt-4.1"
"#,
    )
    .await?;

    let standalone_agents_dir = repo_root.path().join(".codex").join("agents");
    tokio::fs::create_dir_all(&standalone_agents_dir).await?;
    tokio::fs::write(
        standalone_agents_dir.join("researcher.toml"),
        r#"
name = "researcher"
description = "Research role from file"
nickname_candidates = ["Hypatia"]
developer_instructions = "Research from file"
model = "gpt-5-mini"
"#,
    )
    .await?;
    tokio::fs::write(
        standalone_agents_dir.join("writer.toml"),
        r#"
name = "writer"
description = "Writer role from file"
nickname_candidates = ["Sagan"]
developer_instructions = "Write carefully"
model = "gpt-5"
"#,
    )
    .await?;

    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .harness_overrides(ConfigOverrides {
            cwd: Some(nested_cwd),
            ..Default::default()
        })
        .build()
        .await?;

    assert_eq!(
        config
            .agent_roles
            .get("researcher")
            .and_then(|role| role.description.as_deref()),
        Some("Research role from file")
    );
    assert_eq!(
        config
            .agent_roles
            .get("researcher")
            .and_then(|role| role.config_file.as_ref()),
        Some(&standalone_agents_dir.join("researcher.toml"))
    );
    assert_eq!(
        config
            .agent_roles
            .get("researcher")
            .and_then(|role| role.nickname_candidates.as_ref())
            .map(|candidates| candidates.iter().map(String::as_str).collect::<Vec<_>>()),
        Some(vec!["Hypatia"])
    );
    assert_eq!(
        config
            .agent_roles
            .get("critic")
            .and_then(|role| role.description.as_deref()),
        Some("Critic role from config")
    );
    assert_eq!(
        config
            .agent_roles
            .get("critic")
            .and_then(|role| role.config_file.as_ref()),
        Some(&home_agents_dir.join("critic.toml"))
    );
    assert_eq!(
        config
            .agent_roles
            .get("critic")
            .and_then(|role| role.nickname_candidates.as_ref())
            .map(|candidates| candidates.iter().map(String::as_str).collect::<Vec<_>>()),
        Some(vec!["Ada"])
    );
    assert_eq!(
        config
            .agent_roles
            .get("writer")
            .and_then(|role| role.description.as_deref()),
        Some("Writer role from file")
    );
    assert_eq!(
        config
            .agent_roles
            .get("writer")
            .and_then(|role| role.nickname_candidates.as_ref())
            .map(|candidates| candidates.iter().map(String::as_str).collect::<Vec<_>>()),
        Some(vec!["Sagan"])
    );

    Ok(())
}

#[tokio::test]
async fn higher_precedence_agent_role_can_inherit_description_from_lower_layer()
-> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let repo_root = TempDir::new()?;
    let nested_cwd = repo_root.path().join("packages").join("app");
    std::fs::create_dir_all(repo_root.path().join(".git"))?;
    std::fs::create_dir_all(&nested_cwd)?;

    let workspace_key = repo_root.path().to_string_lossy().replace('\\', "\\\\");
    tokio::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        format!(
            r#"[projects."{workspace_key}"]
trust_level = "trusted"

[agents.researcher]
description = "Research role from config"
config_file = "./agents/researcher.toml"
"#
        ),
    )
    .await?;

    let home_agents_dir = codex_home.path().join("agents");
    tokio::fs::create_dir_all(&home_agents_dir).await?;
    tokio::fs::write(
        home_agents_dir.join("researcher.toml"),
        r#"
developer_instructions = "Research carefully"
model = "gpt-5"
"#,
    )
    .await?;

    let standalone_agents_dir = repo_root.path().join(".codex").join("agents");
    tokio::fs::create_dir_all(&standalone_agents_dir).await?;
    tokio::fs::write(
        standalone_agents_dir.join("researcher.toml"),
        r#"
name = "researcher"
nickname_candidates = ["Hypatia"]
developer_instructions = "Research from file"
model = "gpt-5-mini"
"#,
    )
    .await?;

    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .harness_overrides(ConfigOverrides {
            cwd: Some(nested_cwd),
            ..Default::default()
        })
        .build()
        .await?;

    assert_eq!(
        config
            .agent_roles
            .get("researcher")
            .and_then(|role| role.description.as_deref()),
        Some("Research role from config")
    );
    assert_eq!(
        config
            .agent_roles
            .get("researcher")
            .and_then(|role| role.config_file.as_ref()),
        Some(&standalone_agents_dir.join("researcher.toml"))
    );
    assert_eq!(
        config
            .agent_roles
            .get("researcher")
            .and_then(|role| role.nickname_candidates.as_ref())
            .map(|candidates| candidates.iter().map(String::as_str).collect::<Vec<_>>()),
        Some(vec!["Hypatia"])
    );

    Ok(())
}

#[test]
fn load_config_normalizes_agent_role_nickname_candidates() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let cfg = ConfigToml {
        agents: Some(AgentsToml {
            max_threads: None,
            max_depth: None,
            job_max_runtime_seconds: None,
            roles: BTreeMap::from([(
                "researcher".to_string(),
                AgentRoleToml {
                    description: Some("Research role".to_string()),
                    config_file: None,
                    nickname_candidates: Some(vec![
                        "  Hypatia  ".to_string(),
                        "Noether".to_string(),
                    ]),
                },
            )]),
        }),
        ..Default::default()
    };

    let config = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    )?;

    assert_eq!(
        config
            .agent_roles
            .get("researcher")
            .and_then(|role| role.nickname_candidates.as_ref())
            .map(|candidates| candidates.iter().map(String::as_str).collect::<Vec<_>>()),
        Some(vec!["Hypatia", "Noether"])
    );

    Ok(())
}

#[test]
fn load_config_rejects_empty_agent_role_nickname_candidates() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let cfg = ConfigToml {
        agents: Some(AgentsToml {
            max_threads: None,
            max_depth: None,
            job_max_runtime_seconds: None,
            roles: BTreeMap::from([(
                "researcher".to_string(),
                AgentRoleToml {
                    description: Some("Research role".to_string()),
                    config_file: None,
                    nickname_candidates: Some(Vec::new()),
                },
            )]),
        }),
        ..Default::default()
    };

    let result = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    );
    let err = result.expect_err("empty nickname candidates should be rejected");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(
        err.to_string()
            .contains("agents.researcher.nickname_candidates")
    );

    Ok(())
}

#[test]
fn load_config_rejects_duplicate_agent_role_nickname_candidates() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let cfg = ConfigToml {
        agents: Some(AgentsToml {
            max_threads: None,
            max_depth: None,
            job_max_runtime_seconds: None,
            roles: BTreeMap::from([(
                "researcher".to_string(),
                AgentRoleToml {
                    description: Some("Research role".to_string()),
                    config_file: None,
                    nickname_candidates: Some(vec!["Hypatia".to_string(), " Hypatia ".to_string()]),
                },
            )]),
        }),
        ..Default::default()
    };

    let result = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    );
    let err = result.expect_err("duplicate nickname candidates should be rejected");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(
        err.to_string()
            .contains("agents.researcher.nickname_candidates cannot contain duplicates")
    );

    Ok(())
}

#[test]
fn load_config_rejects_unsafe_agent_role_nickname_candidates() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let cfg = ConfigToml {
        agents: Some(AgentsToml {
            max_threads: None,
            max_depth: None,
            job_max_runtime_seconds: None,
            roles: BTreeMap::from([(
                "researcher".to_string(),
                AgentRoleToml {
                    description: Some("Research role".to_string()),
                    config_file: None,
                    nickname_candidates: Some(vec!["Agent <One>".to_string()]),
                },
            )]),
        }),
        ..Default::default()
    };

    let result = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    );
    let err = result.expect_err("unsafe nickname candidates should be rejected");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(err.to_string().contains(
            "agents.researcher.nickname_candidates may only contain ASCII letters, digits, spaces, hyphens, and underscores"
        ));

    Ok(())
}

#[test]
fn model_catalog_json_loads_from_path() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let catalog_path = codex_home.path().join("catalog.json");
    let mut catalog: ModelsResponse =
        serde_json::from_str(include_str!("../../models.json")).expect("valid models.json");
    catalog.models = catalog.models.into_iter().take(1).collect();
    std::fs::write(
        &catalog_path,
        serde_json::to_string(&catalog).expect("serialize catalog"),
    )?;

    let cfg = ConfigToml {
        model_catalog_json: Some(AbsolutePathBuf::from_absolute_path(catalog_path)?),
        ..Default::default()
    };

    let config = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    )?;

    assert_eq!(config.model_catalog, Some(catalog));
    Ok(())
}

#[test]
fn model_catalog_json_rejects_empty_catalog() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let catalog_path = codex_home.path().join("catalog.json");
    std::fs::write(&catalog_path, r#"{"models":[]}"#)?;

    let cfg = ConfigToml {
        model_catalog_json: Some(AbsolutePathBuf::from_absolute_path(catalog_path)?),
        ..Default::default()
    };

    let err = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    )
    .expect_err("empty custom catalog should fail config load");

    assert_eq!(err.kind(), ErrorKind::InvalidData);
    assert!(
        err.to_string().contains("must contain at least one model"),
        "unexpected error: {err}"
    );
    Ok(())
}

fn create_test_fixture() -> std::io::Result<PrecedenceTestFixture> {
    let toml = r#"
model = "o3"
approval_policy = "untrusted"

# Can be used to determine which profile to use if not specified by
# `ConfigOverrides`.
profile = "gpt3"

[analytics]
enabled = true

[model_providers.openai-custom]
name = "OpenAI custom"
base_url = "https://api.openai.com/v1"
env_key = "OPENAI_API_KEY"
wire_api = "responses"
request_max_retries = 4            # retry failed HTTP requests
stream_max_retries = 10            # retry dropped SSE streams
stream_idle_timeout_ms = 300000    # 5m idle timeout

[profiles.o3]
model = "o3"
model_provider = "openai"
approval_policy = "never"
model_reasoning_effort = "high"
model_reasoning_summary = "detailed"

[profiles.gpt3]
model = "gpt-3.5-turbo"
model_provider = "openai-custom"

[profiles.zdr]
model = "o3"
model_provider = "openai"
approval_policy = "on-failure"

[profiles.zdr.analytics]
enabled = false

[profiles.gpt5]
model = "gpt-5.1"
model_provider = "openai"
approval_policy = "on-failure"
model_reasoning_effort = "high"
model_reasoning_summary = "detailed"
model_verbosity = "high"
"#;

    let cfg: ConfigToml = toml::from_str(toml).expect("TOML deserialization should succeed");

    // Use a temporary directory for the cwd so it does not contain an
    // AGENTS.md file.
    let cwd_temp_dir = TempDir::new().unwrap();
    let cwd = cwd_temp_dir.path().to_path_buf();
    // Make it look like a Git repo so it does not search for AGENTS.md in
    // a parent folder, either.
    std::fs::write(cwd.join(".git"), "gitdir: nowhere")?;

    let codex_home_temp_dir = TempDir::new().unwrap();

    let openai_custom_provider = ModelProviderInfo {
        name: "OpenAI custom".to_string(),
        base_url: Some("https://api.openai.com/v1".to_string()),
        env_key: Some("OPENAI_API_KEY".to_string()),
        wire_api: crate::WireApi::Responses,
        env_key_instructions: None,
        experimental_bearer_token: None,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: Some(4),
        stream_max_retries: Some(10),
        stream_idle_timeout_ms: Some(300_000),
        requires_openai_auth: false,
        supports_websockets: false,
    };
    let model_provider_map = {
        let mut model_provider_map = built_in_model_providers();
        model_provider_map.insert("openai-custom".to_string(), openai_custom_provider.clone());
        model_provider_map
    };

    let openai_provider = model_provider_map
        .get("openai")
        .expect("openai provider should exist")
        .clone();

    Ok(PrecedenceTestFixture {
        cwd: cwd_temp_dir,
        codex_home: codex_home_temp_dir,
        cfg,
        model_provider_map,
        openai_provider,
        openai_custom_provider,
    })
}

/// Users can specify config values at multiple levels that have the
/// following precedence:
///
/// 1. custom command-line argument, e.g. `--model o3`
/// 2. as part of a profile, where the `--profile` is specified via a CLI
///    (or in the config file itself)
/// 3. as an entry in `config.toml`, e.g. `model = "o3"`
/// 4. the default value for a required field defined in code, e.g.,
///    `crate::flags::OPENAI_DEFAULT_MODEL`
///
/// Note that profiles are the recommended way to specify a group of
/// configuration options together.
#[test]
fn test_precedence_fixture_with_o3_profile() -> std::io::Result<()> {
    let fixture = create_test_fixture()?;

    let o3_profile_overrides = ConfigOverrides {
        config_profile: Some("o3".to_string()),
        cwd: Some(fixture.cwd()),
        ..Default::default()
    };
    let o3_profile_config: Config = Config::load_from_base_config_with_overrides(
        fixture.cfg.clone(),
        o3_profile_overrides,
        fixture.codex_home(),
    )?;
    assert_eq!(
        Config {
            model: Some("o3".to_string()),
            review_model: None,
            model_context_window: None,
            model_auto_compact_token_limit: None,
            service_tier: None,
            model_provider_id: "openai".to_string(),
            model_provider: fixture.openai_provider.clone(),
            permissions: Permissions {
                approval_policy: Constrained::allow_any(AskForApproval::Never),
                sandbox_policy: Constrained::allow_any(SandboxPolicy::new_read_only_policy()),
                file_system_sandbox_policy: FileSystemSandboxPolicy::from(
                    &SandboxPolicy::new_read_only_policy(),
                ),
                network_sandbox_policy: NetworkSandboxPolicy::Restricted,
                network: None,
                allow_login_shell: true,
                shell_environment_policy: ShellEnvironmentPolicy::default(),
                windows_sandbox_mode: None,
                windows_sandbox_private_desktop: true,
                macos_seatbelt_profile_extensions: None,
            },
            approval_review_policy: ApprovalReviewPolicy::ManualOnly,
            enforce_residency: Constrained::allow_any(None),
            user_instructions: None,
            notify: None,
            cwd: fixture.cwd(),
            cli_auth_credentials_store_mode: Default::default(),
            mcp_servers: Constrained::allow_any(HashMap::new()),
            mcp_oauth_credentials_store_mode: Default::default(),
            mcp_oauth_callback_port: None,
            mcp_oauth_callback_url: None,
            model_providers: fixture.model_provider_map.clone(),
            project_doc_max_bytes: PROJECT_DOC_MAX_BYTES,
            project_doc_fallback_filenames: Vec::new(),
            tool_output_token_limit: None,
            agent_max_threads: DEFAULT_AGENT_MAX_THREADS,
            agent_max_depth: DEFAULT_AGENT_MAX_DEPTH,
            agent_roles: BTreeMap::new(),
            memories: MemoriesConfig::default(),
            agent_job_max_runtime_seconds: DEFAULT_AGENT_JOB_MAX_RUNTIME_SECONDS,
            codex_home: fixture.codex_home(),
            sqlite_home: fixture.codex_home(),
            log_dir: fixture.codex_home().join("log"),
            config_layer_stack: Default::default(),
            startup_warnings: Vec::new(),
            history: History::default(),
            ephemeral: false,
            file_opener: UriBasedFileOpener::VsCode,
            codex_linux_sandbox_exe: None,
            main_execve_wrapper_exe: None,
            js_repl_node_path: None,
            js_repl_node_module_dirs: Vec::new(),
            zsh_path: None,
            hide_agent_reasoning: false,
            show_raw_agent_reasoning: false,
            model_reasoning_effort: Some(ReasoningEffort::High),
            plan_mode_reasoning_effort: None,
            model_reasoning_summary: Some(ReasoningSummary::Detailed),
            model_supports_reasoning_summaries: None,
            model_catalog: None,
            model_verbosity: None,
            personality: Some(Personality::Pragmatic),
            chatgpt_base_url: "https://chatgpt.com/backend-api/".to_string(),
            realtime_audio: RealtimeAudioConfig::default(),
            experimental_realtime_start_instructions: None,
            experimental_realtime_ws_base_url: None,
            experimental_realtime_ws_model: None,
            realtime: RealtimeConfig::default(),
            experimental_realtime_ws_backend_prompt: None,
            experimental_realtime_ws_startup_context: None,
            base_instructions: None,
            developer_instructions: None,
            compact_prompt: None,
            commit_attribution: None,
            forced_chatgpt_workspace_id: None,
            forced_login_method: None,
            include_apply_patch_tool: false,
            web_search_mode: Constrained::allow_any(WebSearchMode::Cached),
            web_search_config: None,
            use_experimental_unified_exec_tool: !cfg!(windows),
            background_terminal_max_timeout: DEFAULT_MAX_BACKGROUND_TERMINAL_TIMEOUT_MS,
            ghost_snapshot: GhostSnapshotConfig::default(),
            features: Features::with_defaults().into(),
            suppress_unstable_features_warning: false,
            active_profile: Some("o3".to_string()),
            active_project: ProjectConfig { trust_level: None },
            windows_wsl_setup_acknowledged: false,
            notices: Default::default(),
            check_for_update_on_startup: true,
            disable_paste_burst: false,
            tui_notifications: Default::default(),
            tui_notification_method: Default::default(),
            animations: true,
            show_tooltips: true,
            model_availability_nux: ModelAvailabilityNuxConfig::default(),
            analytics_enabled: Some(true),
            feedback_enabled: true,
            tui_alternate_screen: AltScreenMode::Auto,
            tui_status_line: None,
            tui_theme: None,
            otel: OtelConfig::default(),
        },
        o3_profile_config
    );
    Ok(())
}

#[test]
fn metrics_exporter_defaults_to_statsig_when_missing() -> std::io::Result<()> {
    let fixture = create_test_fixture()?;

    let config = Config::load_from_base_config_with_overrides(
        fixture.cfg.clone(),
        ConfigOverrides {
            cwd: Some(fixture.cwd()),
            ..Default::default()
        },
        fixture.codex_home(),
    )?;

    assert_eq!(config.otel.metrics_exporter, OtelExporterKind::Statsig);
    Ok(())
}

#[test]
fn test_precedence_fixture_with_gpt3_profile() -> std::io::Result<()> {
    let fixture = create_test_fixture()?;

    let gpt3_profile_overrides = ConfigOverrides {
        config_profile: Some("gpt3".to_string()),
        cwd: Some(fixture.cwd()),
        ..Default::default()
    };
    let gpt3_profile_config = Config::load_from_base_config_with_overrides(
        fixture.cfg.clone(),
        gpt3_profile_overrides,
        fixture.codex_home(),
    )?;
    let expected_gpt3_profile_config = Config {
        model: Some("gpt-3.5-turbo".to_string()),
        review_model: None,
        model_context_window: None,
        model_auto_compact_token_limit: None,
        service_tier: None,
        model_provider_id: "openai-custom".to_string(),
        model_provider: fixture.openai_custom_provider.clone(),
        permissions: Permissions {
            approval_policy: Constrained::allow_any(AskForApproval::UnlessTrusted),
            sandbox_policy: Constrained::allow_any(SandboxPolicy::new_read_only_policy()),
            file_system_sandbox_policy: FileSystemSandboxPolicy::from(
                &SandboxPolicy::new_read_only_policy(),
            ),
            network_sandbox_policy: NetworkSandboxPolicy::Restricted,
            network: None,
            allow_login_shell: true,
            shell_environment_policy: ShellEnvironmentPolicy::default(),
            windows_sandbox_mode: None,
            windows_sandbox_private_desktop: true,
            macos_seatbelt_profile_extensions: None,
        },
        approval_review_policy: ApprovalReviewPolicy::ManualOnly,
        enforce_residency: Constrained::allow_any(None),
        user_instructions: None,
        notify: None,
        cwd: fixture.cwd(),
        cli_auth_credentials_store_mode: Default::default(),
        mcp_servers: Constrained::allow_any(HashMap::new()),
        mcp_oauth_credentials_store_mode: Default::default(),
        mcp_oauth_callback_port: None,
        mcp_oauth_callback_url: None,
        model_providers: fixture.model_provider_map.clone(),
        project_doc_max_bytes: PROJECT_DOC_MAX_BYTES,
        project_doc_fallback_filenames: Vec::new(),
        tool_output_token_limit: None,
        agent_max_threads: DEFAULT_AGENT_MAX_THREADS,
        agent_max_depth: DEFAULT_AGENT_MAX_DEPTH,
        agent_roles: BTreeMap::new(),
        memories: MemoriesConfig::default(),
        agent_job_max_runtime_seconds: DEFAULT_AGENT_JOB_MAX_RUNTIME_SECONDS,
        codex_home: fixture.codex_home(),
        sqlite_home: fixture.codex_home(),
        log_dir: fixture.codex_home().join("log"),
        config_layer_stack: Default::default(),
        startup_warnings: Vec::new(),
        history: History::default(),
        ephemeral: false,
        file_opener: UriBasedFileOpener::VsCode,
        codex_linux_sandbox_exe: None,
        main_execve_wrapper_exe: None,
        js_repl_node_path: None,
        js_repl_node_module_dirs: Vec::new(),
        zsh_path: None,
        hide_agent_reasoning: false,
        show_raw_agent_reasoning: false,
        model_reasoning_effort: None,
        plan_mode_reasoning_effort: None,
        model_reasoning_summary: None,
        model_supports_reasoning_summaries: None,
        model_catalog: None,
        model_verbosity: None,
        personality: Some(Personality::Pragmatic),
        chatgpt_base_url: "https://chatgpt.com/backend-api/".to_string(),
        realtime_audio: RealtimeAudioConfig::default(),
        experimental_realtime_start_instructions: None,
        experimental_realtime_ws_base_url: None,
        experimental_realtime_ws_model: None,
        realtime: RealtimeConfig::default(),
        experimental_realtime_ws_backend_prompt: None,
        experimental_realtime_ws_startup_context: None,
        base_instructions: None,
        developer_instructions: None,
        compact_prompt: None,
        commit_attribution: None,
        forced_chatgpt_workspace_id: None,
        forced_login_method: None,
        include_apply_patch_tool: false,
        web_search_mode: Constrained::allow_any(WebSearchMode::Cached),
        web_search_config: None,
        use_experimental_unified_exec_tool: !cfg!(windows),
        background_terminal_max_timeout: DEFAULT_MAX_BACKGROUND_TERMINAL_TIMEOUT_MS,
        ghost_snapshot: GhostSnapshotConfig::default(),
        features: Features::with_defaults().into(),
        suppress_unstable_features_warning: false,
        active_profile: Some("gpt3".to_string()),
        active_project: ProjectConfig { trust_level: None },
        windows_wsl_setup_acknowledged: false,
        notices: Default::default(),
        check_for_update_on_startup: true,
        disable_paste_burst: false,
        tui_notifications: Default::default(),
        tui_notification_method: Default::default(),
        animations: true,
        show_tooltips: true,
        model_availability_nux: ModelAvailabilityNuxConfig::default(),
        analytics_enabled: Some(true),
        feedback_enabled: true,
        tui_alternate_screen: AltScreenMode::Auto,
        tui_status_line: None,
        tui_theme: None,
        otel: OtelConfig::default(),
    };

    assert_eq!(expected_gpt3_profile_config, gpt3_profile_config);

    // Verify that loading without specifying a profile in ConfigOverrides
    // uses the default profile from the config file (which is "gpt3").
    let default_profile_overrides = ConfigOverrides {
        cwd: Some(fixture.cwd()),
        ..Default::default()
    };

    let default_profile_config = Config::load_from_base_config_with_overrides(
        fixture.cfg.clone(),
        default_profile_overrides,
        fixture.codex_home(),
    )?;

    assert_eq!(expected_gpt3_profile_config, default_profile_config);
    Ok(())
}

#[test]
fn test_precedence_fixture_with_zdr_profile() -> std::io::Result<()> {
    let fixture = create_test_fixture()?;

    let zdr_profile_overrides = ConfigOverrides {
        config_profile: Some("zdr".to_string()),
        cwd: Some(fixture.cwd()),
        ..Default::default()
    };
    let zdr_profile_config = Config::load_from_base_config_with_overrides(
        fixture.cfg.clone(),
        zdr_profile_overrides,
        fixture.codex_home(),
    )?;
    let expected_zdr_profile_config = Config {
        model: Some("o3".to_string()),
        review_model: None,
        model_context_window: None,
        model_auto_compact_token_limit: None,
        service_tier: None,
        model_provider_id: "openai".to_string(),
        model_provider: fixture.openai_provider.clone(),
        permissions: Permissions {
            approval_policy: Constrained::allow_any(AskForApproval::OnFailure),
            sandbox_policy: Constrained::allow_any(SandboxPolicy::new_read_only_policy()),
            file_system_sandbox_policy: FileSystemSandboxPolicy::from(
                &SandboxPolicy::new_read_only_policy(),
            ),
            network_sandbox_policy: NetworkSandboxPolicy::Restricted,
            network: None,
            allow_login_shell: true,
            shell_environment_policy: ShellEnvironmentPolicy::default(),
            windows_sandbox_mode: None,
            windows_sandbox_private_desktop: true,
            macos_seatbelt_profile_extensions: None,
        },
        approval_review_policy: ApprovalReviewPolicy::ManualOnly,
        enforce_residency: Constrained::allow_any(None),
        user_instructions: None,
        notify: None,
        cwd: fixture.cwd(),
        cli_auth_credentials_store_mode: Default::default(),
        mcp_servers: Constrained::allow_any(HashMap::new()),
        mcp_oauth_credentials_store_mode: Default::default(),
        mcp_oauth_callback_port: None,
        mcp_oauth_callback_url: None,
        model_providers: fixture.model_provider_map.clone(),
        project_doc_max_bytes: PROJECT_DOC_MAX_BYTES,
        project_doc_fallback_filenames: Vec::new(),
        tool_output_token_limit: None,
        agent_max_threads: DEFAULT_AGENT_MAX_THREADS,
        agent_max_depth: DEFAULT_AGENT_MAX_DEPTH,
        agent_roles: BTreeMap::new(),
        memories: MemoriesConfig::default(),
        agent_job_max_runtime_seconds: DEFAULT_AGENT_JOB_MAX_RUNTIME_SECONDS,
        codex_home: fixture.codex_home(),
        sqlite_home: fixture.codex_home(),
        log_dir: fixture.codex_home().join("log"),
        config_layer_stack: Default::default(),
        startup_warnings: Vec::new(),
        history: History::default(),
        ephemeral: false,
        file_opener: UriBasedFileOpener::VsCode,
        codex_linux_sandbox_exe: None,
        main_execve_wrapper_exe: None,
        js_repl_node_path: None,
        js_repl_node_module_dirs: Vec::new(),
        zsh_path: None,
        hide_agent_reasoning: false,
        show_raw_agent_reasoning: false,
        model_reasoning_effort: None,
        plan_mode_reasoning_effort: None,
        model_reasoning_summary: None,
        model_supports_reasoning_summaries: None,
        model_catalog: None,
        model_verbosity: None,
        personality: Some(Personality::Pragmatic),
        chatgpt_base_url: "https://chatgpt.com/backend-api/".to_string(),
        realtime_audio: RealtimeAudioConfig::default(),
        experimental_realtime_start_instructions: None,
        experimental_realtime_ws_base_url: None,
        experimental_realtime_ws_model: None,
        realtime: RealtimeConfig::default(),
        experimental_realtime_ws_backend_prompt: None,
        experimental_realtime_ws_startup_context: None,
        base_instructions: None,
        developer_instructions: None,
        compact_prompt: None,
        commit_attribution: None,
        forced_chatgpt_workspace_id: None,
        forced_login_method: None,
        include_apply_patch_tool: false,
        web_search_mode: Constrained::allow_any(WebSearchMode::Cached),
        web_search_config: None,
        use_experimental_unified_exec_tool: !cfg!(windows),
        background_terminal_max_timeout: DEFAULT_MAX_BACKGROUND_TERMINAL_TIMEOUT_MS,
        ghost_snapshot: GhostSnapshotConfig::default(),
        features: Features::with_defaults().into(),
        suppress_unstable_features_warning: false,
        active_profile: Some("zdr".to_string()),
        active_project: ProjectConfig { trust_level: None },
        windows_wsl_setup_acknowledged: false,
        notices: Default::default(),
        check_for_update_on_startup: true,
        disable_paste_burst: false,
        tui_notifications: Default::default(),
        tui_notification_method: Default::default(),
        animations: true,
        show_tooltips: true,
        model_availability_nux: ModelAvailabilityNuxConfig::default(),
        analytics_enabled: Some(false),
        feedback_enabled: true,
        tui_alternate_screen: AltScreenMode::Auto,
        tui_status_line: None,
        tui_theme: None,
        otel: OtelConfig::default(),
    };

    assert_eq!(expected_zdr_profile_config, zdr_profile_config);

    Ok(())
}

#[test]
fn test_precedence_fixture_with_gpt5_profile() -> std::io::Result<()> {
    let fixture = create_test_fixture()?;

    let gpt5_profile_overrides = ConfigOverrides {
        config_profile: Some("gpt5".to_string()),
        cwd: Some(fixture.cwd()),
        ..Default::default()
    };
    let gpt5_profile_config = Config::load_from_base_config_with_overrides(
        fixture.cfg.clone(),
        gpt5_profile_overrides,
        fixture.codex_home(),
    )?;
    let expected_gpt5_profile_config = Config {
        model: Some("gpt-5.1".to_string()),
        review_model: None,
        model_context_window: None,
        model_auto_compact_token_limit: None,
        service_tier: None,
        model_provider_id: "openai".to_string(),
        model_provider: fixture.openai_provider.clone(),
        permissions: Permissions {
            approval_policy: Constrained::allow_any(AskForApproval::OnFailure),
            sandbox_policy: Constrained::allow_any(SandboxPolicy::new_read_only_policy()),
            file_system_sandbox_policy: FileSystemSandboxPolicy::from(
                &SandboxPolicy::new_read_only_policy(),
            ),
            network_sandbox_policy: NetworkSandboxPolicy::Restricted,
            network: None,
            allow_login_shell: true,
            shell_environment_policy: ShellEnvironmentPolicy::default(),
            windows_sandbox_mode: None,
            windows_sandbox_private_desktop: true,
            macos_seatbelt_profile_extensions: None,
        },
        approval_review_policy: ApprovalReviewPolicy::ManualOnly,
        enforce_residency: Constrained::allow_any(None),
        user_instructions: None,
        notify: None,
        cwd: fixture.cwd(),
        cli_auth_credentials_store_mode: Default::default(),
        mcp_servers: Constrained::allow_any(HashMap::new()),
        mcp_oauth_credentials_store_mode: Default::default(),
        mcp_oauth_callback_port: None,
        mcp_oauth_callback_url: None,
        model_providers: fixture.model_provider_map.clone(),
        project_doc_max_bytes: PROJECT_DOC_MAX_BYTES,
        project_doc_fallback_filenames: Vec::new(),
        tool_output_token_limit: None,
        agent_max_threads: DEFAULT_AGENT_MAX_THREADS,
        agent_max_depth: DEFAULT_AGENT_MAX_DEPTH,
        agent_roles: BTreeMap::new(),
        memories: MemoriesConfig::default(),
        agent_job_max_runtime_seconds: DEFAULT_AGENT_JOB_MAX_RUNTIME_SECONDS,
        codex_home: fixture.codex_home(),
        sqlite_home: fixture.codex_home(),
        log_dir: fixture.codex_home().join("log"),
        config_layer_stack: Default::default(),
        startup_warnings: Vec::new(),
        history: History::default(),
        ephemeral: false,
        file_opener: UriBasedFileOpener::VsCode,
        codex_linux_sandbox_exe: None,
        main_execve_wrapper_exe: None,
        js_repl_node_path: None,
        js_repl_node_module_dirs: Vec::new(),
        zsh_path: None,
        hide_agent_reasoning: false,
        show_raw_agent_reasoning: false,
        model_reasoning_effort: Some(ReasoningEffort::High),
        plan_mode_reasoning_effort: None,
        model_reasoning_summary: Some(ReasoningSummary::Detailed),
        model_supports_reasoning_summaries: None,
        model_catalog: None,
        model_verbosity: Some(Verbosity::High),
        personality: Some(Personality::Pragmatic),
        chatgpt_base_url: "https://chatgpt.com/backend-api/".to_string(),
        realtime_audio: RealtimeAudioConfig::default(),
        experimental_realtime_start_instructions: None,
        experimental_realtime_ws_base_url: None,
        experimental_realtime_ws_model: None,
        realtime: RealtimeConfig::default(),
        experimental_realtime_ws_backend_prompt: None,
        experimental_realtime_ws_startup_context: None,
        base_instructions: None,
        developer_instructions: None,
        compact_prompt: None,
        commit_attribution: None,
        forced_chatgpt_workspace_id: None,
        forced_login_method: None,
        include_apply_patch_tool: false,
        web_search_mode: Constrained::allow_any(WebSearchMode::Cached),
        web_search_config: None,
        use_experimental_unified_exec_tool: !cfg!(windows),
        background_terminal_max_timeout: DEFAULT_MAX_BACKGROUND_TERMINAL_TIMEOUT_MS,
        ghost_snapshot: GhostSnapshotConfig::default(),
        features: Features::with_defaults().into(),
        suppress_unstable_features_warning: false,
        active_profile: Some("gpt5".to_string()),
        active_project: ProjectConfig { trust_level: None },
        windows_wsl_setup_acknowledged: false,
        notices: Default::default(),
        check_for_update_on_startup: true,
        disable_paste_burst: false,
        tui_notifications: Default::default(),
        tui_notification_method: Default::default(),
        animations: true,
        show_tooltips: true,
        model_availability_nux: ModelAvailabilityNuxConfig::default(),
        analytics_enabled: Some(true),
        feedback_enabled: true,
        tui_alternate_screen: AltScreenMode::Auto,
        tui_status_line: None,
        tui_theme: None,
        otel: OtelConfig::default(),
    };

    assert_eq!(expected_gpt5_profile_config, gpt5_profile_config);

    Ok(())
}

#[test]
fn test_requirements_web_search_mode_allowlist_does_not_warn_when_unset() -> anyhow::Result<()> {
    let fixture = create_test_fixture()?;

    let requirements_toml = crate::config_loader::ConfigRequirementsToml {
        allowed_approval_policies: None,
        allowed_sandbox_modes: None,
        allowed_web_search_modes: Some(vec![
            crate::config_loader::WebSearchModeRequirement::Cached,
        ]),
        feature_requirements: None,
        mcp_servers: None,
        apps: None,
        rules: None,
        enforce_residency: None,
        network: None,
    };
    let requirement_source = crate::config_loader::RequirementSource::Unknown;
    let requirement_source_for_error = requirement_source.clone();
    let allowed = vec![WebSearchMode::Disabled, WebSearchMode::Cached];
    let constrained = Constrained::new(WebSearchMode::Cached, move |candidate| {
        if matches!(candidate, WebSearchMode::Cached | WebSearchMode::Disabled) {
            Ok(())
        } else {
            Err(ConstraintError::InvalidValue {
                field_name: "web_search_mode",
                candidate: format!("{candidate:?}"),
                allowed: format!("{allowed:?}"),
                requirement_source: requirement_source_for_error.clone(),
            })
        }
    })?;
    let requirements = crate::config_loader::ConfigRequirements {
        web_search_mode: crate::config_loader::ConstrainedWithSource::new(
            constrained,
            Some(requirement_source),
        ),
        ..Default::default()
    };
    let config_layer_stack =
        crate::config_loader::ConfigLayerStack::new(Vec::new(), requirements, requirements_toml)
            .expect("config layer stack");

    let config = Config::load_config_with_layer_stack(
        fixture.cfg.clone(),
        ConfigOverrides {
            cwd: Some(fixture.cwd()),
            ..Default::default()
        },
        fixture.codex_home(),
        config_layer_stack,
    )?;

    assert!(
        !config
            .startup_warnings
            .iter()
            .any(|warning| warning.contains("Configured value for `web_search_mode`")),
        "{:?}",
        config.startup_warnings
    );

    Ok(())
}

#[test]
fn test_set_project_trusted_writes_explicit_tables() -> anyhow::Result<()> {
    let project_dir = Path::new("/some/path");
    let mut doc = DocumentMut::new();

    set_project_trust_level_inner(&mut doc, project_dir, TrustLevel::Trusted)?;

    let contents = doc.to_string();

    let raw_path = project_dir.to_string_lossy();
    let path_str = if raw_path.contains('\\') {
        format!("'{raw_path}'")
    } else {
        format!("\"{raw_path}\"")
    };
    let expected = format!(
        r#"[projects.{path_str}]
trust_level = "trusted"
"#
    );
    assert_eq!(contents, expected);

    Ok(())
}

#[test]
fn test_set_project_trusted_converts_inline_to_explicit() -> anyhow::Result<()> {
    let project_dir = Path::new("/some/path");

    // Seed config.toml with an inline project entry under [projects]
    let raw_path = project_dir.to_string_lossy();
    let path_str = if raw_path.contains('\\') {
        format!("'{raw_path}'")
    } else {
        format!("\"{raw_path}\"")
    };
    // Use a quoted key so backslashes don't require escaping on Windows
    let initial = format!(
        r#"[projects]
{path_str} = {{ trust_level = "untrusted" }}
"#
    );
    let mut doc = initial.parse::<DocumentMut>()?;

    // Run the function; it should convert to explicit tables and set trusted
    set_project_trust_level_inner(&mut doc, project_dir, TrustLevel::Trusted)?;

    let contents = doc.to_string();

    // Assert exact output after conversion to explicit table
    let expected = format!(
        r#"[projects]

[projects.{path_str}]
trust_level = "trusted"
"#
    );
    assert_eq!(contents, expected);

    Ok(())
}

#[test]
fn test_set_project_trusted_migrates_top_level_inline_projects_preserving_entries()
-> anyhow::Result<()> {
    let initial = r#"toplevel = "baz"
projects = { "/Users/mbolin/code/codex4" = { trust_level = "trusted", foo = "bar" } , "/Users/mbolin/code/codex3" = { trust_level = "trusted" } }
model = "foo""#;
    let mut doc = initial.parse::<DocumentMut>()?;

    // Approve a new directory
    let new_project = Path::new("/Users/mbolin/code/codex2");
    set_project_trust_level_inner(&mut doc, new_project, TrustLevel::Trusted)?;

    let contents = doc.to_string();

    // Since we created the [projects] table as part of migration, it is kept implicit.
    // Expect explicit per-project tables, preserving prior entries and appending the new one.
    let expected = r#"toplevel = "baz"
model = "foo"

[projects."/Users/mbolin/code/codex4"]
trust_level = "trusted"
foo = "bar"

[projects."/Users/mbolin/code/codex3"]
trust_level = "trusted"

[projects."/Users/mbolin/code/codex2"]
trust_level = "trusted"
"#;
    assert_eq!(contents, expected);

    Ok(())
}

#[test]
fn test_set_default_oss_provider() -> std::io::Result<()> {
    let temp_dir = TempDir::new()?;
    let codex_home = temp_dir.path();
    let config_path = codex_home.join(CONFIG_TOML_FILE);

    // Test setting valid provider on empty config
    set_default_oss_provider(codex_home, OLLAMA_OSS_PROVIDER_ID)?;
    let content = std::fs::read_to_string(&config_path)?;
    assert!(content.contains("oss_provider = \"ollama\""));

    // Test updating existing config
    std::fs::write(&config_path, "model = \"gpt-4\"\n")?;
    set_default_oss_provider(codex_home, LMSTUDIO_OSS_PROVIDER_ID)?;
    let content = std::fs::read_to_string(&config_path)?;
    assert!(content.contains("oss_provider = \"lmstudio\""));
    assert!(content.contains("model = \"gpt-4\""));

    // Test overwriting existing oss_provider
    set_default_oss_provider(codex_home, OLLAMA_OSS_PROVIDER_ID)?;
    let content = std::fs::read_to_string(&config_path)?;
    assert!(content.contains("oss_provider = \"ollama\""));
    assert!(!content.contains("oss_provider = \"lmstudio\""));

    // Test invalid provider
    let result = set_default_oss_provider(codex_home, "invalid_provider");
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains("Invalid OSS provider"));
    assert!(error.to_string().contains("invalid_provider"));

    Ok(())
}

#[test]
fn test_set_default_oss_provider_rejects_legacy_ollama_chat_provider() -> std::io::Result<()> {
    let temp_dir = TempDir::new()?;
    let codex_home = temp_dir.path();

    let result = set_default_oss_provider(codex_home, LEGACY_OLLAMA_CHAT_PROVIDER_ID);
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert!(
        error
            .to_string()
            .contains(OLLAMA_CHAT_PROVIDER_REMOVED_ERROR)
    );

    Ok(())
}

#[test]
fn test_load_config_rejects_legacy_ollama_chat_provider_with_helpful_error() -> std::io::Result<()>
{
    let codex_home = TempDir::new()?;
    let cfg = ConfigToml {
        model_provider: Some(LEGACY_OLLAMA_CHAT_PROVIDER_ID.to_string()),
        ..Default::default()
    };

    let result = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    );
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert!(
        error
            .to_string()
            .contains(OLLAMA_CHAT_PROVIDER_REMOVED_ERROR)
    );

    Ok(())
}

#[test]
fn test_untrusted_project_gets_workspace_write_sandbox() -> anyhow::Result<()> {
    let config_with_untrusted = r#"
[projects."/tmp/test"]
trust_level = "untrusted"
"#;

    let cfg = toml::from_str::<ConfigToml>(config_with_untrusted)
        .expect("TOML deserialization should succeed");

    let resolution = cfg.derive_sandbox_policy(
        None,
        None,
        WindowsSandboxLevel::Disabled,
        &PathBuf::from("/tmp/test"),
        None,
    );

    // Verify that untrusted projects get WorkspaceWrite (or ReadOnly on Windows due to downgrade)
    if cfg!(target_os = "windows") {
        assert!(
            matches!(resolution, SandboxPolicy::ReadOnly { .. }),
            "Expected ReadOnly on Windows, got {resolution:?}"
        );
    } else {
        assert!(
            matches!(resolution, SandboxPolicy::WorkspaceWrite { .. }),
            "Expected WorkspaceWrite for untrusted project, got {resolution:?}"
        );
    }

    Ok(())
}

#[test]
fn derive_sandbox_policy_falls_back_to_constraint_value_for_implicit_defaults() -> anyhow::Result<()>
{
    let project_dir = TempDir::new()?;
    let project_path = project_dir.path().to_path_buf();
    let project_key = project_path.to_string_lossy().to_string();
    let cfg = ConfigToml {
        projects: Some(HashMap::from([(
            project_key,
            ProjectConfig {
                trust_level: Some(TrustLevel::Trusted),
            },
        )])),
        ..Default::default()
    };
    let constrained = Constrained::new(SandboxPolicy::DangerFullAccess, |candidate| {
        if matches!(candidate, SandboxPolicy::DangerFullAccess) {
            Ok(())
        } else {
            Err(ConstraintError::InvalidValue {
                field_name: "sandbox_mode",
                candidate: format!("{candidate:?}"),
                allowed: "[DangerFullAccess]".to_string(),
                requirement_source: RequirementSource::Unknown,
            })
        }
    })?;

    let resolution = cfg.derive_sandbox_policy(
        None,
        None,
        WindowsSandboxLevel::Disabled,
        &project_path,
        Some(&constrained),
    );

    assert_eq!(resolution, SandboxPolicy::DangerFullAccess);
    Ok(())
}

#[test]
fn derive_sandbox_policy_preserves_windows_downgrade_for_unsupported_fallback() -> anyhow::Result<()>
{
    let project_dir = TempDir::new()?;
    let project_path = project_dir.path().to_path_buf();
    let project_key = project_path.to_string_lossy().to_string();
    let cfg = ConfigToml {
        projects: Some(HashMap::from([(
            project_key,
            ProjectConfig {
                trust_level: Some(TrustLevel::Trusted),
            },
        )])),
        ..Default::default()
    };
    let constrained = Constrained::new(SandboxPolicy::new_workspace_write_policy(), |candidate| {
        if matches!(candidate, SandboxPolicy::WorkspaceWrite { .. }) {
            Ok(())
        } else {
            Err(ConstraintError::InvalidValue {
                field_name: "sandbox_mode",
                candidate: format!("{candidate:?}"),
                allowed: "[WorkspaceWrite]".to_string(),
                requirement_source: RequirementSource::Unknown,
            })
        }
    })?;

    let resolution = cfg.derive_sandbox_policy(
        None,
        None,
        WindowsSandboxLevel::Disabled,
        &project_path,
        Some(&constrained),
    );

    if cfg!(target_os = "windows") {
        assert_eq!(resolution, SandboxPolicy::new_read_only_policy());
    } else {
        assert_eq!(resolution, SandboxPolicy::new_workspace_write_policy());
    }
    Ok(())
}

#[test]
fn test_resolve_oss_provider_explicit_override() {
    let config_toml = ConfigToml::default();
    let result = resolve_oss_provider(Some("custom-provider"), &config_toml, None);
    assert_eq!(result, Some("custom-provider".to_string()));
}

#[test]
fn test_resolve_oss_provider_from_profile() {
    let mut profiles = std::collections::HashMap::new();
    let profile = ConfigProfile {
        oss_provider: Some("profile-provider".to_string()),
        ..Default::default()
    };
    profiles.insert("test-profile".to_string(), profile);
    let config_toml = ConfigToml {
        profiles,
        ..Default::default()
    };

    let result = resolve_oss_provider(None, &config_toml, Some("test-profile".to_string()));
    assert_eq!(result, Some("profile-provider".to_string()));
}

#[test]
fn test_resolve_oss_provider_from_global_config() {
    let config_toml = ConfigToml {
        oss_provider: Some("global-provider".to_string()),
        ..Default::default()
    };

    let result = resolve_oss_provider(None, &config_toml, None);
    assert_eq!(result, Some("global-provider".to_string()));
}

#[test]
fn test_resolve_oss_provider_profile_fallback_to_global() {
    let mut profiles = std::collections::HashMap::new();
    let profile = ConfigProfile::default(); // No oss_provider set
    profiles.insert("test-profile".to_string(), profile);
    let config_toml = ConfigToml {
        oss_provider: Some("global-provider".to_string()),
        profiles,
        ..Default::default()
    };

    let result = resolve_oss_provider(None, &config_toml, Some("test-profile".to_string()));
    assert_eq!(result, Some("global-provider".to_string()));
}

#[test]
fn test_resolve_oss_provider_none_when_not_configured() {
    let config_toml = ConfigToml::default();
    let result = resolve_oss_provider(None, &config_toml, None);
    assert_eq!(result, None);
}

#[test]
fn test_resolve_oss_provider_explicit_overrides_all() {
    let mut profiles = std::collections::HashMap::new();
    let profile = ConfigProfile {
        oss_provider: Some("profile-provider".to_string()),
        ..Default::default()
    };
    profiles.insert("test-profile".to_string(), profile);
    let config_toml = ConfigToml {
        oss_provider: Some("global-provider".to_string()),
        profiles,
        ..Default::default()
    };

    let result = resolve_oss_provider(
        Some("explicit-provider"),
        &config_toml,
        Some("test-profile".to_string()),
    );
    assert_eq!(result, Some("explicit-provider".to_string()));
}

#[test]
fn config_toml_deserializes_mcp_oauth_callback_port() {
    let toml = r#"mcp_oauth_callback_port = 4321"#;
    let cfg: ConfigToml =
        toml::from_str(toml).expect("TOML deserialization should succeed for callback port");
    assert_eq!(cfg.mcp_oauth_callback_port, Some(4321));
}

#[test]
fn config_toml_deserializes_mcp_oauth_callback_url() {
    let toml = r#"mcp_oauth_callback_url = "https://example.com/callback""#;
    let cfg: ConfigToml =
        toml::from_str(toml).expect("TOML deserialization should succeed for callback URL");
    assert_eq!(
        cfg.mcp_oauth_callback_url.as_deref(),
        Some("https://example.com/callback")
    );
}

#[test]
fn config_loads_mcp_oauth_callback_port_from_toml() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let toml = r#"
model = "gpt-5.1"
mcp_oauth_callback_port = 5678
"#;
    let cfg: ConfigToml =
        toml::from_str(toml).expect("TOML deserialization should succeed for callback port");

    let config = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    )?;

    assert_eq!(config.mcp_oauth_callback_port, Some(5678));
    Ok(())
}

#[test]
fn config_loads_allow_login_shell_from_toml() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let cfg: ConfigToml = toml::from_str(
        r#"
model = "gpt-5.1"
allow_login_shell = false
"#,
    )
    .expect("TOML deserialization should succeed for allow_login_shell");

    let config = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    )?;

    assert!(!config.permissions.allow_login_shell);
    Ok(())
}

#[test]
fn config_loads_mcp_oauth_callback_url_from_toml() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let toml = r#"
model = "gpt-5.1"
mcp_oauth_callback_url = "https://example.com/callback"
"#;
    let cfg: ConfigToml =
        toml::from_str(toml).expect("TOML deserialization should succeed for callback URL");

    let config = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    )?;

    assert_eq!(
        config.mcp_oauth_callback_url.as_deref(),
        Some("https://example.com/callback")
    );
    Ok(())
}

#[test]
fn test_untrusted_project_gets_unless_trusted_approval_policy() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;
    let test_project_dir = TempDir::new()?;
    let test_path = test_project_dir.path();

    let config = Config::load_from_base_config_with_overrides(
        ConfigToml {
            projects: Some(HashMap::from([(
                test_path.to_string_lossy().to_string(),
                ProjectConfig {
                    trust_level: Some(TrustLevel::Untrusted),
                },
            )])),
            ..Default::default()
        },
        ConfigOverrides {
            cwd: Some(test_path.to_path_buf()),
            ..Default::default()
        },
        codex_home.path().to_path_buf(),
    )?;

    // Verify that untrusted projects get UnlessTrusted approval policy
    assert_eq!(
        config.permissions.approval_policy.value(),
        AskForApproval::UnlessTrusted,
        "Expected UnlessTrusted approval policy for untrusted project"
    );

    // Verify that untrusted projects still get WorkspaceWrite sandbox (or ReadOnly on Windows)
    if cfg!(target_os = "windows") {
        assert!(
            matches!(
                config.permissions.sandbox_policy.get(),
                SandboxPolicy::ReadOnly { .. }
            ),
            "Expected ReadOnly on Windows"
        );
    } else {
        assert!(
            matches!(
                config.permissions.sandbox_policy.get(),
                SandboxPolicy::WorkspaceWrite { .. }
            ),
            "Expected WorkspaceWrite sandbox for untrusted project"
        );
    }

    Ok(())
}

#[tokio::test]
async fn requirements_disallowing_default_sandbox_falls_back_to_required_default()
-> std::io::Result<()> {
    let codex_home = TempDir::new()?;

    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .cloud_requirements(CloudRequirementsLoader::new(async {
            Ok(Some(crate::config_loader::ConfigRequirementsToml {
                allowed_sandbox_modes: Some(vec![
                    crate::config_loader::SandboxModeRequirement::ReadOnly,
                ]),
                ..Default::default()
            }))
        }))
        .build()
        .await?;
    assert_eq!(
        *config.permissions.sandbox_policy.get(),
        SandboxPolicy::new_read_only_policy()
    );
    Ok(())
}

#[tokio::test]
async fn explicit_sandbox_mode_falls_back_when_disallowed_by_requirements() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        r#"sandbox_mode = "danger-full-access"
"#,
    )?;

    let requirements = crate::config_loader::ConfigRequirementsToml {
        allowed_approval_policies: None,
        allowed_sandbox_modes: Some(vec![crate::config_loader::SandboxModeRequirement::ReadOnly]),
        allowed_web_search_modes: None,
        feature_requirements: None,
        mcp_servers: None,
        apps: None,
        rules: None,
        enforce_residency: None,
        network: None,
    };

    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cloud_requirements(CloudRequirementsLoader::new(async move {
            Ok(Some(requirements))
        }))
        .build()
        .await?;
    assert_eq!(
        *config.permissions.sandbox_policy.get(),
        SandboxPolicy::new_read_only_policy()
    );
    Ok(())
}

#[tokio::test]
async fn requirements_web_search_mode_overrides_danger_full_access_default() -> std::io::Result<()>
{
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        r#"sandbox_mode = "danger-full-access"
"#,
    )?;

    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cloud_requirements(CloudRequirementsLoader::new(async {
            Ok(Some(crate::config_loader::ConfigRequirementsToml {
                allowed_web_search_modes: Some(vec![
                    crate::config_loader::WebSearchModeRequirement::Cached,
                ]),
                ..Default::default()
            }))
        }))
        .build()
        .await?;

    assert_eq!(config.web_search_mode.value(), WebSearchMode::Cached);
    assert_eq!(
        resolve_web_search_mode_for_turn(
            &config.web_search_mode,
            config.permissions.sandbox_policy.get(),
        ),
        WebSearchMode::Cached,
    );
    Ok(())
}

#[tokio::test]
async fn requirements_disallowing_default_approval_falls_back_to_required_default()
-> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    let workspace = TempDir::new()?;
    let workspace_key = workspace.path().to_string_lossy().replace('\\', "\\\\");
    std::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        format!(
            r#"
[projects."{workspace_key}"]
trust_level = "untrusted"
"#
        ),
    )?;

    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(workspace.path().to_path_buf()))
        .cloud_requirements(CloudRequirementsLoader::new(async {
            Ok(Some(crate::config_loader::ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::OnRequest]),
                ..Default::default()
            }))
        }))
        .build()
        .await?;

    assert_eq!(
        config.permissions.approval_policy.value(),
        AskForApproval::OnRequest
    );
    Ok(())
}

#[tokio::test]
async fn explicit_approval_policy_falls_back_when_disallowed_by_requirements() -> std::io::Result<()>
{
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        r#"approval_policy = "untrusted"
"#,
    )?;

    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cloud_requirements(CloudRequirementsLoader::new(async {
            Ok(Some(crate::config_loader::ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::OnRequest]),
                ..Default::default()
            }))
        }))
        .build()
        .await?;
    assert_eq!(
        config.permissions.approval_policy.value(),
        AskForApproval::OnRequest
    );
    Ok(())
}

#[tokio::test]
async fn feature_requirements_normalize_effective_feature_values() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;

    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .cloud_requirements(CloudRequirementsLoader::new(async {
            Ok(Some(crate::config_loader::ConfigRequirementsToml {
                feature_requirements: Some(crate::config_loader::FeatureRequirementsToml {
                    entries: BTreeMap::from([
                        ("personality".to_string(), true),
                        ("shell_tool".to_string(), false),
                    ]),
                }),
                ..Default::default()
            }))
        }))
        .build()
        .await?;

    assert!(config.features.enabled(Feature::Personality));
    assert!(!config.features.enabled(Feature::ShellTool));
    assert!(
        !config
            .startup_warnings
            .iter()
            .any(|warning| warning.contains("Configured value for `features`")),
        "{:?}",
        config.startup_warnings
    );

    Ok(())
}

#[tokio::test]
async fn explicit_feature_config_is_normalized_by_requirements() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        r#"
[features]
personality = false
shell_tool = true
"#,
    )?;

    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .cloud_requirements(CloudRequirementsLoader::new(async {
            Ok(Some(crate::config_loader::ConfigRequirementsToml {
                feature_requirements: Some(crate::config_loader::FeatureRequirementsToml {
                    entries: BTreeMap::from([
                        ("personality".to_string(), true),
                        ("shell_tool".to_string(), false),
                    ]),
                }),
                ..Default::default()
            }))
        }))
        .build()
        .await?;

    assert!(config.features.enabled(Feature::Personality));
    assert!(!config.features.enabled(Feature::ShellTool));
    assert!(
        !config
            .startup_warnings
            .iter()
            .any(|warning| warning.contains("Configured value for `features`")),
        "{:?}",
        config.startup_warnings
    );

    Ok(())
}

#[tokio::test]
async fn approval_review_policy_defaults_to_manual_only_without_guardian_feature()
-> std::io::Result<()> {
    let codex_home = TempDir::new()?;

    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .build()
        .await?;

    assert_eq!(
        config.approval_review_policy,
        ApprovalReviewPolicy::ManualOnly
    );
    Ok(())
}

#[tokio::test]
async fn approval_review_policy_stays_manual_only_when_guardian_feature_is_enabled()
-> std::io::Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        r#"[features]
guardian_approval = true
"#,
    )?;

    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .build()
        .await?;

    assert_eq!(
        config.approval_review_policy,
        ApprovalReviewPolicy::ManualOnly
    );
    Ok(())
}

#[tokio::test]
async fn feature_requirements_normalize_runtime_feature_mutations() -> std::io::Result<()> {
    let codex_home = TempDir::new()?;

    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .cloud_requirements(CloudRequirementsLoader::new(async {
            Ok(Some(crate::config_loader::ConfigRequirementsToml {
                feature_requirements: Some(crate::config_loader::FeatureRequirementsToml {
                    entries: BTreeMap::from([
                        ("personality".to_string(), true),
                        ("shell_tool".to_string(), false),
                    ]),
                }),
                ..Default::default()
            }))
        }))
        .build()
        .await?;

    let mut requested = config.features.get().clone();
    requested
        .disable(Feature::Personality)
        .enable(Feature::ShellTool);
    assert!(config.features.can_set(&requested).is_ok());
    config
        .features
        .set(requested)
        .expect("managed feature mutations should normalize successfully");

    assert!(config.features.enabled(Feature::Personality));
    assert!(!config.features.enabled(Feature::ShellTool));

    Ok(())
}

#[tokio::test]
async fn feature_requirements_reject_collab_legacy_alias() {
    let codex_home = TempDir::new().expect("tempdir");

    let err = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .cloud_requirements(CloudRequirementsLoader::new(async {
            Ok(Some(crate::config_loader::ConfigRequirementsToml {
                feature_requirements: Some(crate::config_loader::FeatureRequirementsToml {
                    entries: BTreeMap::from([("collab".to_string(), true)]),
                }),
                ..Default::default()
            }))
        }))
        .build()
        .await
        .expect_err("legacy aliases should be rejected");

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    assert!(
        err.to_string()
            .contains("use canonical feature key `multi_agent`"),
        "{err}"
    );
}

#[test]
fn experimental_realtime_start_instructions_load_from_config_toml() -> std::io::Result<()> {
    let cfg: ConfigToml = toml::from_str(
        r#"
experimental_realtime_start_instructions = "start instructions from config"
"#,
    )
    .expect("TOML deserialization should succeed");

    assert_eq!(
        cfg.experimental_realtime_start_instructions.as_deref(),
        Some("start instructions from config")
    );

    let codex_home = TempDir::new()?;
    let config = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    )?;

    assert_eq!(
        config.experimental_realtime_start_instructions.as_deref(),
        Some("start instructions from config")
    );
    Ok(())
}

#[test]
fn experimental_realtime_ws_base_url_loads_from_config_toml() -> std::io::Result<()> {
    let cfg: ConfigToml = toml::from_str(
        r#"
experimental_realtime_ws_base_url = "http://127.0.0.1:8011"
"#,
    )
    .expect("TOML deserialization should succeed");

    assert_eq!(
        cfg.experimental_realtime_ws_base_url.as_deref(),
        Some("http://127.0.0.1:8011")
    );

    let codex_home = TempDir::new()?;
    let config = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    )?;

    assert_eq!(
        config.experimental_realtime_ws_base_url.as_deref(),
        Some("http://127.0.0.1:8011")
    );
    Ok(())
}

#[test]
fn experimental_realtime_ws_backend_prompt_loads_from_config_toml() -> std::io::Result<()> {
    let cfg: ConfigToml = toml::from_str(
        r#"
experimental_realtime_ws_backend_prompt = "prompt from config"
"#,
    )
    .expect("TOML deserialization should succeed");

    assert_eq!(
        cfg.experimental_realtime_ws_backend_prompt.as_deref(),
        Some("prompt from config")
    );

    let codex_home = TempDir::new()?;
    let config = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    )?;

    assert_eq!(
        config.experimental_realtime_ws_backend_prompt.as_deref(),
        Some("prompt from config")
    );
    Ok(())
}

#[test]
fn experimental_realtime_ws_startup_context_loads_from_config_toml() -> std::io::Result<()> {
    let cfg: ConfigToml = toml::from_str(
        r#"
experimental_realtime_ws_startup_context = "startup context from config"
"#,
    )
    .expect("TOML deserialization should succeed");

    assert_eq!(
        cfg.experimental_realtime_ws_startup_context.as_deref(),
        Some("startup context from config")
    );

    let codex_home = TempDir::new()?;
    let config = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    )?;

    assert_eq!(
        config.experimental_realtime_ws_startup_context.as_deref(),
        Some("startup context from config")
    );
    Ok(())
}

#[test]
fn experimental_realtime_ws_model_loads_from_config_toml() -> std::io::Result<()> {
    let cfg: ConfigToml = toml::from_str(
        r#"
experimental_realtime_ws_model = "realtime-test-model"
"#,
    )
    .expect("TOML deserialization should succeed");

    assert_eq!(
        cfg.experimental_realtime_ws_model.as_deref(),
        Some("realtime-test-model")
    );

    let codex_home = TempDir::new()?;
    let config = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    )?;

    assert_eq!(
        config.experimental_realtime_ws_model.as_deref(),
        Some("realtime-test-model")
    );
    Ok(())
}

#[test]
fn realtime_loads_from_config_toml() -> std::io::Result<()> {
    let cfg: ConfigToml = toml::from_str(
        r#"
[realtime]
version = "v2"
type = "transcription"
"#,
    )
    .expect("TOML deserialization should succeed");

    assert_eq!(
        cfg.realtime,
        Some(RealtimeToml {
            version: Some(RealtimeWsVersion::V2),
            session_type: Some(RealtimeWsMode::Transcription),
        })
    );

    let codex_home = TempDir::new()?;
    let config = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    )?;

    assert_eq!(
        config.realtime,
        RealtimeConfig {
            version: RealtimeWsVersion::V2,
            session_type: RealtimeWsMode::Transcription,
        }
    );
    Ok(())
}

#[test]
fn realtime_audio_loads_from_config_toml() -> std::io::Result<()> {
    let cfg: ConfigToml = toml::from_str(
        r#"
[audio]
microphone = "USB Mic"
speaker = "Desk Speakers"
"#,
    )
    .expect("TOML deserialization should succeed");

    let realtime_audio = cfg
        .audio
        .as_ref()
        .expect("realtime audio config should be present");
    assert_eq!(realtime_audio.microphone.as_deref(), Some("USB Mic"));
    assert_eq!(realtime_audio.speaker.as_deref(), Some("Desk Speakers"));

    let codex_home = TempDir::new()?;
    let config = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides::default(),
        codex_home.path().to_path_buf(),
    )?;

    assert_eq!(config.realtime_audio.microphone.as_deref(), Some("USB Mic"));
    assert_eq!(
        config.realtime_audio.speaker.as_deref(),
        Some("Desk Speakers")
    );
    Ok(())
}

#[derive(Deserialize, Debug, PartialEq)]
struct TuiTomlTest {
    #[serde(default)]
    notifications: Notifications,
    #[serde(default)]
    notification_method: NotificationMethod,
}

#[derive(Deserialize, Debug, PartialEq)]
struct RootTomlTest {
    tui: TuiTomlTest,
}

#[test]
fn test_tui_notifications_true() {
    let toml = r#"
            [tui]
            notifications = true
        "#;
    let parsed: RootTomlTest = toml::from_str(toml).expect("deserialize notifications=true");
    assert_matches!(parsed.tui.notifications, Notifications::Enabled(true));
}

#[test]
fn test_tui_notifications_custom_array() {
    let toml = r#"
            [tui]
            notifications = ["foo"]
        "#;
    let parsed: RootTomlTest = toml::from_str(toml).expect("deserialize notifications=[\"foo\"]");
    assert_matches!(
        parsed.tui.notifications,
        Notifications::Custom(ref v) if v == &vec!["foo".to_string()]
    );
}

#[test]
fn test_tui_notification_method() {
    let toml = r#"
            [tui]
            notification_method = "bel"
        "#;
    let parsed: RootTomlTest =
        toml::from_str(toml).expect("deserialize notification_method=\"bel\"");
    assert_eq!(parsed.tui.notification_method, NotificationMethod::Bel);
}
