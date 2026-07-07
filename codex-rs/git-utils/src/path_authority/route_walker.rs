use std::collections::BTreeSet;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use codex_utils_absolute_path::AbsolutePathBuf;

#[cfg(windows)]
use super::windows_path::windows_path_is_ambiguous;

#[derive(Clone, Debug)]
pub(super) struct RawRouteObservation {
    pub(super) spelling: PathBuf,
    pub(super) normalized_prefixes: Vec<PathBuf>,
    pub(super) normalized: PathBuf,
    pub(super) projected: PathBuf,
}

#[derive(Clone, Debug)]
pub(super) struct SymlinkRouteObservation {
    pub(super) entry: PathBuf,
    pub(super) parent: PathBuf,
    pub(super) target: RawRouteObservation,
    pub(super) projected: RawRouteObservation,
}

#[derive(Clone, Debug)]
pub(super) struct RouteObservationSnapshot {
    pub(super) raw: RawRouteObservation,
    pub(super) symlink_hops: Vec<SymlinkRouteObservation>,
}

impl RouteObservationSnapshot {
    pub(super) fn observed_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        append_raw_observation_paths(&mut paths, &self.raw);
        for hop in &self.symlink_hops {
            push_unique_spelling(&mut paths, hop.parent.clone());
            push_unique_spelling(&mut paths, hop.entry.clone());
            append_raw_observation_paths(&mut paths, &hop.target);
            append_raw_observation_paths(&mut paths, &hop.projected);
        }
        paths
    }
}

#[derive(Clone, Debug)]
struct SymlinkRouteHop {
    entry: PathBuf,
    parent: PathBuf,
    target: PathBuf,
    projected: PathBuf,
}

pub(super) fn observe_route(path: &Path) -> io::Result<RouteObservationSnapshot> {
    let raw = observe_raw_route(path)?;
    let symlink_hops = symlink_route_hops(path)?
        .into_iter()
        .map(|hop| {
            Ok(SymlinkRouteObservation {
                entry: hop.entry,
                parent: hop.parent,
                target: observe_raw_route(&hop.target)?,
                projected: observe_raw_route(&hop.projected)?,
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    Ok(RouteObservationSnapshot { raw, symlink_hops })
}

fn observe_raw_route(route: &Path) -> io::Result<RawRouteObservation> {
    let base = route
        .ancestors()
        .last()
        .ok_or_else(|| invalid_data("authority path has no root"))?;
    let normalized = AbsolutePathBuf::resolve_path_against_base(route, base).into_path_buf();
    let normalized_prefixes = route
        .ancestors()
        .map(|prefix| AbsolutePathBuf::resolve_path_against_base(prefix, base).into_path_buf())
        .collect();
    let projected = project_through_longest_existing_ancestor(route)?;
    Ok(RawRouteObservation {
        spelling: route.to_path_buf(),
        normalized_prefixes,
        normalized,
        projected,
    })
}

fn append_raw_observation_paths(paths: &mut Vec<PathBuf>, observation: &RawRouteObservation) {
    for prefix in &observation.normalized_prefixes {
        push_unique_spelling(paths, prefix.clone());
    }
    push_unique_spelling(paths, observation.projected.clone());
}

fn push_unique_spelling(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

fn symlink_route_hops(path: &Path) -> io::Result<Vec<SymlinkRouteHop>> {
    let mut hops = Vec::new();
    let mut seen = BTreeSet::new();
    collect_symlink_route_hops(path, /*depth*/ 0, &mut seen, &mut hops)?;
    Ok(hops)
}

/// Detect Linux procfs routes whose target can differ in a later Git process.
///
/// Existing procfs-hosted symlinks are process-dependent, and missing procfs
/// descendants can materialize when the Git child's PID or descriptors exist.
pub(super) fn route_contains_process_relative_procfs_path(path: &Path) -> io::Result<bool> {
    #[cfg(target_os = "linux")]
    {
        let hops = symlink_route_hops(path)?;
        for hop in &hops {
            if symlink_is_on_procfs(&hop.entry)? {
                return Ok(true);
            }
        }
        if missing_descendant_is_below_procfs(path)? {
            return Ok(true);
        }
        for hop in hops {
            if missing_descendant_is_below_procfs(&hop.target)?
                || missing_descendant_is_below_procfs(&hop.projected)?
            {
                return Ok(true);
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = path;
    Ok(false)
}

#[cfg(target_os = "linux")]
fn symlink_is_on_procfs(path: &Path) -> io::Result<bool> {
    filesystem_is_procfs(
        path,
        rustix::fs::OFlags::PATH | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
    )
}

#[cfg(target_os = "linux")]
fn missing_descendant_is_below_procfs(path: &Path) -> io::Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => return Ok(false),
        Err(error) if is_missing_path_error(&error) => {}
        Err(error) => return Err(error),
    }
    for ancestor in path.ancestors().skip(1) {
        match std::fs::symlink_metadata(ancestor) {
            Ok(_) => match filesystem_is_procfs(
                ancestor,
                rustix::fs::OFlags::PATH | rustix::fs::OFlags::CLOEXEC,
            ) {
                Ok(is_procfs) => return Ok(is_procfs),
                Err(error) if is_missing_path_error(&error) => {}
                Err(error) => return Err(error),
            },
            Err(error) if is_missing_path_error(&error) => {}
            Err(error) => return Err(error),
        }
    }
    Err(invalid_data("authority path has no existing ancestor"))
}

#[cfg(target_os = "linux")]
fn filesystem_is_procfs(path: &Path, flags: rustix::fs::OFlags) -> io::Result<bool> {
    let fd = rustix::fs::open(path, flags, rustix::fs::Mode::empty()).map_err(io::Error::from)?;
    let filesystem = rustix::fs::fstatfs(&fd).map_err(io::Error::from)?;
    Ok(filesystem.f_type == rustix::fs::PROC_SUPER_MAGIC)
}

#[cfg(target_os = "linux")]
fn is_missing_path_error(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
    )
}

fn collect_symlink_route_hops(
    path: &Path,
    depth: usize,
    seen: &mut BTreeSet<PathBuf>,
    hops: &mut Vec<SymlinkRouteHop>,
) -> io::Result<()> {
    if depth > 40 {
        return Err(invalid_data("authority path symlink cycle"));
    }
    if !seen.insert(path.to_path_buf()) {
        return Ok(());
    }
    for ancestor in path.ancestors() {
        match std::fs::symlink_metadata(ancestor) {
            Ok(_) => match read_link_if_symlink(ancestor) {
                Ok(Some(target)) => {
                    let parent = ancestor
                        .parent()
                        .ok_or_else(|| invalid_data("authority symlink has no parent"))?;
                    let target = resolve_literal_path(target, parent);
                    #[cfg(windows)]
                    if target.to_str().is_none_or(windows_path_is_ambiguous) {
                        return Err(invalid_data("ambiguous Windows authority symlink"));
                    }
                    let suffix = path
                        .strip_prefix(ancestor)
                        .map_err(|_| invalid_data("failed to project authority symlink path"))?;
                    let projected = resolve_literal_path(suffix, &target);
                    hops.push(SymlinkRouteHop {
                        entry: ancestor.to_path_buf(),
                        parent: parent.to_path_buf(),
                        target,
                        projected: projected.clone(),
                    });
                    collect_symlink_route_hops(&projected, depth + 1, seen, hops)?;
                }
                Ok(None) => {}
                Err(error) => return Err(error),
            },
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                ) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn read_link_if_symlink(path: &Path) -> io::Result<Option<PathBuf>> {
    match std::fs::read_link(path) {
        Ok(target) => Ok(Some(target)),
        Err(error) if read_link_error_means_not_symlink(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

fn read_link_error_means_not_symlink(error: &io::Error) -> bool {
    #[cfg(windows)]
    {
        // Win32 reports this for ordinary files and directories passed to
        // `std::fs::read_link`; it is the Windows equivalent of EINVAL here.
        const ERROR_NOT_A_REPARSE_POINT: i32 = 4390;
        error.raw_os_error() == Some(ERROR_NOT_A_REPARSE_POINT)
    }
    #[cfg(not(windows))]
    {
        error.kind() == io::ErrorKind::InvalidInput
    }
}

fn project_through_longest_existing_ancestor(path: &Path) -> io::Result<PathBuf> {
    project_path(path, /*symlink_depth*/ 0)
}

fn project_path(path: &Path, symlink_depth: usize) -> io::Result<PathBuf> {
    if symlink_depth > 40 {
        return Err(invalid_data("too many authority path symlinks"));
    }
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return Ok(canonical);
    }
    for ancestor in path.ancestors() {
        if std::fs::symlink_metadata(ancestor).is_ok()
            && let Ok(target) = std::fs::read_link(ancestor)
        {
            let parent = ancestor
                .parent()
                .ok_or_else(|| invalid_data("authority symlink has no parent"))?;
            let target = resolve_literal_path(target, parent);
            #[cfg(windows)]
            if target.to_str().is_none_or(windows_path_is_ambiguous) {
                return Err(invalid_data("ambiguous Windows authority symlink"));
            }
            let suffix = path
                .strip_prefix(ancestor)
                .map_err(|_| invalid_data("failed to project authority path"))?;
            return project_path(&resolve_literal_path(suffix, &target), symlink_depth + 1);
        }
    }
    for ancestor in path.ancestors() {
        match std::fs::canonicalize(ancestor) {
            Ok(canonical) => {
                let suffix = path
                    .strip_prefix(ancestor)
                    .map_err(|_| invalid_data("failed to project authority path"))?;
                return Ok(
                    AbsolutePathBuf::resolve_path_against_base(suffix, canonical).into_path_buf(),
                );
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                ) => {}
            Err(error) => return Err(error),
        }
    }
    Err(invalid_data("authority path has no existing ancestor"))
}

fn resolve_literal_path(path: impl AsRef<Path>, base: &Path) -> PathBuf {
    let path = path.as_ref();
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

pub(super) fn invalid_data(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
#[path = "route_walker_tests.rs"]
mod tests;
