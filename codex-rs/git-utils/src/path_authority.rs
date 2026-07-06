use std::io;
use std::path::Path;
use std::path::PathBuf;

use same_file::Handle;

use crate::git_config::path_is_within;

mod cancellation;
mod route_walker;
#[cfg(any(windows, test))]
mod windows_path;

use route_walker::RawRouteObservation;
use route_walker::RouteObservationSnapshot;
#[cfg(windows)]
use route_walker::invalid_data;
use route_walker::observe_route;
use route_walker::route_contains_process_relative_procfs_path;
#[cfg(any(windows, test))]
pub(crate) use windows_path::windows_authority_path_is_ambiguous;
#[cfg(windows)]
pub(crate) use windows_path::windows_path_is_ambiguous;

#[derive(Clone, Copy, Eq, PartialEq)]
enum PathBoundary {
    Worktree,
    Metadata,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum CandidateBoundary {
    Outside,
    Metadata,
    ExactWorktree,
    Worktree,
}

#[derive(Debug, Default)]
pub(crate) struct RepositoryRouteBoundaries {
    worktree_roots: Vec<PathBuf>,
    metadata_dirs: Vec<PathBuf>,
    worktree_identities: Vec<Handle>,
    metadata_identities: Vec<Handle>,
}

pub(crate) struct RouteInspection {
    pub(crate) crosses_worktree: bool,
    pub(crate) touches_worktree: bool,
    pub(crate) crosses_metadata: bool,
    pub(crate) observed_paths: Vec<PathBuf>,
}

impl RepositoryRouteBoundaries {
    pub(crate) fn inspect_route(&self, route: &Path) -> io::Result<RouteInspection> {
        inspect_route(route, self)
    }

    pub(crate) fn contains_known_boundary(&self, path: &Path) -> io::Result<bool> {
        Ok(classify_candidate(
            path,
            &self.worktree_roots,
            &self.metadata_dirs,
            &self.worktree_identities,
            &self.metadata_identities,
        )? != CandidateBoundary::Outside)
    }

    pub(crate) fn route_contains_process_relative_procfs_path(
        &self,
        route: &Path,
    ) -> io::Result<bool> {
        route_contains_process_relative_procfs_path(route)
    }
}

pub(crate) fn repository_route_boundaries(
    roots: &[PathBuf],
    common_dirs: &[PathBuf],
) -> io::Result<RepositoryRouteBoundaries> {
    let mut worktree_roots = Vec::new();
    for root in roots {
        push_unique_location(&mut worktree_roots, root.clone())?;
    }
    let worktree_identities = directory_identities(&worktree_roots)?;

    let mut metadata_dirs = Vec::new();
    for root in &worktree_roots {
        let marker = root.join(".git");
        let Ok(metadata) = std::fs::symlink_metadata(&marker) else {
            continue;
        };
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            continue;
        }
        let canonical_root = std::fs::canonicalize(root)?;
        let canonical_marker = std::fs::canonicalize(&marker)?;
        if canonical_marker == canonical_root.join(".git") {
            push_unique_location(&mut metadata_dirs, marker)?;
        }
    }
    for common in common_dirs {
        let within_standard_metadata = metadata_dirs
            .iter()
            .any(|metadata| path_is_within(common, metadata));
        let within_worktree = worktree_roots
            .iter()
            .any(|root| path_is_within(common, root))
            || nearest_identity_boundary(common, &worktree_identities, &[])?
                == Some(PathBoundary::Worktree);
        if within_standard_metadata || !within_worktree {
            push_unique_location(&mut metadata_dirs, common.clone())?;
        }
    }

    let metadata_identities = directory_identities(&metadata_dirs)?;
    Ok(RepositoryRouteBoundaries {
        worktree_roots,
        metadata_dirs,
        worktree_identities,
        metadata_identities,
    })
}

fn push_unique_location(paths: &mut Vec<PathBuf>, path: PathBuf) -> io::Result<()> {
    for existing in paths.iter() {
        if paths_refer_to_same_location(existing, &path)? {
            return Ok(());
        }
    }
    paths.push(path);
    Ok(())
}

fn paths_refer_to_same_location(left: &Path, right: &Path) -> io::Result<bool> {
    if path_is_within(left, right) && path_is_within(right, left) {
        return Ok(true);
    }
    if let (Ok(left), Ok(right)) = (Handle::from_path(left), Handle::from_path(right)) {
        return Ok(left == right);
    }
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => Ok(left == right),
        (Err(left), _) if is_missing(&left) => Ok(false),
        (_, Err(right)) if is_missing(&right) => Ok(false),
        (Err(error), _) | (_, Err(error)) => Err(error),
    }
}

fn is_missing(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
    )
}

fn inspect_route(
    route: &Path,
    boundaries: &RepositoryRouteBoundaries,
) -> io::Result<RouteInspection> {
    #[cfg(windows)]
    {
        let route = route
            .to_str()
            .ok_or_else(|| invalid_data("non-UTF-8 Windows authority path"))?;
        if windows_path_is_ambiguous(route) {
            return Ok(RouteInspection {
                crosses_worktree: true,
                touches_worktree: true,
                crosses_metadata: true,
                observed_paths: Vec::new(),
            });
        }
    }

    let observation = observe_route(route)?;
    let crosses_worktree = observation_crosses_worktree(
        &observation,
        &boundaries.worktree_roots,
        &boundaries.metadata_dirs,
        &boundaries.worktree_identities,
        &boundaries.metadata_identities,
    )?;
    let observed_paths = observation.observed_paths();
    let mut touches_worktree = false;
    let mut crosses_metadata = false;
    for path in &observed_paths {
        match classify_candidate(
            path,
            &boundaries.worktree_roots,
            &boundaries.metadata_dirs,
            &boundaries.worktree_identities,
            &boundaries.metadata_identities,
        )? {
            CandidateBoundary::Worktree | CandidateBoundary::ExactWorktree => {
                touches_worktree = true;
            }
            CandidateBoundary::Metadata => crosses_metadata = true,
            CandidateBoundary::Outside => {}
        }
    }
    Ok(RouteInspection {
        crosses_worktree,
        touches_worktree,
        crosses_metadata,
        observed_paths,
    })
}

fn observation_crosses_worktree(
    observation: &RouteObservationSnapshot,
    worktree_roots: &[PathBuf],
    metadata_dirs: &[PathBuf],
    worktree_identities: &[Handle],
    metadata_identities: &[Handle],
) -> io::Result<bool> {
    let raw_pivot = raw_observation_has_worktree_descendant_pivot(
        &observation.raw,
        worktree_roots,
        metadata_dirs,
        worktree_identities,
        metadata_identities,
    )?;
    let normalized_crosses = strict_candidate_crosses(
        &observation.raw.normalized,
        worktree_roots,
        metadata_dirs,
        worktree_identities,
        metadata_identities,
    )?;
    let projected_crosses = strict_candidate_crosses(
        &observation.raw.projected,
        worktree_roots,
        metadata_dirs,
        worktree_identities,
        metadata_identities,
    )?;
    if raw_pivot || normalized_crosses || projected_crosses {
        return Ok(true);
    }
    for hop in &observation.symlink_hops {
        // A symlink entry located directly under an exact worktree root is
        // already a mutable descendant; classify its parent strictly.
        if strict_candidate_crosses(
            &hop.parent,
            worktree_roots,
            metadata_dirs,
            worktree_identities,
            metadata_identities,
        )? || intermediate_candidate_crosses(
            &hop.entry,
            worktree_roots,
            metadata_dirs,
            worktree_identities,
            metadata_identities,
        )? || raw_observation_has_worktree_descendant_pivot(
            &hop.target,
            worktree_roots,
            metadata_dirs,
            worktree_identities,
            metadata_identities,
        )? || intermediate_observation_crosses(
            &hop.target,
            worktree_roots,
            metadata_dirs,
            worktree_identities,
            metadata_identities,
        )? || raw_observation_has_worktree_descendant_pivot(
            &hop.projected,
            worktree_roots,
            metadata_dirs,
            worktree_identities,
            metadata_identities,
        )? || terminal_observation_crosses(
            &hop.projected,
            worktree_roots,
            metadata_dirs,
            worktree_identities,
            metadata_identities,
        )? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn intermediate_observation_crosses(
    observation: &RawRouteObservation,
    worktree_roots: &[PathBuf],
    metadata_dirs: &[PathBuf],
    worktree_identities: &[Handle],
    metadata_identities: &[Handle],
) -> io::Result<bool> {
    Ok(intermediate_candidate_crosses(
        &observation.normalized,
        worktree_roots,
        metadata_dirs,
        worktree_identities,
        metadata_identities,
    )? || intermediate_candidate_crosses(
        &observation.projected,
        worktree_roots,
        metadata_dirs,
        worktree_identities,
        metadata_identities,
    )?)
}

fn terminal_observation_crosses(
    observation: &RawRouteObservation,
    worktree_roots: &[PathBuf],
    metadata_dirs: &[PathBuf],
    worktree_identities: &[Handle],
    metadata_identities: &[Handle],
) -> io::Result<bool> {
    Ok(strict_candidate_crosses(
        &observation.normalized,
        worktree_roots,
        metadata_dirs,
        worktree_identities,
        metadata_identities,
    )? || strict_candidate_crosses(
        &observation.projected,
        worktree_roots,
        metadata_dirs,
        worktree_identities,
        metadata_identities,
    )?)
}

fn strict_candidate_crosses(
    path: &Path,
    worktree_roots: &[PathBuf],
    metadata_dirs: &[PathBuf],
    worktree_identities: &[Handle],
    metadata_identities: &[Handle],
) -> io::Result<bool> {
    Ok(matches!(
        classify_candidate(
            path,
            worktree_roots,
            metadata_dirs,
            worktree_identities,
            metadata_identities,
        )?,
        CandidateBoundary::ExactWorktree | CandidateBoundary::Worktree
    ))
}

fn intermediate_candidate_crosses(
    path: &Path,
    worktree_roots: &[PathBuf],
    metadata_dirs: &[PathBuf],
    worktree_identities: &[Handle],
    metadata_identities: &[Handle],
) -> io::Result<bool> {
    // Exact roots remain valid intermediates for Git-generated active metadata
    // routes such as a nested submodule's `nested/../.git/modules/...`.
    Ok(classify_candidate(
        path,
        worktree_roots,
        metadata_dirs,
        worktree_identities,
        metadata_identities,
    )? == CandidateBoundary::Worktree)
}

fn raw_observation_has_worktree_descendant_pivot(
    observation: &RawRouteObservation,
    worktree_roots: &[PathBuf],
    metadata_dirs: &[PathBuf],
    worktree_identities: &[Handle],
    metadata_identities: &[Handle],
) -> io::Result<bool> {
    for prefix in &observation.normalized_prefixes {
        // Active metadata routes are revalidated before each child, so an
        // exact nested root is allowed here. Retained registry and standard
        // directory routes use the stricter role-aware cancellation policy.
        let boundary = classify_candidate(
            prefix,
            worktree_roots,
            metadata_dirs,
            worktree_identities,
            metadata_identities,
        )?;
        if boundary == CandidateBoundary::Worktree {
            return Ok(true);
        }
    }
    Ok(false)
}

fn classify_candidate(
    path: &Path,
    worktree_roots: &[PathBuf],
    metadata_dirs: &[PathBuf],
    worktree_identities: &[Handle],
    metadata_identities: &[Handle],
) -> io::Result<CandidateBoundary> {
    if metadata_dirs
        .iter()
        .any(|metadata| path_is_within(path, metadata))
    {
        return Ok(CandidateBoundary::Metadata);
    }
    if worktree_roots
        .iter()
        .any(|root| path_is_within(path, root) && path_is_within(root, path))
    {
        return Ok(CandidateBoundary::ExactWorktree);
    }
    if worktree_roots.iter().any(|root| path_is_within(path, root)) {
        return Ok(CandidateBoundary::Worktree);
    }
    for (depth, ancestor) in path.ancestors().enumerate() {
        let metadata = match std::fs::metadata(ancestor) {
            Ok(metadata) if metadata.is_dir() => metadata,
            Ok(_) => continue,
            Err(error) if is_missing(&error) => continue,
            Err(error) => return Err(error),
        };
        let identity = Handle::from_path(ancestor)?;
        if metadata_identities.contains(&identity) {
            return Ok(CandidateBoundary::Metadata);
        }
        if worktree_identities.contains(&identity) {
            return Ok(if depth == 0 && metadata.is_dir() {
                CandidateBoundary::ExactWorktree
            } else {
                CandidateBoundary::Worktree
            });
        }
    }
    Ok(CandidateBoundary::Outside)
}

fn directory_identities(paths: &[PathBuf]) -> io::Result<Vec<Handle>> {
    paths
        .iter()
        .filter_map(|path| match std::fs::metadata(path) {
            Ok(metadata) if metadata.is_dir() => Some(Handle::from_path(path)),
            Ok(_) => None,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                ) =>
            {
                None
            }
            Err(error) => Some(Err(error)),
        })
        .collect()
}

fn nearest_identity_boundary(
    path: &Path,
    worktree_identities: &[Handle],
    metadata_identities: &[Handle],
) -> io::Result<Option<PathBoundary>> {
    for ancestor in path.ancestors() {
        let metadata = match std::fs::metadata(ancestor) {
            Ok(metadata) => metadata,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                ) =>
            {
                continue;
            }
            Err(error) => return Err(error),
        };
        if !metadata.is_dir() {
            continue;
        }
        let identity = Handle::from_path(ancestor)?;
        if metadata_identities.contains(&identity) {
            return Ok(Some(PathBoundary::Metadata));
        }
        if worktree_identities.contains(&identity) {
            return Ok(Some(PathBoundary::Worktree));
        }
    }
    Ok(None)
}
