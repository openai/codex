use codex_app_server_protocol::AdditionalNetworkPermissions;
use codex_app_server_protocol::FileUpdateChange;
use codex_app_server_protocol::GrantedPermissionProfile;
use codex_app_server_protocol::NetworkApprovalContext as AppServerNetworkApprovalContext;
use codex_app_server_protocol::PatchChangeKind;
use codex_protocol::protocol::FileChange;
use codex_protocol::protocol::NetworkApprovalContext;
use codex_protocol::protocol::NetworkApprovalProtocol;
use codex_protocol::request_permissions::RequestPermissionProfile as CoreRequestPermissionProfile;
use std::collections::HashMap;
use std::path::PathBuf;

pub(crate) fn network_approval_context_to_core(
    value: AppServerNetworkApprovalContext,
) -> NetworkApprovalContext {
    NetworkApprovalContext {
        host: value.host,
        protocol: match value.protocol {
            codex_app_server_protocol::NetworkApprovalProtocol::Http => {
                NetworkApprovalProtocol::Http
            }
            codex_app_server_protocol::NetworkApprovalProtocol::Https => {
                NetworkApprovalProtocol::Https
            }
            codex_app_server_protocol::NetworkApprovalProtocol::Socks5Tcp => {
                NetworkApprovalProtocol::Socks5Tcp
            }
            codex_app_server_protocol::NetworkApprovalProtocol::Socks5Udp => {
                NetworkApprovalProtocol::Socks5Udp
            }
        },
    }
}

pub(crate) fn granted_permission_profile_from_request(
    value: CoreRequestPermissionProfile,
) -> GrantedPermissionProfile {
    GrantedPermissionProfile {
        network: value.network.map(|network| AdditionalNetworkPermissions {
            enabled: network.enabled,
        }),
        file_system: value.file_system.map(Into::into),
    }
}

pub(crate) fn file_update_changes_to_core(
    changes: Vec<FileUpdateChange>,
) -> HashMap<PathBuf, FileChange> {
    changes
        .into_iter()
        .map(|change| {
            let path = PathBuf::from(change.path);
            let file_change = match change.kind {
                PatchChangeKind::Add => FileChange::Add {
                    content: change.diff,
                },
                PatchChangeKind::Delete => FileChange::Delete {
                    content: change.diff,
                },
                PatchChangeKind::Update { move_path } => FileChange::Update {
                    unified_diff: change.diff,
                    move_path,
                },
            };
            (path, file_change)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::file_update_changes_to_core;
    use super::granted_permission_profile_from_request;
    use super::network_approval_context_to_core;
    use codex_app_server_protocol::FileUpdateChange;
    use codex_app_server_protocol::PatchChangeKind;
    use codex_protocol::models::FileSystemPermissions;
    use codex_protocol::models::NetworkPermissions;
    use codex_protocol::permissions::FileSystemAccessMode;
    use codex_protocol::permissions::FileSystemPath;
    use codex_protocol::permissions::FileSystemSandboxEntry;
    use codex_protocol::permissions::FileSystemSpecialPath;
    use codex_protocol::protocol::FileChange;
    use codex_protocol::protocol::NetworkApprovalContext;
    use codex_protocol::protocol::NetworkApprovalProtocol;
    use codex_protocol::request_permissions::RequestPermissionProfile as CoreRequestPermissionProfile;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn absolute_path(path: &str) -> AbsolutePathBuf {
        AbsolutePathBuf::try_from(PathBuf::from(path)).expect("path must be absolute")
    }

    #[test]
    fn converts_app_server_network_approval_context_to_core() {
        assert_eq!(
            network_approval_context_to_core(codex_app_server_protocol::NetworkApprovalContext {
                host: "example.com".to_string(),
                protocol: codex_app_server_protocol::NetworkApprovalProtocol::Socks5Tcp,
            }),
            NetworkApprovalContext {
                host: "example.com".to_string(),
                protocol: NetworkApprovalProtocol::Socks5Tcp,
            }
        );
    }

    #[test]
    fn converts_file_update_changes_to_core() {
        assert_eq!(
            file_update_changes_to_core(vec![FileUpdateChange {
                path: "foo.txt".to_string(),
                kind: PatchChangeKind::Add,
                diff: "hello\n".to_string(),
            }]),
            HashMap::from([(
                PathBuf::from("foo.txt"),
                FileChange::Add {
                    content: "hello\n".to_string(),
                },
            )])
        );
    }

    #[test]
    fn converts_request_permissions_into_granted_permissions() {
        assert_eq!(
            granted_permission_profile_from_request(CoreRequestPermissionProfile {
                network: Some(NetworkPermissions {
                    enabled: Some(true),
                }),
                file_system: Some(FileSystemPermissions::from_read_write_roots(
                    Some(vec![absolute_path("/tmp/read-only")]),
                    Some(vec![absolute_path("/tmp/write")]),
                )),
            }),
            codex_app_server_protocol::GrantedPermissionProfile {
                network: Some(codex_app_server_protocol::AdditionalNetworkPermissions {
                    enabled: Some(true),
                }),
                file_system: Some(codex_app_server_protocol::AdditionalFileSystemPermissions {
                    read: Some(vec![absolute_path("/tmp/read-only")]),
                    write: Some(vec![absolute_path("/tmp/write")]),
                    glob_scan_max_depth: None,
                    entries: Some(vec![
                        codex_app_server_protocol::FileSystemSandboxEntry {
                            path: codex_app_server_protocol::FileSystemPath::Path {
                                path: absolute_path("/tmp/read-only"),
                            },
                            access: codex_app_server_protocol::FileSystemAccessMode::Read,
                        },
                        codex_app_server_protocol::FileSystemSandboxEntry {
                            path: codex_app_server_protocol::FileSystemPath::Path {
                                path: absolute_path("/tmp/write"),
                            },
                            access: codex_app_server_protocol::FileSystemAccessMode::Write,
                        },
                    ]),
                }),
            }
        );
    }

    #[test]
    fn converts_request_permissions_into_canonical_granted_permissions() {
        assert_eq!(
            granted_permission_profile_from_request(CoreRequestPermissionProfile {
                file_system: Some(FileSystemPermissions {
                    entries: vec![FileSystemSandboxEntry {
                        path: FileSystemPath::Special {
                            value: FileSystemSpecialPath::Root,
                        },
                        access: FileSystemAccessMode::Write,
                    }],
                    glob_scan_max_depth: None,
                }),
                ..Default::default()
            }),
            codex_app_server_protocol::GrantedPermissionProfile {
                network: None,
                file_system: Some(codex_app_server_protocol::AdditionalFileSystemPermissions {
                    read: None,
                    write: None,
                    glob_scan_max_depth: None,
                    entries: Some(vec![codex_app_server_protocol::FileSystemSandboxEntry {
                        path: codex_app_server_protocol::FileSystemPath::Special {
                            value: codex_app_server_protocol::FileSystemSpecialPath::Root,
                        },
                        access: codex_app_server_protocol::FileSystemAccessMode::Write,
                    },]),
                }),
            }
        );
    }
}
