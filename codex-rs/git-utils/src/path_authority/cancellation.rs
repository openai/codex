use std::io;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use same_file::Handle;

use crate::git_config::path_is_within;

use super::CandidateBoundary;
use super::RepositoryRouteBoundaries;
use super::classify_candidate;
use super::is_missing;
use super::route_walker::RouteObservationSnapshot;
use super::route_walker::observe_route;
#[cfg(windows)]
use super::windows_path_is_ambiguous;

impl RepositoryRouteBoundaries {
    pub(crate) fn route_cancels_worktree_descendant(&self, route: &Path) -> io::Result<bool> {
        if route_requires_fail_closed(route)? {
            return Ok(true);
        }
        observation_cancels_worktree_descendant(&observe_route(route)?, self)
    }

    pub(crate) fn retained_route_is_untrusted(&self, route: &Path) -> io::Result<bool> {
        if route_requires_fail_closed(route)? {
            return Ok(true);
        }
        let observation = observe_route(route)?;
        if observation_cancels_worktree_descendant(&observation, self)? {
            return Ok(true);
        }
        // Unlike active metadata, retained registry routes are not rebound
        // before every child. A symlink or junction entry below a worktree is
        // therefore mutable authority even when it currently aliases metadata.
        // Parent identity classification covers Windows Unicode case aliases.
        for hop in &observation.symlink_hops {
            if matches!(
                classify_candidate(
                    &hop.parent,
                    &self.worktree_roots,
                    &self.metadata_dirs,
                    &self.worktree_identities,
                    &self.metadata_identities,
                )?,
                CandidateBoundary::ExactWorktree | CandidateBoundary::Worktree
            ) {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

fn observation_cancels_worktree_descendant(
    observation: &RouteObservationSnapshot,
    boundaries: &RepositoryRouteBoundaries,
) -> io::Result<bool> {
    if raw_spelling_cancels_worktree_descendant(&observation.raw.spelling, boundaries)? {
        return Ok(true);
    }
    for hop in &observation.symlink_hops {
        if raw_spelling_cancels_worktree_descendant(&hop.target.spelling, boundaries)?
            || raw_spelling_cancels_worktree_descendant(&hop.projected.spelling, boundaries)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn route_requires_fail_closed(route: &Path) -> io::Result<bool> {
    #[cfg(windows)]
    {
        let route = route.to_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "non-UTF-8 Windows authority path",
            )
        })?;
        Ok(windows_path_is_ambiguous(route))
    }
    #[cfg(not(windows))]
    {
        let _ = route;
        Ok(false)
    }
}

fn raw_spelling_cancels_worktree_descendant(
    spelling: &Path,
    boundaries: &RepositoryRouteBoundaries,
) -> io::Result<bool> {
    let mut prefix = PathBuf::new();
    for component in spelling.components() {
        match component {
            Component::ParentDir => {
                if candidate_is_proper_worktree_descendant(
                    &prefix,
                    &boundaries.worktree_roots,
                    &boundaries.metadata_dirs,
                    &boundaries.worktree_identities,
                    &boundaries.metadata_identities,
                )? {
                    return Ok(true);
                }
                prefix.pop();
            }
            Component::CurDir => {}
            _ => prefix.push(component.as_os_str()),
        }
    }
    Ok(false)
}

pub(super) fn candidate_is_proper_worktree_descendant(
    path: &Path,
    worktree_roots: &[PathBuf],
    metadata_dirs: &[PathBuf],
    worktree_identities: &[Handle],
    metadata_identities: &[Handle],
) -> io::Result<bool> {
    if metadata_dirs
        .iter()
        .any(|metadata| path_is_within(path, metadata))
    {
        return Ok(false);
    }

    // A worktree-controlled spelling cannot become trusted merely because a
    // mutable symlink or junction currently aliases protected metadata. Keep
    // genuinely metadata-spelled paths exempt above, but reject lexical
    // worktree descendants before following identities.
    if worktree_roots
        .iter()
        .any(|root| path_is_within(path, root) && !path_is_within(root, path))
    {
        return Ok(true);
    }

    let mut ancestor_identities = Vec::new();
    for (depth, ancestor) in path.ancestors().enumerate() {
        match std::fs::metadata(ancestor) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => continue,
            Err(error) if is_missing(&error) => continue,
            Err(error) => return Err(error),
        }
        let identity = Handle::from_path(ancestor)?;
        if metadata_identities.contains(&identity) {
            return Ok(false);
        }
        ancestor_identities.push((depth, identity));
    }

    Ok(ancestor_identities
        .into_iter()
        .any(|(depth, identity)| depth > 0 && worktree_identities.contains(&identity)))
}
