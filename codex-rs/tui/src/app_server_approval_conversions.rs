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
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::ApiPathString;
use codex_utils_path_uri::PathConvention;
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

pub(crate) fn command_approval_cwd_to_host(
    cwd: Option<&ApiPathString>,
    fallback_cwd: &AbsolutePathBuf,
) -> AbsolutePathBuf {
    cwd.and_then(|cwd| {
        let convention = cwd.infer_absolute_path_convention()?;
        let cwd = cwd.to_path_uri(convention).ok()?;
        let cwd = ApiPathString::from_path_uri(&cwd, PathConvention::native()).ok()?;
        AbsolutePathBuf::try_from(cwd.into_string()).ok()
    })
    .unwrap_or_else(|| fallback_cwd.clone())
}

#[cfg(test)]
mod tests {
    use super::file_update_changes_to_display;
    use super::granted_permission_profile_from_request;
    use crate::diff_model::FileChange;
    use codex_app_server_protocol::AdditionalFileSystemPermissions;
    use codex_app_server_protocol::AdditionalNetworkPermissions;
    use codex_app_server_protocol::FileSystemAccessMode;
    use codex_app_server_protocol::FileSystemPath;
    use codex_app_server_protocol::FileSystemSandboxEntry;
    use codex_app_server_protocol::FileSystemSpecialPath;
    use codex_app_server_protocol::FileUpdateChange;
    use codex_app_server_protocol::GrantedPermissionProfile;
    use codex_app_server_protocol::PatchChangeKind;
    use codex_app_server_protocol::RequestPermissionProfile;
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
        let request = RequestPermissionProfile {
            network: Some(AdditionalNetworkPermissions {
                enabled: Some(true),
            }),
            file_system: Some(AdditionalFileSystemPermissions {
                read: Some(vec![absolute_path("/tmp/read-only").into()]),
                write: Some(vec![absolute_path("/tmp/write").into()]),
                glob_scan_max_depth: None,
                entries: None,
            }),
        };
        let request = CoreRequestPermissionProfile::try_from(request)
            .expect("API paths should convert to native paths");

        assert_eq!(
            granted_permission_profile_from_request(request),
            GrantedPermissionProfile {
                network: Some(AdditionalNetworkPermissions {
                    enabled: Some(true),
                }),
                file_system: Some(AdditionalFileSystemPermissions {
                    read: Some(vec![absolute_path("/tmp/read-only").into()]),
                    write: Some(vec![absolute_path("/tmp/write").into()]),
                    glob_scan_max_depth: None,
                    entries: Some(vec![
                        FileSystemSandboxEntry {
                            path: FileSystemPath::Path {
                                path: absolute_path("/tmp/read-only").into(),
                            },
                            access: FileSystemAccessMode::Read,
                        },
                        FileSystemSandboxEntry {
                            path: FileSystemPath::Path {
                                path: absolute_path("/tmp/write").into(),
                            },
                            access: FileSystemAccessMode::Write,
                        },
                    ]),
                }),
            }
        );
    }

    #[test]
    fn converts_request_permissions_into_canonical_granted_permissions() {
        let request = RequestPermissionProfile {
            network: None,
            file_system: Some(AdditionalFileSystemPermissions {
                read: None,
                write: None,
                glob_scan_max_depth: None,
                entries: Some(vec![FileSystemSandboxEntry {
                    path: FileSystemPath::Special {
                        value: FileSystemSpecialPath::Root,
                    },
                    access: FileSystemAccessMode::Write,
                }]),
            }),
        };
        let request = CoreRequestPermissionProfile::try_from(request)
            .expect("API paths should convert to native paths");

        assert_eq!(
            granted_permission_profile_from_request(request),
            GrantedPermissionProfile {
                network: None,
                file_system: Some(AdditionalFileSystemPermissions {
                    read: None,
                    write: None,
                    glob_scan_max_depth: None,
                    entries: Some(vec![FileSystemSandboxEntry {
                        path: FileSystemPath::Special {
                            value: FileSystemSpecialPath::Root,
                        },
                        access: FileSystemAccessMode::Write,
                    }]),
                }),
            }
        );
    }
}
