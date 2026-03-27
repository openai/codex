use super::*;
use crate::config::Config;
use crate::config::ConfigOverrides;
use crate::config::ConfigToml;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use tempfile::TempDir;

#[test]
fn normalize_absolute_path_for_platform_simplifies_windows_verbatim_paths() {
    let parsed =
        normalize_absolute_path_for_platform(r"\\?\D:\c\x\worktrees\2508\swift-base", true);
    assert_eq!(parsed, PathBuf::from(r"D:\c\x\worktrees\2508\swift-base"));
}

#[test]
fn restricted_read_implicitly_allows_helper_executables() -> std::io::Result<()> {
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
        codex_home,
    )?;

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
