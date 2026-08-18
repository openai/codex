use std::path::Path;

use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::canonicalize_preserving_symlinks;
use thiserror::Error;

use crate::models::PermissionProfile;
use crate::permissions::FileSystemAccessMode;
use crate::permissions::FileSystemPath;
use crate::permissions::FileSystemSandboxEntry;
use crate::permissions::FileSystemSandboxKind;
use crate::permissions::FileSystemSandboxPolicy;
use crate::permissions::FileSystemSpecialPath;
use crate::permissions::NetworkSandboxPolicy;
use crate::permissions::PROTECTED_METADATA_PATH_NAMES;
use crate::permissions::ReadDenyMatcher;
use crate::permissions::default_read_only_subpaths_for_writable_root;
use crate::permissions::project_roots_glob_pattern;

/// A policy cannot be intersected without weakening either input.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum PermissionIntersectionError {
    #[error("externally enforced filesystem permissions cannot be intersected safely")]
    ExternalSandbox,
    #[error("platform-default filesystem permissions cannot be intersected safely")]
    PlatformDefaults,
    #[error("unsupported permission path: {0}")]
    UnsupportedPath(String),
}

/// Intersects already-effective filesystem permissions and network access.
///
/// Both profiles must already be materialized for the same local executor and
/// cwd. Concrete grant paths are canonicalized before comparison and in the
/// result, so symlinks cannot acquire authority beyond either input.
/// Unsupported policy shapes fail closed.
pub fn intersect_effective_permission_profiles(
    authority: &PermissionProfile,
    requested: &PermissionProfile,
    cwd: &Path,
) -> Result<PermissionProfile, PermissionIntersectionError> {
    if matches!(authority, PermissionProfile::External { .. })
        || matches!(requested, PermissionProfile::External { .. })
    {
        return Err(PermissionIntersectionError::ExternalSandbox);
    }
    AbsolutePathBuf::from_absolute_path_checked(cwd)
        .map_err(|error| PermissionIntersectionError::UnsupportedPath(error.to_string()))?;
    for profile in [authority, requested] {
        for entry in profile.file_system_sandbox_policy().entries {
            match entry.path {
                FileSystemPath::Path { path } => {
                    path.to_abs_path().map_err(|error| {
                        PermissionIntersectionError::UnsupportedPath(error.to_string())
                    })?;
                }
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::ProjectRoots { .. },
                } => {
                    return Err(PermissionIntersectionError::UnsupportedPath(
                        "workspace permissions must already be materialized".to_string(),
                    ));
                }
                FileSystemPath::GlobPattern { pattern }
                    if pattern.starts_with(&project_roots_glob_pattern(Path::new(""))) =>
                {
                    return Err(PermissionIntersectionError::UnsupportedPath(pattern));
                }
                FileSystemPath::GlobPattern { .. } | FileSystemPath::Special { .. } => {}
            }
        }
    }

    let mut authority_policy = authority.file_system_sandbox_policy();
    let mut requested_policy = requested.file_system_sandbox_policy();
    let root_path = FileSystemPath::Special {
        value: FileSystemSpecialPath::Root,
    };
    let network = if authority.network_sandbox_policy().is_enabled()
        && requested.network_sandbox_policy().is_enabled()
    {
        NetworkSandboxPolicy::Enabled
    } else {
        NetworkSandboxPolicy::Restricted
    };
    if matches!(authority, PermissionProfile::Disabled)
        && matches!(requested, PermissionProfile::Disabled)
    {
        return Ok(PermissionProfile::Disabled);
    }
    if authority_policy == requested_policy {
        return Ok(PermissionProfile::from_runtime_permissions(
            &authority_policy,
            network,
        ));
    }
    if matches!(authority_policy.kind, FileSystemSandboxKind::Unrestricted) {
        return Ok(PermissionProfile::from_runtime_permissions(
            &requested_policy,
            network,
        ));
    }
    if matches!(requested_policy.kind, FileSystemSandboxKind::Unrestricted) {
        return Ok(PermissionProfile::from_runtime_permissions(
            &authority_policy,
            network,
        ));
    }

    normalize_policy(&mut authority_policy)?;
    normalize_policy(&mut requested_policy)?;
    let tmpdir = FileSystemPath::Special {
        value: FileSystemSpecialPath::Tmpdir,
    };
    let temp_entries = [&authority_policy, &requested_policy].map(|policy| {
        policy
            .entries
            .iter()
            .filter(|entry| entry.path == tmpdir)
            .collect::<Vec<_>>()
    });
    let common_temp = match (temp_entries[0].as_slice(), temp_entries[1].as_slice()) {
        ([left], [right])
            if [left, right]
                .into_iter()
                .all(|entry| entry.access.can_write() && !entry.skips_missing_path()) =>
        {
            Some(FileSystemSandboxEntry::new(tmpdir.clone(), left.access))
        }
        ([], []) => None,
        ([entry], []) | ([], [entry])
            if entry.access.can_write() && !entry.skips_missing_path() =>
        {
            None
        }
        _ => {
            return Err(PermissionIntersectionError::UnsupportedPath(
                ":tmpdir restrictions require executor-local bindings".to_string(),
            ));
        }
    };
    for policy in [&mut authority_policy, &mut requested_policy] {
        policy.entries.retain(|entry| entry.path != tmpdir);
    }

    let authority_denies = ReadDenyMatcher::try_new(&authority_policy, cwd)
        .map_err(PermissionIntersectionError::UnsupportedPath)?;
    let requested_denies = ReadDenyMatcher::try_new(&requested_policy, cwd)
        .map_err(PermissionIntersectionError::UnsupportedPath)?;
    let sources = [
        (&authority_policy, authority_denies.as_ref()),
        (&requested_policy, requested_denies.as_ref()),
    ];
    let mut paths = Vec::new();
    let mut denies = Vec::new();
    let mut glob_depth: Option<Option<usize>> = None;
    for (policy, _) in sources {
        let writable_roots = policy.get_writable_roots_with_cwd(cwd);
        for entry in &policy.entries {
            if entry.access == FileSystemAccessMode::Deny && !denies.contains(entry) {
                denies.push(entry.clone());
            }
            if matches!(entry.path, FileSystemPath::GlobPattern { .. }) {
                glob_depth = Some(match glob_depth {
                    None => policy.glob_scan_max_depth,
                    Some(Some(previous)) => {
                        policy.glob_scan_max_depth.map(|depth| previous.max(depth))
                    }
                    Some(None) => None,
                });
            }
            let Some(path) = entry_path(entry, cwd)? else {
                continue;
            };
            if entry.skips_missing_path()
                && (entry.access != FileSystemAccessMode::Read
                    || !writable_roots.iter().any(|root| {
                        root.read_only_subpaths.contains(&path)
                            || default_read_only_subpaths_for_writable_root(
                                &root.root, /*protect_missing_dot_codex*/ false,
                            )
                            .contains(&path)
                            || path.parent().as_ref() == Some(&root.root)
                                && PROTECTED_METADATA_PATH_NAMES.iter().any(|name| {
                                    path.as_path().file_name() == Some(std::ffi::OsStr::new(name))
                                })
                    }))
            {
                return Err(PermissionIntersectionError::UnsupportedPath(
                    "non-metadata optional permissions".to_string(),
                ));
            }
            paths.push(path);
        }
        paths.extend(policy.get_readable_roots_with_cwd(cwd));
        for root in writable_roots {
            paths.push(root.root.clone());
            paths.extend(
                PROTECTED_METADATA_PATH_NAMES
                    .iter()
                    .map(|name| root.root.join(name)),
            );
            paths.extend(root.read_only_subpaths);
        }
    }
    paths.sort_by(|left, right| {
        left.as_path()
            .components()
            .count()
            .cmp(&right.as_path().components().count())
            .then_with(|| left.as_path().cmp(right.as_path()))
    });
    paths.dedup();
    denies.sort_by_cached_key(|entry| format!("{:?}", entry.path));
    let mut intersection = FileSystemSandboxPolicy::restricted(denies);
    intersection.glob_scan_max_depth = glob_depth.flatten();
    let intersection_denies = ReadDenyMatcher::new(&intersection, cwd);

    for path in paths {
        let [left, right] =
            sources.map(|(policy, denies)| effective_access(policy, denies, path.as_path(), cwd));
        let access = intersect_access(left, right);
        let current = effective_access(
            &intersection,
            intersection_denies.as_ref(),
            path.as_path(),
            cwd,
        );
        let optional_metadata = sources.into_iter().any(|(policy, _)| {
            policy.entries.iter().any(|entry| {
                entry.skips_missing_path()
                    && entry_path(entry, cwd).ok().flatten().as_ref() == Some(&path)
            })
        });
        let required_restriction = sources.into_iter().any(|(policy, _)| {
            policy.entries.iter().any(|entry| {
                !entry.skips_missing_path()
                    && !entry.access.can_write()
                    && entry_path(entry, cwd).ok().flatten().as_ref() == Some(&path)
            })
        });
        if access == current && !optional_metadata && !required_restriction {
            continue;
        }
        let path = if path.as_path().parent().is_none()
            && cwd.starts_with(path.as_path())
            && sources
                .into_iter()
                .all(|(policy, _)| policy.entries.iter().any(|entry| entry.path == root_path))
        {
            root_path.clone()
        } else {
            path.into()
        };
        let entry =
            if optional_metadata && !required_restriction && access == FileSystemAccessMode::Read {
                FileSystemSandboxEntry::skip_missing_path(path, access)
            } else {
                FileSystemSandboxEntry::new(path, access)
            };
        if !intersection.entries.contains(&entry) {
            intersection.entries.push(entry);
        }
    }
    if let Some(temp) = common_temp {
        intersection.entries.push(temp);
    }
    Ok(PermissionProfile::from_runtime_permissions(
        &intersection,
        network,
    ))
}

fn normalize_policy(
    policy: &mut FileSystemSandboxPolicy,
) -> Result<(), PermissionIntersectionError> {
    if policy.include_platform_defaults() {
        return Err(PermissionIntersectionError::PlatformDefaults);
    }
    for entry in &mut policy.entries {
        match &entry.path {
            FileSystemPath::Path { path } => {
                let path = path.to_abs_path().map_err(|error| {
                    PermissionIntersectionError::UnsupportedPath(error.to_string())
                })?;
                let physical = if entry.access.can_write()
                    || entry.access == FileSystemAccessMode::Read && !entry.skips_missing_path()
                {
                    path.canonicalize().map_err(|error| {
                        PermissionIntersectionError::UnsupportedPath(format!(
                            "{}: {error}",
                            path.as_path().display()
                        ))
                    })?
                } else {
                    physical_path(&path)?
                };
                let logical = path.ancestors().find_map(|ancestor| {
                    std::fs::symlink_metadata(ancestor.as_path()).ok()?;
                    let preserved = canonicalize_preserving_symlinks(ancestor.as_path()).ok()?;
                    let suffix = path.as_path().strip_prefix(ancestor.as_path()).ok()?;
                    Some(
                        AbsolutePathBuf::from_absolute_path_checked(preserved)
                            .ok()?
                            .join(suffix),
                    )
                });
                if !entry.access.can_write() && logical.as_ref() != Some(&physical) {
                    return Err(PermissionIntersectionError::UnsupportedPath(format!(
                        "symlinked restriction: {}",
                        path.as_path().display()
                    )));
                }
                entry.path = physical.into();
            }
            FileSystemPath::GlobPattern { pattern }
                if entry.access != FileSystemAccessMode::Deny || entry.skips_missing_path() =>
            {
                return Err(PermissionIntersectionError::UnsupportedPath(format!(
                    "glob permissions: {pattern}"
                )));
            }
            FileSystemPath::GlobPattern { .. } => {}
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Root | FileSystemSpecialPath::Tmpdir,
            } => {}
            FileSystemPath::Special {
                value: FileSystemSpecialPath::SlashTmp,
            } => {
                if cfg!(unix) {
                    entry.path = physical_path(
                        &AbsolutePathBuf::from_absolute_path_checked("/tmp").map_err(|error| {
                            PermissionIntersectionError::UnsupportedPath(error.to_string())
                        })?,
                    )?
                    .into();
                }
            }
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Minimal,
            } => return Err(PermissionIntersectionError::PlatformDefaults),
            FileSystemPath::Special { value } => {
                return Err(PermissionIntersectionError::UnsupportedPath(format!(
                    "{value:?}"
                )));
            }
        }
    }
    Ok(())
}

fn entry_path(
    entry: &FileSystemSandboxEntry,
    cwd: &Path,
) -> Result<Option<AbsolutePathBuf>, PermissionIntersectionError> {
    match &entry.path {
        FileSystemPath::Path { path } => path
            .to_abs_path()
            .map(Some)
            .map_err(|error| PermissionIntersectionError::UnsupportedPath(error.to_string())),
        FileSystemPath::Special {
            value: FileSystemSpecialPath::Root,
        } => cwd
            .ancestors()
            .last()
            .and_then(|root| AbsolutePathBuf::from_absolute_path_checked(root).ok())
            .map(Some)
            .ok_or_else(|| PermissionIntersectionError::UnsupportedPath(cwd.display().to_string())),
        FileSystemPath::Special {
            value: FileSystemSpecialPath::Tmpdir | FileSystemSpecialPath::SlashTmp,
        } => Ok(None),
        FileSystemPath::Special { value } => Err(PermissionIntersectionError::UnsupportedPath(
            format!("{value:?}"),
        )),
        FileSystemPath::GlobPattern { .. } => Ok(None),
    }
}

fn physical_path(path: &AbsolutePathBuf) -> Result<AbsolutePathBuf, PermissionIntersectionError> {
    for ancestor in path.ancestors() {
        if let Ok(physical) = ancestor.canonicalize()
            && let Ok(suffix) = path.as_path().strip_prefix(ancestor.as_path())
        {
            return Ok(physical.join(suffix));
        }
    }
    Err(PermissionIntersectionError::UnsupportedPath(
        path.as_path().display().to_string(),
    ))
}

fn effective_access(
    policy: &FileSystemSandboxPolicy,
    denies: Option<&ReadDenyMatcher>,
    path: &Path,
    cwd: &Path,
) -> FileSystemAccessMode {
    if denies.is_some_and(|matcher| matcher.is_read_denied(path)) {
        FileSystemAccessMode::Deny
    } else if policy.can_write_path_with_cwd(path, cwd) {
        FileSystemAccessMode::Write
    } else if policy.can_read_path_with_cwd(path, cwd) {
        FileSystemAccessMode::Read
    } else {
        FileSystemAccessMode::Deny
    }
}

fn intersect_access(
    left: FileSystemAccessMode,
    right: FileSystemAccessMode,
) -> FileSystemAccessMode {
    match (left, right) {
        (FileSystemAccessMode::Deny, _) | (_, FileSystemAccessMode::Deny) => {
            FileSystemAccessMode::Deny
        }
        (FileSystemAccessMode::Write, FileSystemAccessMode::Write) => FileSystemAccessMode::Write,
        (FileSystemAccessMode::Read | FileSystemAccessMode::Write, _) => FileSystemAccessMode::Read,
    }
}

#[cfg(test)]
#[path = "permission_profile_intersection_tests.rs"]
mod tests;
