use std::io;
use std::path::Path;

use crate::git_command::GitRunner;

mod path_safety;
mod primary_sources;

use path_safety::normalize_absolute_path;
use path_safety::resolve_literal_path;
use primary_sources::is_disabled_primary_config_path;
use primary_sources::legacy_primary_config_source_candidates;
use primary_sources::selected_git_home_config_candidates;
use primary_sources::selected_git_prefix_system_candidate;

/// Reject primary Git configuration paths selected from worktree-controlled
/// routes before any ordinary Git command can open them.
///
/// This layer intentionally does not authorize includes. The complete include
/// graph is added by the next stack layer before this is promoted to a full
/// config-source capability.
pub(crate) fn ensure_no_worktree_primary_config_sources(
    git: &GitRunner,
    git_root: &Path,
) -> io::Result<()> {
    let git_root = normalize_absolute_path(std::fs::canonicalize(git_root)?)?;

    for (description, candidate) in legacy_primary_config_source_candidates(git)? {
        if !is_disabled_primary_config_path(&candidate) {
            reject_source(git, &git_root, &candidate, description)?;
        }
    }
    for candidate in selected_git_home_config_candidates(git, &git_root)? {
        reject_source(git, &git_root, &candidate, "selected Git HOME config")?;
    }
    if let Some(candidate) = selected_git_prefix_system_candidate(git, &git_root)? {
        reject_source(
            git,
            &git_root,
            &candidate,
            "selected Git prefix system config",
        )?;
    }
    Ok(())
}

fn reject_source(
    git: &GitRunner,
    cwd: &Path,
    candidate: &Path,
    description: &str,
) -> io::Result<()> {
    let absolute = resolve_literal_path(candidate, cwd);
    git.ensure_config_source_is_not_worktree_controlled(&absolute, description)
}

#[cfg(test)]
#[path = "git_config_sources_tests.rs"]
mod tests;
