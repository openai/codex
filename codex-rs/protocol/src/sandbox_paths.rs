use std::collections::HashSet;
use std::ffi::OsStr;

use codex_utils_absolute_path::AbsolutePathBuf;
use tracing::error;

pub(crate) fn default_read_only_subpaths_for_writable_root(
    writable_root: &AbsolutePathBuf,
    protect_missing_dot_codex: bool,
    allow_limited_git_writes: bool,
) -> Vec<AbsolutePathBuf> {
    let mut subpaths: Vec<AbsolutePathBuf> = Vec::new();
    let top_level_git = writable_root.join(".git");
    // This applies to typical repos (directory .git), worktrees/submodules
    // (file .git with gitdir pointer), and bare repos when the gitdir is the
    // writable root itself.
    let top_level_git_is_file = top_level_git.as_path().is_file();
    let top_level_git_is_dir = top_level_git.as_path().is_dir();
    if top_level_git_is_dir || top_level_git_is_file {
        if top_level_git_is_file
            && is_git_pointer_file(&top_level_git)
            && let Some(gitdir) = resolve_gitdir_from_file(&top_level_git)
        {
            if allow_limited_git_writes {
                subpaths.extend(protected_git_subpaths_when_writable(&gitdir));
            } else {
                subpaths.push(gitdir);
            }
        }
        if allow_limited_git_writes {
            if top_level_git_is_dir {
                subpaths.extend(protected_git_subpaths_when_writable(&top_level_git));
            }
        } else {
            subpaths.push(top_level_git);
        }
    }

    let top_level_agents = writable_root.join(".agents");
    if top_level_agents.as_path().is_dir() {
        subpaths.push(top_level_agents);
    }

    // Keep top-level project metadata under .codex read-only to the agent by
    // default. For the workspace root itself, protect it even before the
    // directory exists so first-time creation still goes through the
    // protected-path approval flow.
    let top_level_codex = writable_root.join(".codex");
    if protect_missing_dot_codex || top_level_codex.as_path().is_dir() {
        subpaths.push(top_level_codex);
    }

    let mut deduped = Vec::with_capacity(subpaths.len());
    let mut seen = HashSet::new();
    for path in subpaths {
        if seen.insert(path.to_path_buf()) {
            deduped.push(path);
        }
    }
    deduped
}

fn protected_git_subpaths_when_writable(gitdir: &AbsolutePathBuf) -> Vec<AbsolutePathBuf> {
    ["config", "hooks"]
        .into_iter()
        .map(|subpath| gitdir.join(subpath))
        .collect()
}

fn is_git_pointer_file(path: &AbsolutePathBuf) -> bool {
    path.as_path().is_file() && path.as_path().file_name() == Some(OsStr::new(".git"))
}

fn resolve_gitdir_from_file(dot_git: &AbsolutePathBuf) -> Option<AbsolutePathBuf> {
    let contents = match std::fs::read_to_string(dot_git.as_path()) {
        Ok(contents) => contents,
        Err(err) => {
            error!(
                "Failed to read {path} for gitdir pointer: {err}",
                path = dot_git.as_path().display()
            );
            return None;
        }
    };

    let trimmed = contents.trim();
    let (_, gitdir_raw) = match trimmed.split_once(':') {
        Some(parts) => parts,
        None => {
            error!(
                "Expected {path} to contain a gitdir pointer, but it did not match `gitdir: <path>`.",
                path = dot_git.as_path().display()
            );
            return None;
        }
    };
    let gitdir_raw = gitdir_raw.trim();
    if gitdir_raw.is_empty() {
        error!(
            "Expected {path} to contain a gitdir pointer, but it was empty.",
            path = dot_git.as_path().display()
        );
        return None;
    }
    let base = match dot_git.as_path().parent() {
        Some(base) => base,
        None => {
            error!(
                "Unable to resolve parent directory for {path}.",
                path = dot_git.as_path().display()
            );
            return None;
        }
    };
    let gitdir_path = AbsolutePathBuf::resolve_path_against_base(gitdir_raw, base);
    if !gitdir_path.as_path().exists() {
        error!(
            "Resolved gitdir path {path} does not exist.",
            path = gitdir_path.as_path().display()
        );
        return None;
    }
    Some(gitdir_path)
}
