use std::collections::BTreeSet;
use std::io;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

use crate::git_command::GitRunner;
use crate::git_config::read_config_entries_without_includes;

mod include_graph;
mod path_safety;
mod primary_sources;

use include_graph::validate_include_entries;
use path_safety::normalize_absolute_path;
use path_safety::resolve_literal_path;
use primary_sources::default_system_config_source_candidates;
use primary_sources::is_disabled_primary_config_path;
use primary_sources::legacy_primary_config_source_candidates;
use primary_sources::selected_git_home_config_candidates;
use primary_sources::selected_git_prefix_system_candidate;

const INCLUDE_CONFIG_PATTERN: &str = r"^include(\.path|if\..*\.path)$";
const MAX_CONFIG_INCLUDE_DEPTH: usize = 10;
const MAX_CONFIG_INCLUDE_FILES: usize = 1024;

/// Reject configuration that an untrusted worktree writer can change between
/// a policy probe and the Git command it guards.
pub(crate) fn ensure_no_worktree_config_sources(
    git: &GitRunner,
    git_root: &Path,
    git_config_args: &[String],
) -> io::Result<()> {
    let git_root = normalize_absolute_path(std::fs::canonicalize(git_root)?)?;

    // Environment, HOME, and XDG paths are attacker-controlled byte strings.
    // Classify their exact OsString spelling before `git var` can open or
    // newline-delimit one of them.
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
    let entries = read_config_entries_without_includes(
        git,
        &git_root,
        git_config_args,
        INCLUDE_CONFIG_PATTERN,
        "include",
        /*config_file*/ None,
    )?;
    let mut pending = Vec::new();
    validate_include_entries(git, &git_root, entries, /*depth*/ 1, &mut pending)?;
    let mut visited = BTreeSet::new();
    while let Some((config_path, depth)) = pending.pop() {
        if depth > MAX_CONFIG_INCLUDE_DEPTH {
            return Err(path_safety::invalid_config_source(
                "Git config include depth exceeded",
            ));
        }
        match std::fs::canonicalize(&config_path) {
            Ok(_) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                ) =>
            {
                continue;
            }
            Err(error) => return Err(error),
        }
        // The same file reached through two spellings can resolve a relative
        // child include differently. Deduplicate only the exact source
        // spelling, not its canonical target.
        if !visited.insert(config_path.clone()) {
            continue;
        }
        if visited.len() > MAX_CONFIG_INCLUDE_FILES {
            return Err(path_safety::invalid_config_source(
                "too many Git config include files",
            ));
        }
        let entries = read_config_entries_without_includes(
            git,
            &git_root,
            &[],
            INCLUDE_CONFIG_PATTERN,
            "include",
            Some(&config_path),
        )?;
        validate_include_entries(git, &git_root, entries, depth + 1, &mut pending)?;
    }
    // `git var` loads repository config before reporting modern default-system
    // paths. Delay that supplemental probe until the complete no-includes
    // source graph above has been classified, so it cannot be tricked into
    // opening an untrusted include or FIFO first.
    for (description, candidate) in default_system_config_source_candidates(git, &git_root)? {
        if !is_disabled_primary_config_path(&candidate) {
            reject_source(git, &git_root, &candidate, description)?;
        }
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

#[cfg(all(test, not(windows)))]
use crate::git_config::GitConfigEntry;
#[cfg(all(test, not(windows)))]
use codex_utils_absolute_path::AbsolutePathBuf;
#[cfg(test)]
use include_graph::expand_git_config_path;
#[cfg(all(test, not(windows)))]
use include_graph::resolve_include_path;
#[cfg(test)]
use path_safety::windows_config_path_is_ambiguous;

#[cfg(test)]
#[path = "git_config_sources_tests.rs"]
mod tests;
