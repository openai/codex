use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io;
use std::io::Seek;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use tokio::process::Command as TokioCommand;
use tokio::time::Duration;
use tokio::time::timeout;

use crate::git_config::GitConfigEntry;
use crate::git_config::parse_effective_config;

pub(crate) const DISABLED_HOOKS_PATH: &str = if cfg!(windows) { "NUL" } else { "/dev/null" };
pub(crate) const EXECUTABLE_FILTER_CONFIG_PATTERN: &str = r"^filter\..*\.(clean|smudge|process)$";
/// Timeout for internal Git commands to prevent freezing on large repositories.
pub(crate) const GIT_COMMAND_TIMEOUT: Duration = Duration::from_secs(5);

const ISOLATED_GIT_ENVIRONMENT: [&str; 11] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_INDEX_FILE",
    "GIT_PREFIX",
    "GIT_LITERAL_PATHSPECS",
    "GIT_GLOB_PATHSPECS",
    "GIT_NOGLOB_PATHSPECS",
    "GIT_ICASE_PATHSPECS",
    "GIT_EXEC_PATH",
    // Legacy `GIT_CONFIG` affects `git config` but not ordinary worktree
    // commands, so inheriting it can make a safety probe inspect different
    // configuration than the command it guards.
    "GIT_CONFIG",
];

/// Keep internal worktree operations bound to their explicit cwd and pathspec
/// semantics instead of inheriting repository, index, or pathspec selectors.
/// Deliberately leave Git config channels intact: callers may rely on normal
/// system/global configuration, and executable helpers are probed separately.
pub(crate) fn isolate_git_command_environment(command: &mut Command) {
    for name in ISOLATED_GIT_ENVIRONMENT {
        command.env_remove(name);
    }
}

pub(crate) fn isolate_tokio_git_command_environment(command: &mut tokio::process::Command) {
    for name in ISOLATED_GIT_ENVIRONMENT {
        command.env_remove(name);
    }
}

pub(crate) async fn has_selected_executable_filters_from(git: &Path, cwd: &Path) -> Option<bool> {
    let git_root = resolve_git_root_async(git, cwd).await?;
    let entries = read_filter_config_async(git, &git_root).await?;
    if !entries.values().any(|entry| !entry.value.is_empty()) {
        return Some(false);
    }
    let paths = read_paths_async(git, &git_root, PathSelection::Tracked).await?;
    let attributes = read_filter_attributes_async(git, &git_root, &paths).await?;
    selected_executable_filter(&entries, &attributes)
        .ok()
        .map(|selected| selected.is_some())
}

/// Validate every tracked path plus the exact untracked paths that the caller
/// will later feed to `git diff --no-index`. The returned raw paths must be
/// reused rather than reconstructed from Git's quoted line-oriented output.
pub(crate) async fn safe_untracked_paths_for_diff(git: &Path, cwd: &Path) -> Option<Vec<Vec<u8>>> {
    let requested_cwd = std::fs::canonicalize(cwd).ok()?;
    let git_root = resolve_git_root_async(git, &requested_cwd).await?;
    let untracked = read_paths_async(git, &requested_cwd, PathSelection::Untracked).await?;
    // An embedded untracked repository is reported by `ls-files` as a single
    // directory entry. Passing that entry to `git diff --no-index` invokes
    // file-vs-directory comparison semantics, which can derive and open a
    // child path that was never returned by the path probe. Fail closed rather
    // than let the sink operate on a different path vector than we validated.
    for path in &untracked {
        let path = git_path_bytes_to_path_buf(path)?;
        if std::fs::symlink_metadata(requested_cwd.join(path))
            .ok()?
            .file_type()
            .is_dir()
        {
            return None;
        }
    }
    let entries = read_filter_config_async(git, &git_root).await?;
    if !entries.values().any(|entry| !entry.value.is_empty()) {
        return Some(untracked);
    }

    let tracked = read_paths_async(git, &git_root, PathSelection::Tracked).await?;
    let tracked_attributes = read_filter_attributes_async(git, &git_root, &tracked).await?;
    if selected_executable_filter(&entries, &tracked_attributes)
        .ok()?
        .is_some()
    {
        return None;
    }
    let untracked_attributes =
        read_filter_attributes_async(git, &requested_cwd, &untracked).await?;
    if selected_executable_filter(&entries, &untracked_attributes)
        .ok()?
        .is_some()
    {
        return None;
    }
    Some(untracked)
}

fn git_path_bytes_to_path_buf(path: &[u8]) -> Option<PathBuf> {
    #[cfg(unix)]
    {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        Some(PathBuf::from(OsString::from_vec(path.to_vec())))
    }
    #[cfg(windows)]
    {
        String::from_utf8(path.to_vec()).ok().map(PathBuf::from)
    }
}

async fn resolve_git_root_async(git: &Path, cwd: &Path) -> Option<PathBuf> {
    let requested_cwd = std::fs::canonicalize(cwd).ok()?;
    let mut command = TokioCommand::new(git);
    isolate_tokio_git_command_environment(&mut command);
    command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args([
            "-c",
            &format!("core.hooksPath={DISABLED_HOOKS_PATH}"),
            "-c",
            "core.fsmonitor=false",
            "rev-parse",
            "--show-toplevel",
        ])
        .current_dir(&requested_cwd)
        .kill_on_drop(true);
    let output = match timeout(GIT_COMMAND_TIMEOUT, command.output()).await {
        Ok(Ok(output)) => output,
        _ => return None,
    };
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8(output.stdout).ok()?;
    let path = path.trim_end_matches(['\r', '\n']);
    if path.is_empty() {
        return None;
    }
    let reported_root = std::fs::canonicalize(PathBuf::from(path)).ok()?;
    let expected_root = crate::get_git_repo_root(&requested_cwd)
        .and_then(|root| std::fs::canonicalize(root).ok())?;
    if reported_root != expected_root {
        return None;
    }
    Some(reported_root)
}

pub(crate) fn ensure_no_selected_executable_git_filters(
    cwd: &Path,
    paths: &[String],
    git_config_args: &[String],
) -> io::Result<()> {
    let entries = read_filter_config(cwd, git_config_args)?;
    if !entries.values().any(|entry| !entry.value.is_empty()) {
        return Ok(());
    }
    let paths = paths
        .iter()
        .map(|path| path.as_bytes().to_vec())
        .collect::<Vec<_>>();
    let attributes = read_filter_attributes(cwd, &paths, git_config_args)?;
    if let Some((driver, path)) = selected_executable_filter(&entries, &attributes)? {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "refusing to run an internal Git worktree operation with executable filter {driver:?} selected for {}",
                String::from_utf8_lossy(&path)
            ),
        ));
    }
    Ok(())
}

fn read_filter_config(
    cwd: &Path,
    git_config_args: &[String],
) -> io::Result<BTreeMap<String, GitConfigEntry>> {
    let mut command = Command::new("git");
    isolate_git_command_environment(&mut command);
    let output = command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(git_config_args)
        .args([
            "config",
            "--null",
            "--show-scope",
            "--show-origin",
            "--includes",
            "--get-regexp",
            EXECUTABLE_FILTER_CONFIG_PATTERN,
        ])
        .current_dir(cwd)
        .output()?;
    if !output
        .status
        .code()
        .is_some_and(|code| code == 0 || code == 1)
    {
        return Err(io::Error::other(format!(
            "git config probe failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    parse_effective_config(&output.stdout)
}

async fn read_filter_config_async(
    git: &Path,
    cwd: &Path,
) -> Option<BTreeMap<String, GitConfigEntry>> {
    let mut command = TokioCommand::new(git);
    isolate_tokio_git_command_environment(&mut command);
    command
        .args([
            "config",
            "--null",
            "--show-scope",
            "--show-origin",
            "--includes",
            "--get-regexp",
            EXECUTABLE_FILTER_CONFIG_PATTERN,
        ])
        .current_dir(cwd)
        .kill_on_drop(true);
    let output = match timeout(GIT_COMMAND_TIMEOUT, command.output()).await {
        Ok(Ok(output)) => output,
        _ => return None,
    };
    if !output
        .status
        .code()
        .is_some_and(|code| code == 0 || code == 1)
    {
        return None;
    }
    parse_effective_config(&output.stdout).ok()
}

#[derive(Clone, Copy)]
enum PathSelection {
    Tracked,
    Untracked,
}

async fn read_paths_async(
    git: &Path,
    cwd: &Path,
    selection: PathSelection,
) -> Option<Vec<Vec<u8>>> {
    let mut command = TokioCommand::new(git);
    isolate_tokio_git_command_environment(&mut command);
    let hooks_config = format!("core.hooksPath={DISABLED_HOOKS_PATH}");
    let mut args = vec![
        "-c",
        hooks_config.as_str(),
        "-c",
        "core.fsmonitor=false",
        "ls-files",
        "-z",
    ];
    match selection {
        PathSelection::Tracked => args.push("--cached"),
        PathSelection::Untracked => args.extend(["--others", "--exclude-standard"]),
    }
    command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(args)
        .current_dir(cwd)
        .kill_on_drop(true);
    let output = match timeout(GIT_COMMAND_TIMEOUT, command.output()).await {
        Ok(Ok(output)) => output,
        _ => return None,
    };
    if !output.status.success() {
        return None;
    }
    parse_nul_paths(&output.stdout).ok()
}

async fn read_filter_attributes_async(
    git: &Path,
    cwd: &Path,
    paths: &[Vec<u8>],
) -> Option<BTreeMap<Vec<u8>, String>> {
    if paths.is_empty() {
        return Some(BTreeMap::new());
    }
    let mut input = tempfile::tempfile().ok()?;
    write_nul_paths(&mut input, paths).ok()?;
    input.rewind().ok()?;

    let mut command = TokioCommand::new(git);
    isolate_tokio_git_command_environment(&mut command);
    command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args([
            "-c",
            &format!("core.hooksPath={DISABLED_HOOKS_PATH}"),
            "-c",
            "core.fsmonitor=false",
            "check-attr",
            "--stdin",
            "-z",
            "filter",
        ])
        .current_dir(cwd)
        .stdin(Stdio::from(input))
        .kill_on_drop(true);
    let output = match timeout(GIT_COMMAND_TIMEOUT, command.output()).await {
        Ok(Ok(output)) => output,
        _ => return None,
    };
    if !output.status.success() {
        return None;
    }
    parse_filter_attributes(&output.stdout, paths).ok()
}

fn read_filter_attributes(
    cwd: &Path,
    paths: &[Vec<u8>],
    git_config_args: &[String],
) -> io::Result<BTreeMap<Vec<u8>, String>> {
    if paths.is_empty() {
        return Ok(BTreeMap::new());
    }
    let mut input = tempfile::tempfile()?;
    write_nul_paths(&mut input, paths)?;
    input.rewind()?;

    let mut command = Command::new("git");
    isolate_git_command_environment(&mut command);
    let output = command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(git_config_args)
        .args([
            "-c",
            &format!("core.hooksPath={DISABLED_HOOKS_PATH}"),
            "-c",
            "core.fsmonitor=false",
            "check-attr",
            "--stdin",
            "-z",
            "filter",
        ])
        .current_dir(cwd)
        .stdin(Stdio::from(input))
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "git filter attribute probe failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    parse_filter_attributes(&output.stdout, paths)
}

fn selected_executable_filter(
    entries: &BTreeMap<String, GitConfigEntry>,
    attributes: &BTreeMap<Vec<u8>, String>,
) -> io::Result<Option<(String, Vec<u8>)>> {
    let mut executable_drivers = BTreeSet::new();
    for entry in entries.values() {
        let driver = filter_driver_name(&entry.key)?;
        if !entry.value.is_empty() {
            executable_drivers.insert(driver);
        }
    }
    for (path, driver) in attributes {
        if executable_drivers.contains(driver) {
            return Ok(Some((driver.clone(), path.clone())));
        }
    }
    Ok(None)
}

fn filter_driver_name(key: &str) -> io::Result<String> {
    let Some(remainder) = key.strip_prefix("filter.") else {
        return Err(invalid_filter_output("malformed filter config key"));
    };
    let driver = [".clean", ".smudge", ".process"]
        .into_iter()
        .find_map(|suffix| remainder.strip_suffix(suffix))
        .filter(|driver| !driver.is_empty())
        .ok_or_else(|| invalid_filter_output("malformed filter config key"))?;
    Ok(driver.to_string())
}

fn parse_nul_paths(output: &[u8]) -> io::Result<Vec<Vec<u8>>> {
    if output.is_empty() {
        return Ok(Vec::new());
    }
    let Some(body) = output.strip_suffix(&[0]) else {
        return Err(invalid_filter_output("unterminated Git path output"));
    };
    let mut paths = Vec::new();
    for path in body.split(|byte| *byte == 0) {
        if path.is_empty() {
            return Err(invalid_filter_output("empty Git path"));
        }
        paths.push(path.to_vec());
    }
    Ok(paths)
}

fn write_nul_paths(input: &mut std::fs::File, paths: &[Vec<u8>]) -> io::Result<()> {
    let mut unique = BTreeSet::new();
    for path in paths {
        if path.is_empty() || path.contains(&0) {
            return Err(invalid_filter_output("invalid Git path"));
        }
        if unique.insert(path.as_slice()) {
            input.write_all(path)?;
            input.write_all(&[0])?;
        }
    }
    Ok(())
}

fn parse_filter_attributes(
    output: &[u8],
    expected_paths: &[Vec<u8>],
) -> io::Result<BTreeMap<Vec<u8>, String>> {
    let expected = expected_paths
        .iter()
        .map(Vec::as_slice)
        .collect::<BTreeSet<_>>();
    if expected.is_empty() && output.is_empty() {
        return Ok(BTreeMap::new());
    }
    let Some(body) = output.strip_suffix(&[0]) else {
        return Err(invalid_filter_output(
            "unterminated Git filter attribute output",
        ));
    };
    let fields = body.split(|byte| *byte == 0).collect::<Vec<_>>();
    if fields.len() % 3 != 0 {
        return Err(invalid_filter_output(
            "incomplete Git filter attribute record",
        ));
    }
    let mut attributes = BTreeMap::new();
    for record in fields.chunks_exact(3) {
        if !expected.contains(record[0]) || record[1] != b"filter" {
            return Err(invalid_filter_output(
                "unexpected Git filter attribute record",
            ));
        }
        let driver = std::str::from_utf8(record[2])
            .map_err(|_| invalid_filter_output("non-UTF-8 Git filter attribute value"))?;
        if attributes
            .insert(record[0].to_vec(), driver.to_string())
            .is_some()
        {
            return Err(invalid_filter_output(
                "duplicate Git filter attribute record",
            ));
        }
    }
    if attributes.len() != expected.len() {
        return Err(invalid_filter_output("missing Git filter attribute record"));
    }
    Ok(attributes)
}

fn invalid_filter_output(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
#[path = "safe_git_tests.rs"]
mod tests;
