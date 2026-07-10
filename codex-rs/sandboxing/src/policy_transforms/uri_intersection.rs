use std::num::NonZeroUsize;

use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::NetworkPermissions;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::LegacyAppPathString;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;

use super::access_covers;
use super::intersect_permission_profiles;
use super::merge_glob_scan_max_depth;

const PROJECT_ROOTS_GLOB_PATTERN_PREFIX: &str = "codex-project-roots://";

/// Intersects permission profiles without projecting foreign paths onto the host.
///
/// Host-native profiles use the established native implementation. The URI
/// fallback only accepts grants whose coverage can be proven lexically and
/// drops the filesystem result when a narrowing constraint is ambiguous.
pub fn intersect_permission_profiles_for_uri(
    requested: AdditionalPermissionProfile<PathUri>,
    granted: AdditionalPermissionProfile<PathUri>,
    cwd: &PathUri,
) -> AdditionalPermissionProfile<PathUri> {
    if let Some(intersected) = intersect_native_profiles(requested.clone(), granted.clone(), cwd) {
        return intersected;
    }

    let network = intersect_network_permissions(requested.network, granted.network);
    let file_system =
        intersect_foreign_file_system_permissions(requested.file_system, granted.file_system, cwd);
    AdditionalPermissionProfile {
        network,
        file_system,
    }
}

fn intersect_native_profiles(
    requested: AdditionalPermissionProfile<PathUri>,
    granted: AdditionalPermissionProfile<PathUri>,
    cwd: &PathUri,
) -> Option<AdditionalPermissionProfile<PathUri>> {
    let cwd = cwd.to_abs_path().ok()?;
    let requested = AdditionalPermissionProfile::<AbsolutePathBuf>::try_from(requested).ok()?;
    let granted = AdditionalPermissionProfile::<AbsolutePathBuf>::try_from(granted).ok()?;
    Some(intersect_permission_profiles(requested, granted, cwd.as_path()).into())
}

fn intersect_network_permissions(
    requested: Option<NetworkPermissions>,
    granted: Option<NetworkPermissions>,
) -> Option<NetworkPermissions> {
    match (requested, granted) {
        (
            Some(NetworkPermissions {
                enabled: Some(true),
            }),
            Some(NetworkPermissions {
                enabled: Some(true),
            }),
        ) => Some(NetworkPermissions {
            enabled: Some(true),
        }),
        _ => None,
    }
}

fn intersect_foreign_file_system_permissions(
    requested: Option<FileSystemPermissions<PathUri>>,
    granted: Option<FileSystemPermissions<PathUri>>,
    cwd: &PathUri,
) -> Option<FileSystemPermissions<PathUri>> {
    let requested = requested?;
    let granted = granted.unwrap_or_default();
    let mut entries = granted
        .entries
        .iter()
        .filter_map(|entry| materialize_covered_grant(entry, &requested.entries, cwd))
        .fold(Vec::new(), |mut entries, entry| {
            if !entries.contains(&entry) {
                entries.push(entry);
            }
            entries
        });
    if entries.is_empty() {
        return None;
    }

    let requested_constraints =
        materialize_narrowing_constraints(&requested.entries, &entries, cwd)?;
    let granted_constraints = materialize_narrowing_constraints(&granted.entries, &entries, cwd)?;
    let glob_scan_max_depth = merge_glob_scan_max_depth(
        &requested_constraints,
        requested.glob_scan_max_depth.map(usize::from),
        &granted_constraints,
        granted.glob_scan_max_depth.map(usize::from),
    )
    .and_then(NonZeroUsize::new);
    for entry in requested_constraints.into_iter().chain(granted_constraints) {
        if !entries.contains(&entry) {
            entries.push(entry);
        }
    }

    Some(FileSystemPermissions {
        entries,
        glob_scan_max_depth,
    })
}

fn materialize_covered_grant(
    granted: &FileSystemSandboxEntry<PathUri>,
    requested: &[FileSystemSandboxEntry<PathUri>],
    cwd: &PathUri,
) -> Option<FileSystemSandboxEntry<PathUri>> {
    if !granted.access.can_read() {
        return None;
    }
    let Some(path) = resolve_permission_path_uri(&granted.path, cwd) else {
        return unresolved_grant_is_exactly_covered(granted, requested).then(|| granted.clone());
    };
    if requested_path_is_denied(requested, &path, cwd) {
        return None;
    }
    let requested_access = requested_access_for_path(requested, &path, cwd)?;
    access_covers(requested_access, granted.access).then_some(FileSystemSandboxEntry {
        path: FileSystemPath::Path { path },
        access: granted.access,
    })
}

fn unresolved_grant_is_exactly_covered(
    granted: &FileSystemSandboxEntry<PathUri>,
    requested: &[FileSystemSandboxEntry<PathUri>],
) -> bool {
    if !matches!(
        &granted.path,
        FileSystemPath::Special {
            value: FileSystemSpecialPath::Tmpdir
        }
    ) || requested
        .iter()
        .any(|entry| entry.access == FileSystemAccessMode::Deny)
    {
        return false;
    }

    requested
        .iter()
        .any(|entry| entry.path == granted.path && access_covers(entry.access, granted.access))
}

fn requested_path_is_denied(
    requested: &[FileSystemSandboxEntry<PathUri>],
    path: &PathUri,
    cwd: &PathUri,
) -> bool {
    requested
        .iter()
        .filter(|entry| entry.access == FileSystemAccessMode::Deny)
        .any(|entry| match &entry.path {
            FileSystemPath::GlobPattern { pattern } => {
                glob_may_match_path(pattern, path, cwd).unwrap_or(true)
            }
            FileSystemPath::Path { .. } | FileSystemPath::Special { .. } => {
                resolve_permission_path_uri(&entry.path, cwd).is_none_or(|denied_path| {
                    uri_containment(path, &denied_path) != UriContainment::No
                })
            }
        })
}

fn requested_access_for_path(
    requested: &[FileSystemSandboxEntry<PathUri>],
    path: &PathUri,
    cwd: &PathUri,
) -> Option<FileSystemAccessMode> {
    requested
        .iter()
        .filter_map(|entry| {
            let entry_path = resolve_permission_path_uri(&entry.path, cwd)?;
            (uri_containment(path, &entry_path) == UriContainment::Yes)
                .then(|| (entry_path.ancestors().count(), entry.access))
        })
        .max()
        .map(|(_, access)| access)
}

fn materialize_narrowing_constraints(
    source_entries: &[FileSystemSandboxEntry<PathUri>],
    accepted_entries: &[FileSystemSandboxEntry<PathUri>],
    cwd: &PathUri,
) -> Option<Vec<FileSystemSandboxEntry<PathUri>>> {
    let has_accepted_write = accepted_entries
        .iter()
        .any(|entry| entry.access.can_write());
    let mut constraints = Vec::new();
    for entry in source_entries {
        match entry.access {
            FileSystemAccessMode::Write => continue,
            FileSystemAccessMode::Read if !has_accepted_write => continue,
            FileSystemAccessMode::Read => {
                if !read_constraint_narrows_accepted_grant(entry, accepted_entries, cwd)? {
                    continue;
                }
            }
            FileSystemAccessMode::Deny => {}
        }
        let constraint = materialize_constraint(entry, cwd)?;
        if !constraints.contains(&constraint) {
            constraints.push(constraint);
        }
    }
    Some(constraints)
}

fn read_constraint_narrows_accepted_grant(
    constraint: &FileSystemSandboxEntry<PathUri>,
    accepted_entries: &[FileSystemSandboxEntry<PathUri>],
    cwd: &PathUri,
) -> Option<bool> {
    if matches!(&constraint.path, FileSystemPath::GlobPattern { .. }) {
        return None;
    }

    let constraint_path = resolve_permission_path_uri(&constraint.path, cwd);
    let mut ambiguous = false;
    for accepted in accepted_entries
        .iter()
        .filter(|entry| entry.access.can_write())
    {
        match (
            constraint_path.as_ref(),
            resolve_permission_path_uri(&accepted.path, cwd),
        ) {
            (Some(constraint_path), Some(accepted_path)) => {
                match uri_containment(constraint_path, &accepted_path) {
                    UriContainment::Yes => return Some(true),
                    UriContainment::Unknown => ambiguous = true,
                    UriContainment::No => match uri_containment(&accepted_path, constraint_path) {
                        UriContainment::Yes => {}
                        UriContainment::Unknown => ambiguous = true,
                        UriContainment::No => {
                            ambiguous |=
                                uris_share_target_namespace(constraint_path, &accepted_path);
                        }
                    },
                }
            }
            (None, None) if constraint.path == accepted.path => return Some(true),
            (None, _) | (_, None) => ambiguous = true,
        }
    }
    (!ambiguous).then_some(false)
}

fn materialize_constraint(
    entry: &FileSystemSandboxEntry<PathUri>,
    cwd: &PathUri,
) -> Option<FileSystemSandboxEntry<PathUri>> {
    let path = match &entry.path {
        FileSystemPath::Path { path } => FileSystemPath::Path { path: path.clone() },
        FileSystemPath::GlobPattern { pattern } => FileSystemPath::GlobPattern {
            pattern: materialize_glob_pattern(pattern, cwd)?,
        },
        FileSystemPath::Special {
            value: FileSystemSpecialPath::Tmpdir,
        } => entry.path.clone(),
        FileSystemPath::Special { .. } => FileSystemPath::Path {
            path: resolve_permission_path_uri(&entry.path, cwd)?,
        },
    };
    Some(FileSystemSandboxEntry {
        path,
        access: entry.access,
    })
}

fn glob_may_match_path(pattern: &str, path: &PathUri, cwd: &PathUri) -> Option<bool> {
    if !glob_pattern_is_statically_supported(pattern, cwd) {
        return None;
    }
    materialize_glob_pattern(pattern, cwd)?;
    let prefix = glob_static_prefix_uri(pattern, cwd)?;
    match uri_containment(path, &prefix) {
        UriContainment::Yes => Some(true),
        UriContainment::No => Some(false),
        UriContainment::Unknown => None,
    }
}

fn glob_pattern_is_statically_supported(pattern: &str, cwd: &PathUri) -> bool {
    let Some(convention) = cwd.infer_path_convention() else {
        return false;
    };
    let pattern = project_roots_glob_subpath(pattern).unwrap_or(pattern);
    if path_starts_with_tilde(pattern, convention)
        || convention
            .path_segments(pattern)
            .any(|component| matches!(component, "." | ".."))
    {
        return false;
    }
    !pattern.chars().any(|character| {
        matches!(character, '[' | ']' | '{' | '}' | '!')
            || (convention == PathConvention::Posix && character == '\\')
    })
}

fn glob_static_prefix_uri(pattern: &str, cwd: &PathUri) -> Option<PathUri> {
    let convention = cwd.infer_path_convention()?;
    let project_roots_glob = project_roots_glob_subpath(pattern);
    let pattern = project_roots_glob.unwrap_or(pattern);
    let meta_index = pattern.char_indices().find_map(|(index, character)| {
        matches!(character, '*' | '?' | '[' | ']' | '{' | '}').then_some(index)
    });
    let prefix = match meta_index {
        None => pattern,
        Some(index) => {
            let literal = &pattern[..index];
            if literal.ends_with(|character| is_path_separator(convention, character)) {
                literal
            } else {
                literal
                    .rfind(|character| is_path_separator(convention, character))
                    .map_or("", |separator| &literal[..=separator])
            }
        }
    };
    let prefix = cwd.join(prefix).ok()?;
    if project_roots_glob.is_some() && uri_containment(&prefix, cwd) != UriContainment::Yes {
        return None;
    }
    Some(prefix)
}

fn materialize_glob_pattern(pattern: &str, cwd: &PathUri) -> Option<String> {
    let convention = cwd.infer_path_convention()?;
    let project_roots_glob = project_roots_glob_subpath(pattern);
    let pattern = project_roots_glob.unwrap_or(pattern);
    if path_starts_with_tilde(pattern, convention) {
        return None;
    }
    let resolved = cwd.join(pattern).ok()?;
    if project_roots_glob.is_some() && uri_containment(&resolved, cwd) != UriContainment::Yes {
        return None;
    }
    let rendered = LegacyAppPathString::from_path_uri(&resolved, convention).ok()?;
    (rendered.to_path_uri(convention).ok()? == resolved).then(|| rendered.into_string())
}

fn project_roots_glob_subpath(pattern: &str) -> Option<&str> {
    pattern.strip_prefix(PROJECT_ROOTS_GLOB_PATTERN_PREFIX)
}

fn path_starts_with_tilde(path: &str, convention: PathConvention) -> bool {
    path == "~"
        || path.strip_prefix('~').is_some_and(|path| {
            path.starts_with(|character| is_path_separator(convention, character))
        })
}

fn is_path_separator(convention: PathConvention, character: char) -> bool {
    match convention {
        PathConvention::Posix => character == '/',
        PathConvention::Windows => matches!(character, '/' | '\\'),
    }
}

fn resolve_permission_path_uri(path: &FileSystemPath<PathUri>, cwd: &PathUri) -> Option<PathUri> {
    match path {
        FileSystemPath::Path { path } => Some(path.clone()),
        FileSystemPath::GlobPattern { .. } => None,
        FileSystemPath::Special { value } => match value {
            FileSystemSpecialPath::Root => path_uri_root(cwd),
            FileSystemSpecialPath::ProjectRoots { subpath } => match subpath {
                Some(subpath) => {
                    let convention = cwd.infer_path_convention()?;
                    if path_starts_with_tilde(subpath, convention) {
                        return None;
                    }
                    cwd.join(subpath)
                        .ok()
                        .filter(|path| uri_containment(path, cwd) == UriContainment::Yes)
                }
                None => Some(cwd.clone()),
            },
            FileSystemSpecialPath::SlashTmp
                if cwd.infer_path_convention() == Some(PathConvention::Posix) =>
            {
                path_uri_root(cwd)?.join("tmp").ok()
            }
            FileSystemSpecialPath::Minimal
            | FileSystemSpecialPath::Tmpdir
            | FileSystemSpecialPath::SlashTmp
            | FileSystemSpecialPath::Unknown { .. } => None,
        },
    }
}

fn path_uri_root(path: &PathUri) -> Option<PathUri> {
    let root = path.ancestors().last()?;
    match root.infer_path_convention()? {
        PathConvention::Posix
            if root.to_url().host_str().is_none() && root.encoded_path() == "/" =>
        {
            Some(root)
        }
        PathConvention::Windows if root.basename().is_some() => Some(root),
        PathConvention::Posix | PathConvention::Windows => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UriContainment {
    Yes,
    No,
    Unknown,
}

fn uri_containment(path: &PathUri, base: &PathUri) -> UriContainment {
    if path == base {
        return UriContainment::Yes;
    }
    let path_convention = path.infer_path_convention();
    let base_convention = base.infer_path_convention();
    if path_convention.is_some() && base_convention.is_some() && path_convention != base_convention
    {
        return UriContainment::No;
    }
    if path.to_url().host_str() != base.to_url().host_str() {
        return UriContainment::No;
    }
    if path_convention.is_none()
        || base_convention.is_none()
        || has_ambiguous_encoded_path(path)
        || has_ambiguous_encoded_path(base)
    {
        return UriContainment::Unknown;
    }
    if path.relative_path_from(base).is_some() {
        UriContainment::Yes
    } else {
        UriContainment::No
    }
}

fn has_ambiguous_encoded_path(path: &PathUri) -> bool {
    let encoded_path = path.encoded_path().as_bytes();
    encoded_path.windows(3).any(|bytes| {
        bytes[0] == b'%'
            && matches!(
                (bytes[1].to_ascii_lowercase(), bytes[2].to_ascii_lowercase()),
                (b'0', b'0') | (b'2', b'f') | (b'5', b'c')
            )
    })
}

fn uris_share_target_namespace(left: &PathUri, right: &PathUri) -> bool {
    left.infer_path_convention() == right.infer_path_convention()
        && left.to_url().host_str() == right.to_url().host_str()
}
