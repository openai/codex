use std::io;
use std::path::Path;
use std::path::PathBuf;

use super::path_safety::CONFIG_PATH_KEY;
use super::path_safety::invalid_config_source;
use super::path_safety::reject_raw_ambiguous_windows_config_path;
use super::path_safety::resolve_literal_path;
use super::reject_source;
use crate::git_command::GitRunner;
use crate::git_command::MAX_INTERNAL_GIT_OUTPUT_BYTES;
use crate::git_config::GitConfigEntry;
use crate::git_config::GitConfigOrigin;

#[derive(Debug)]
pub(super) struct IncludeGraphBudget {
    remaining_directives: usize,
}

impl Default for IncludeGraphBudget {
    fn default() -> Self {
        Self {
            remaining_directives: super::MAX_CONFIG_INCLUDE_FILES,
        }
    }
}

impl IncludeGraphBudget {
    fn reserve_directive(&mut self) -> io::Result<()> {
        let Some(remaining) = self.remaining_directives.checked_sub(1) else {
            return Err(invalid_config_source("too many Git config includes"));
        };
        self.remaining_directives = remaining;
        Ok(())
    }

    #[cfg(all(test, unix))]
    pub(super) fn with_limit(limit: usize) -> Self {
        Self {
            remaining_directives: limit,
        }
    }
}

pub(super) fn validate_include_entries(
    git: &GitRunner,
    git_root: &Path,
    entries: Vec<GitConfigEntry>,
    depth: usize,
    pending: &mut Vec<(PathBuf, usize)>,
    budget: &mut IncludeGraphBudget,
) -> io::Result<()> {
    for entry in entries {
        if !is_include_path(&entry.key) {
            return Err(invalid_config_source("unexpected Git config include key"));
        }
        if let Some(origin) = config_file_origin(&entry, git_root)? {
            reject_source(git, git_root, &origin, "Git config origin")?;
        }
        // Consume one graph-wide slot before a path expansion can launch a Git
        // child. Every accepted directive also becomes at most one pending
        // file, so this caps both process fan-out and queued traversal work.
        budget.reserve_directive()?;
        let include = resolve_include_path(git, git_root, &entry)?;
        reject_source(git, git_root, &include, "Git config include")?;
        pending.push((include, depth));
    }
    Ok(())
}

pub(super) async fn validate_include_entries_async(
    git: &GitRunner,
    git_root: &Path,
    entries: Vec<GitConfigEntry>,
    depth: usize,
    pending: &mut Vec<(PathBuf, usize)>,
    budget: &mut IncludeGraphBudget,
) -> io::Result<()> {
    for entry in entries {
        if !is_include_path(&entry.key) {
            return Err(invalid_config_source("unexpected Git config include key"));
        }
        if let Some(origin) = config_file_origin(&entry, git_root)? {
            reject_source(git, git_root, &origin, "Git config origin")?;
        }
        // Keep this check before the await: an over-limit directive must not
        // start another expansion child.
        budget.reserve_directive()?;
        let include = resolve_include_path_async(git, git_root, &entry).await?;
        reject_source(git, git_root, &include, "Git config include")?;
        pending.push((include, depth));
    }
    Ok(())
}

fn config_file_origin(entry: &GitConfigEntry, cwd: &Path) -> io::Result<Option<PathBuf>> {
    match &entry.origin {
        GitConfigOrigin::CommandLine => Ok(None),
        GitConfigOrigin::File(path) => Ok(Some(resolve_literal_path(path, cwd))),
    }
}

fn is_include_path(key: &str) -> bool {
    key == "include.path" || key.starts_with("includeif.") && key.ends_with(".path")
}

pub(super) fn resolve_include_path(
    git: &GitRunner,
    cwd: &Path,
    entry: &GitConfigEntry,
) -> io::Result<PathBuf> {
    let raw = entry.value.as_str();
    if raw.is_empty() {
        return Err(invalid_config_source("empty Git config include path"));
    }
    // Unlike generic `git config --path`, include.path treats `:(...)` as
    // literal path text. Bypass the generic path expander for those spellings
    // so the validated path exactly matches Git's include loader.
    let expanded = if raw.starts_with(":(") {
        PathBuf::from(raw)
    } else {
        expand_git_config_path(git, cwd, raw)?
    };
    reject_raw_ambiguous_windows_config_path(
        expanded
            .to_str()
            .ok_or_else(|| invalid_config_source("non-UTF-8 Git include path"))?,
    )?;
    let base = match config_file_origin(entry, cwd)? {
        Some(origin) => origin
            .parent()
            .ok_or_else(|| invalid_config_source("Git config origin has no parent"))?
            .to_path_buf(),
        None if expanded.is_absolute() => cwd.to_path_buf(),
        None => {
            return Err(invalid_config_source(
                "relative Git config include has no file origin",
            ));
        }
    };
    Ok(resolve_literal_path(expanded, &base))
}

async fn resolve_include_path_async(
    git: &GitRunner,
    cwd: &Path,
    entry: &GitConfigEntry,
) -> io::Result<PathBuf> {
    let raw = entry.value.as_str();
    if raw.is_empty() {
        return Err(invalid_config_source("empty Git config include path"));
    }
    let expanded = if raw.starts_with(":(") {
        PathBuf::from(raw)
    } else {
        expand_git_config_path_async(git, cwd, raw).await?
    };
    reject_raw_ambiguous_windows_config_path(
        expanded
            .to_str()
            .ok_or_else(|| invalid_config_source("non-UTF-8 Git include path"))?,
    )?;
    let base = match config_file_origin(entry, cwd)? {
        Some(origin) => origin
            .parent()
            .ok_or_else(|| invalid_config_source("Git config origin has no parent"))?
            .to_path_buf(),
        None if expanded.is_absolute() => cwd.to_path_buf(),
        None => {
            return Err(invalid_config_source(
                "relative Git config include has no file origin",
            ));
        }
    };
    Ok(resolve_literal_path(expanded, &base))
}

pub(super) fn expand_git_config_path(
    git: &GitRunner,
    cwd: &Path,
    raw: &str,
) -> io::Result<PathBuf> {
    let mut command = git.command_for_cwd(cwd)?;
    command
        .arg("-c")
        .arg(format!("{CONFIG_PATH_KEY}={raw}"))
        .args([
            "config",
            "--null",
            "--no-includes",
            "--path",
            "--get",
            CONFIG_PATH_KEY,
        ]);
    let output = git.output(command)?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "git include path expansion failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    parse_expanded_config_path(&output.stdout)
}

async fn expand_git_config_path_async(
    git: &GitRunner,
    cwd: &Path,
    raw: &str,
) -> io::Result<PathBuf> {
    let mut command = git.async_command_for_cwd(cwd)?;
    command
        .arg("-c")
        .arg(format!("{CONFIG_PATH_KEY}={raw}"))
        .args([
            "config",
            "--null",
            "--no-includes",
            "--path",
            "--get",
            CONFIG_PATH_KEY,
        ]);
    let output = git
        .output_async_bounded(command, MAX_INTERNAL_GIT_OUTPUT_BYTES)
        .await?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "git include path expansion failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    parse_expanded_config_path(&output.stdout)
}

fn parse_expanded_config_path(output: &[u8]) -> io::Result<PathBuf> {
    let value = output
        .strip_suffix(&[0])
        .ok_or_else(|| invalid_config_source("unterminated Git include path expansion"))?;
    if value.is_empty() || value.contains(&0) {
        return Err(invalid_config_source(
            "ambiguous Git include path expansion",
        ));
    }
    let value = std::str::from_utf8(value)
        .map_err(|_| invalid_config_source("non-UTF-8 Git include path expansion"))?;
    Ok(PathBuf::from(value))
}
