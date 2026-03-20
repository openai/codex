use super::*;
use pretty_assertions::assert_eq;

#[test]
fn normalize_absolute_path_for_platform_simplifies_windows_verbatim_paths() {
    let parsed =
        normalize_absolute_path_for_platform(r"\\?\D:\c\x\worktrees\2508\swift-base", true);
    assert_eq!(parsed, PathBuf::from(r"D:\c\x\worktrees\2508\swift-base"));
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
            (
                "ignored.example.com".to_string(),
                NetworkDomainPermissionToml::None,
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
