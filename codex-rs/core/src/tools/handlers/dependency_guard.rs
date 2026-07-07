use codex_dependency_check::is_dependency_manifest_path;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::models::SandboxPermissions;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;

pub(crate) fn dependency_permission_request_message() -> String {
    "Dependency Check is enabled. Generic tools cannot request permissions that would make dependency manifests writable or bypass their sandbox protection. Use the `dependency_check` tool for npm dependency updates."
        .to_string()
}

pub(crate) fn dependency_permissions_overlap_project(
    sandbox_permissions: SandboxPermissions,
    additional_permissions: Option<&AdditionalPermissionProfile>,
    project_root: &AbsolutePathBuf,
) -> bool {
    if sandbox_permissions.requires_escalated_permissions() {
        return true;
    }

    additional_permissions
        .and_then(|permissions| permissions.file_system.as_ref())
        .is_some_and(|permissions| {
            permissions.entries.iter().any(|entry| {
                entry.access == FileSystemAccessMode::Write
                    && file_system_path_overlaps_project(&entry.path, project_root)
            })
        })
}

pub(crate) fn path_uri_is_dependency_manifest(path: &PathUri) -> bool {
    path.to_abs_path()
        .is_ok_and(|path| is_dependency_manifest_path(path.as_path()))
}

fn file_system_path_overlaps_project(
    path: &FileSystemPath<AbsolutePathBuf>,
    project_root: &AbsolutePathBuf,
) -> bool {
    match path {
        FileSystemPath::Path { path } => {
            is_dependency_manifest_path(path.as_path())
                || path.as_path().starts_with(project_root.as_path())
                || project_root.as_path().starts_with(path.as_path())
        }
        FileSystemPath::GlobPattern { .. } => true,
        FileSystemPath::Special { value } => matches!(
            value,
            FileSystemSpecialPath::Root
                | FileSystemSpecialPath::ProjectRoots { .. }
                | FileSystemSpecialPath::Tmpdir
                | FileSystemSpecialPath::SlashTmp
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::models::FileSystemPermissions;
    use codex_protocol::permissions::FileSystemSandboxEntry;
    use pretty_assertions::assert_eq;

    fn write_profile(path: FileSystemPath<AbsolutePathBuf>) -> AdditionalPermissionProfile {
        AdditionalPermissionProfile {
            file_system: Some(FileSystemPermissions {
                entries: vec![FileSystemSandboxEntry {
                    path,
                    access: FileSystemAccessMode::Write,
                }],
                glob_scan_max_depth: None,
            }),
            ..Default::default()
        }
    }

    #[test]
    fn rejects_full_escalation_and_project_writes() {
        let root = AbsolutePathBuf::from_absolute_path("/workspace/project")
            .expect("absolute project root");
        let nested = AbsolutePathBuf::from_absolute_path("/workspace/project/packages/app")
            .expect("absolute nested path");
        let profile = write_profile(FileSystemPath::Path { path: nested });

        assert!(dependency_permissions_overlap_project(
            SandboxPermissions::RequireEscalated,
            /*additional_permissions*/ None,
            &root,
        ));
        assert!(dependency_permissions_overlap_project(
            SandboxPermissions::WithAdditionalPermissions,
            Some(&profile),
            &root,
        ));
    }

    #[test]
    fn permits_unrelated_write_permissions() {
        let root = AbsolutePathBuf::from_absolute_path("/workspace/project")
            .expect("absolute project root");
        let cache =
            AbsolutePathBuf::from_absolute_path("/var/cache/example").expect("absolute cache path");
        let profile = write_profile(FileSystemPath::Path { path: cache });

        assert_eq!(
            dependency_permissions_overlap_project(
                SandboxPermissions::WithAdditionalPermissions,
                Some(&profile),
                &root,
            ),
            false
        );
    }
}
