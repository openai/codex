//! Compatibility projections from the canonical permission profile model into
//! legacy shapes still required by older or remote app-server APIs.

use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::path::Path;

pub(crate) fn legacy_compatible_permission_profile(
    permission_profile: &PermissionProfile,
    cwd: &Path,
) -> PermissionProfile {
    if permission_profile.to_legacy_sandbox_policy(cwd).is_ok() {
        return permission_profile.clone();
    }

    let file_system_policy = permission_profile.file_system_sandbox_policy();
    compatibility_workspace_write_profile(
        &file_system_policy,
        permission_profile.network_sandbox_policy(),
        cwd,
    )
}

fn compatibility_workspace_write_profile(
    file_system_policy: &FileSystemSandboxPolicy,
    network_policy: NetworkSandboxPolicy,
    cwd: &Path,
) -> PermissionProfile {
    let cwd_abs = AbsolutePathBuf::from_absolute_path(cwd).ok();
    let writable_roots = file_system_policy
        .get_writable_roots_with_cwd(cwd)
        .into_iter()
        .map(|root| root.root)
        .filter(|root| cwd_abs.as_ref() != Some(root))
        .collect::<Vec<_>>();
    let tmpdir_writable = std::env::var_os("TMPDIR")
        .filter(|tmpdir| !tmpdir.is_empty())
        .and_then(|tmpdir| {
            AbsolutePathBuf::from_absolute_path(std::path::PathBuf::from(tmpdir)).ok()
        })
        .is_some_and(|tmpdir| file_system_policy.can_write_path_with_cwd(tmpdir.as_path(), cwd));
    let slash_tmp = Path::new("/tmp");
    let slash_tmp_writable = slash_tmp.is_absolute()
        && slash_tmp.is_dir()
        && file_system_policy.can_write_path_with_cwd(slash_tmp, cwd);

    PermissionProfile::workspace_write_with(
        &writable_roots,
        network_policy,
        !tmpdir_writable,
        !slash_tmp_writable,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::models::ManagedFileSystemPermissions;
    use codex_protocol::permissions::FileSystemAccessMode;
    use codex_protocol::permissions::FileSystemPath;
    use codex_protocol::permissions::FileSystemSandboxEntry;
    use codex_protocol::permissions::FileSystemSpecialPath;
    use pretty_assertions::assert_eq;

    #[test]
    fn compatibility_profile_preserves_unbridgeable_write_roots() {
        let cwd = AbsolutePathBuf::try_from("/workspace/project").expect("absolute cwd");
        let extra_root = AbsolutePathBuf::try_from("/workspace/extra").expect("absolute root");
        let permission_profile = PermissionProfile::Managed {
            file_system: ManagedFileSystemPermissions::Restricted {
                entries: vec![
                    FileSystemSandboxEntry {
                        path: FileSystemPath::Special {
                            value: FileSystemSpecialPath::Root,
                        },
                        access: FileSystemAccessMode::Read,
                    },
                    FileSystemSandboxEntry {
                        path: FileSystemPath::Path {
                            path: extra_root.clone(),
                        },
                        access: FileSystemAccessMode::Write,
                    },
                ],
                glob_scan_max_depth: None,
            },
            network: NetworkSandboxPolicy::Restricted,
        };

        let compatibility_profile =
            legacy_compatible_permission_profile(&permission_profile, cwd.as_path());
        let policy = compatibility_profile
            .to_legacy_sandbox_policy(cwd.as_path())
            .expect("compatibility profile should project to legacy policy");
        let roots = policy
            .get_writable_roots_with_cwd(cwd.as_path())
            .into_iter()
            .map(|root| root.root)
            .collect::<Vec<_>>();

        assert_eq!(roots, vec![extra_root, cwd]);
    }
}
