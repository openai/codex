//! Narrow conversion helpers for approval-related app-server payloads.
//!
//! The TUI mostly keeps app-server approval types intact. These helpers cover
//! the remaining cases where the UI consumes a private file-change display
//! model or needs to translate a granted permission response for outbound
//! submission.

use crate::diff_model::FileChange;
use codex_app_server_protocol::AdditionalNetworkPermissions;
use codex_app_server_protocol::FileUpdateChange;
use codex_app_server_protocol::GrantedPermissionProfile;
use codex_app_server_protocol::PatchChangeKind;
use codex_protocol::request_permissions::RequestPermissionProfile as CoreRequestPermissionProfile;
use std::collections::HashMap;
use std::path::PathBuf;

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

pub(crate) fn file_update_changes_to_display(
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
    use super::file_update_changes_to_display;
    use super::granted_permission_profile_from_request;
    use crate::diff_model::FileChange;
    use codex_app_server_protocol::FileUpdateChange;
    use codex_app_server_protocol::PatchChangeKind;
    use codex_protocol::request_permissions::RequestPermissionProfile as CoreRequestPermissionProfile;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn absolute_path(path: &str) -> AbsolutePathBuf {
        AbsolutePathBuf::try_from(PathBuf::from(path)).expect("path must be absolute")
    }

    #[test]
    fn converts_file_update_changes_to_display() {
        assert_eq!(
            file_update_changes_to_display(vec![FileUpdateChange {
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
            granted_permission_profile_from_request(CoreRequestPermissionProfile::from(
                codex_app_server_protocol::RequestPermissionProfile {
                    network: Some(codex_app_server_protocol::AdditionalNetworkPermissions {
                        enabled: Some(true),
                    }),
                    file_system: Some(codex_app_server_protocol::AdditionalFileSystemPermissions {
                        read: Some(vec![absolute_path("/tmp/read-only")]),
                        write: Some(vec![absolute_path("/tmp/write")]),
                        glob_scan_max_depth: None,
                        entries: None,
                    }),
                }
            )),
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
            granted_permission_profile_from_request(CoreRequestPermissionProfile::from(
                codex_app_server_protocol::RequestPermissionProfile {
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
                        }]),
                    }),
                }
            )),
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
