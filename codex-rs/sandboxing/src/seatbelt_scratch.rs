//! Lower implicit process scratch access through the normal Seatbelt exclusions.
//! Explicit restrictions and project metadata also constrain these broad grants.

use super::SeatbeltAccessRoot;
use super::SeatbeltPreparationError;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::PROTECTED_METADATA_PATH_NAMES;
use codex_protocol::permissions::ResolvedFileSystemPermissions;
use codex_protocol::protocol::WritableRoot;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::path::Path;

pub(super) fn scratch_access_roots(
    policy: &FileSystemSandboxPolicy,
    cwd: &Path,
    filesystem: &ResolvedFileSystemPermissions,
) -> Result<(Vec<SeatbeltAccessRoot>, Vec<WritableRoot>), SeatbeltPreparationError> {
    let mut read_only = filesystem
        .readable_roots
        .iter()
        .filter(|path| !policy.can_write_local_path_with_cwd(path.as_path(), cwd))
        .cloned()
        .collect::<Vec<_>>();
    read_only.extend(filesystem.unreadable_roots.iter().cloned());
    for root in &filesystem.writable_roots {
        read_only.extend(root.read_only_subpaths.iter().cloned());
        read_only.extend(
            root.protected_metadata_names
                .iter()
                .map(|name| root.root.join(name)),
        );
    }

    // Use the physical paths: /tmp and /var are trusted system aliases, and
    // the normal lowerer binds both roots and exclusions in that namespace.
    let scratch_paths = ["/private/tmp", "/private/var/tmp"]
        .into_iter()
        .map(AbsolutePathBuf::from_absolute_path)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| SeatbeltPreparationError::FileSystem(error.to_string()))?;
    let mut scratch_policy = policy.clone();
    scratch_policy.entries.extend(
        scratch_paths.iter().map(|path| {
            FileSystemSandboxEntry::new(path.clone().into(), FileSystemAccessMode::Write)
        }),
    );
    let writes = scratch_policy
        .get_writable_roots_with_cwd_preserving_mutable_paths(cwd)
        .into_iter()
        .filter(|root| scratch_paths.contains(&root.root))
        .map(|mut root| {
            // Retain normal logical-path exclusions (including symlink inodes).
            // Explicit restrictions at or above a scratch root, and metadata
            // belonging to narrower project roots, also constrain the default.
            root.read_only_subpaths.extend(read_only.iter().cloned());
            for name in PROTECTED_METADATA_PATH_NAMES {
                if !root
                    .protected_metadata_names
                    .iter()
                    .any(|existing| existing == name)
                    && !policy.can_write_local_path_with_cwd(root.root.join(*name).as_path(), cwd)
                {
                    root.protected_metadata_names.push((*name).to_string());
                }
            }
            root
        })
        .collect();
    let reads = scratch_paths
        .into_iter()
        .map(|root| SeatbeltAccessRoot {
            root,
            excluded_subpaths: filesystem.unreadable_roots.clone(),
            protected_metadata_names: Vec::new(),
        })
        .collect();
    Ok((reads, writes))
}
