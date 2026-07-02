use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io;
use std::io::Seek;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;

use tokio::process::Command as TokioCommand;
use tokio::time::timeout;

use crate::GitReadError;
use crate::git_command::GitRunner;
use crate::git_config::GitConfigEntry;
use crate::git_config::parse_effective_config;
use crate::git_config::parse_effective_config_with_origins;
use crate::safe_git::DISABLED_HOOKS_PATH;
use crate::safe_git::EXECUTABLE_FILTER_CONFIG_PATTERN;
use crate::safe_git::FilterAttributeValue;
use crate::safe_git::GIT_COMMAND_TIMEOUT;
use crate::safe_git::executable_filter_drivers;
use crate::safe_git::git_path_argument;
use crate::safe_git::parse_filter_attributes;
use crate::safe_git::parse_nul_paths;
use crate::safe_git::selected_filter;
use crate::safe_git::write_nul_paths;

pub(crate) async fn selected_executable_filter_from(
    git: &GitRunner,
    cwd: &Path,
) -> Result<Option<(String, Vec<u8>)>, GitReadError> {
    let git_root = resolve_git_root_async(git, cwd).await?;
    let entries = read_filter_config_async(git, &git_root).await?;
    let executable_drivers =
        executable_filter_drivers(&entries).map_err(|_| invalid_output("filterConfig"))?;
    if executable_drivers.is_empty() {
        return Ok(None);
    }
    let paths = read_paths_async(git, &git_root, PathSelection::Tracked).await?;
    let attributes = read_filter_attributes_async(git, &git_root, &paths).await?;
    let attributes =
        resolve_filter_attribute_sentinels_async(git, &git_root, attributes, &executable_drivers)
            .await?;
    Ok(selected_filter(&executable_drivers, &attributes))
}

pub(crate) async fn resolve_git_root_async(
    git: &GitRunner,
    cwd: &Path,
) -> Result<PathBuf, GitReadError> {
    let requested_cwd = std::fs::canonicalize(cwd).map_err(|_| GitReadError::NotRepository {
        path: cwd.to_path_buf(),
    })?;
    let expected_root = crate::get_git_repo_root(&requested_cwd)
        .and_then(|root| std::fs::canonicalize(root).ok())
        .ok_or_else(|| GitReadError::NotRepository {
            path: requested_cwd.clone(),
        })?;
    let mut command = git.tokio_command();
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
    let output = command_output(git, command, "resolveGitRoot").await?;
    if !output.status.success() {
        return Err(command_failed("resolveGitRoot", output.status.code()));
    }
    let reported_root = git_root_from_stdout(output.stdout).and_then(|path| {
        std::fs::canonicalize(path).map_err(|_| invalid_output("resolveGitRoot"))
    })?;
    if reported_root != expected_root {
        return Err(GitReadError::RepositoryRootMismatch {
            expected_root,
            reported_root,
        });
    }
    Ok(reported_root)
}

pub(crate) fn git_root_from_stdout(output: Vec<u8>) -> Result<PathBuf, GitReadError> {
    let output = output.strip_suffix(b"\n").unwrap_or(&output);
    #[cfg(windows)]
    let output = output.strip_suffix(b"\r").unwrap_or(output);
    if output.is_empty() {
        return Err(invalid_output("resolveGitRoot"));
    }
    #[cfg(unix)]
    {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        Ok(PathBuf::from(OsString::from_vec(output.to_vec())))
    }
    #[cfg(not(unix))]
    {
        String::from_utf8(output.to_vec())
            .map(PathBuf::from)
            .map_err(|_| invalid_output("resolveGitRoot"))
    }
}

async fn read_filter_config_async(
    git: &GitRunner,
    cwd: &Path,
) -> Result<BTreeMap<String, GitConfigEntry>, GitReadError> {
    let scoped = run_filter_config_query_async(git, cwd, /*show_scope*/ true).await?;
    if scoped
        .status
        .code()
        .is_some_and(|code| code == 0 || code == 1)
    {
        return parse_effective_config(&scoped.stdout).map_err(|_| invalid_output("filterConfig"));
    }

    let legacy = run_filter_config_query_async(git, cwd, /*show_scope*/ false).await?;
    if !legacy
        .status
        .code()
        .is_some_and(|code| code == 0 || code == 1)
    {
        return Err(command_failed("filterConfig", legacy.status.code()));
    }
    parse_effective_config_with_origins(&legacy.stdout).map_err(|_| invalid_output("filterConfig"))
}

async fn run_filter_config_query_async(
    git: &GitRunner,
    cwd: &Path,
    show_scope: bool,
) -> Result<std::process::Output, GitReadError> {
    let mut command = git.tokio_command();
    command.args(["config", "--null"]);
    if show_scope {
        command.arg("--show-scope");
    }
    command
        .args([
            "--show-origin",
            "--includes",
            "--get-regexp",
            EXECUTABLE_FILTER_CONFIG_PATTERN,
        ])
        .current_dir(cwd)
        .kill_on_drop(true);
    command_output(git, command, "filterConfig").await
}

#[derive(Clone, Copy)]
enum PathSelection {
    Tracked,
}

async fn read_paths_async(
    git: &GitRunner,
    cwd: &Path,
    selection: PathSelection,
) -> Result<Vec<Vec<u8>>, GitReadError> {
    let mut command = git.tokio_command();
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
    }
    command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(args)
        .current_dir(cwd)
        .kill_on_drop(true);
    let operation = match selection {
        PathSelection::Tracked => "trackedPaths",
    };
    let output = command_output(git, command, operation).await?;
    if !output.status.success() {
        return Err(command_failed(operation, output.status.code()));
    }
    parse_nul_paths(&output.stdout).map_err(|_| invalid_output(operation))
}

async fn read_filter_attributes_async(
    git: &GitRunner,
    cwd: &Path,
    paths: &[Vec<u8>],
) -> Result<BTreeMap<Vec<u8>, FilterAttributeValue>, GitReadError> {
    if paths.is_empty() {
        return Ok(BTreeMap::new());
    }
    let mut input = tempfile::tempfile()
        .map_err(|_| command_failed("filterAttributes", /*exit_code*/ None))?;
    write_nul_paths(&mut input, paths).map_err(|_| invalid_output("filterAttributes"))?;
    input
        .rewind()
        .map_err(|_| command_failed("filterAttributes", /*exit_code*/ None))?;

    let mut command = git.tokio_command();
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
    let output = command_output(git, command, "filterAttributes").await?;
    if !output.status.success() {
        return Err(command_failed("filterAttributes", output.status.code()));
    }
    parse_filter_attributes(&output.stdout, paths).map_err(|_| invalid_output("filterAttributes"))
}

async fn resolve_filter_attribute_sentinels_async(
    git: &GitRunner,
    cwd: &Path,
    attributes: BTreeMap<Vec<u8>, FilterAttributeValue>,
    executable_drivers: &BTreeSet<String>,
) -> Result<BTreeMap<Vec<u8>, String>, GitReadError> {
    let mut resolved = BTreeMap::new();
    for (path, attribute) in attributes {
        match attribute {
            FilterAttributeValue::Driver(driver) => {
                resolved.insert(path, driver);
            }
            FilterAttributeValue::AmbiguousSentinel(driver) => {
                if executable_drivers.contains(&driver)
                    && sentinel_spelling_selects_filter_driver_async(git, cwd, &path, &driver)
                        .await?
                {
                    resolved.insert(path, driver);
                }
            }
        }
    }
    Ok(resolved)
}

async fn sentinel_spelling_selects_filter_driver_async(
    git: &GitRunner,
    cwd: &Path,
    path: &[u8],
    driver: &str,
) -> Result<bool, GitReadError> {
    let required =
        run_sentinel_selection_probe_async(git, cwd, path, driver, /*required*/ true).await?;
    if required.status.success() {
        return Ok(false);
    }
    let optional =
        run_sentinel_selection_probe_async(git, cwd, path, driver, /*required*/ false).await?;
    if optional.status.success() {
        return Ok(true);
    }
    Err(command_failed(
        "filterAttributeSelection",
        optional.status.code(),
    ))
}

async fn run_sentinel_selection_probe_async(
    git: &GitRunner,
    cwd: &Path,
    path: &[u8],
    driver: &str,
    required: bool,
) -> Result<std::process::Output, GitReadError> {
    let path = git_path_argument(path).map_err(|_| invalid_output("filterAttributeSelection"))?;
    let mut command = git.tokio_command();
    command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args([
            "-c",
            &format!("core.hooksPath={DISABLED_HOOKS_PATH}"),
            "-c",
            "core.fsmonitor=false",
            "-c",
            &format!("filter.{driver}.required={required}"),
            "-c",
            &format!("filter.{driver}.clean="),
            "-c",
            &format!("filter.{driver}.smudge="),
            "-c",
            &format!("filter.{driver}.process="),
            "hash-object",
            "--stdin",
        ])
        .arg("--path")
        .arg(path)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .kill_on_drop(true);
    command_output(git, command, "filterAttributeSelection").await
}

async fn command_output(
    git: &GitRunner,
    command: TokioCommand,
    operation: &str,
) -> Result<std::process::Output, GitReadError> {
    match timeout(GIT_COMMAND_TIMEOUT, git.output_tokio(command)).await {
        Err(_) => Err(GitReadError::CommandTimedOut {
            operation: operation.to_string(),
        }),
        Ok(Err(error)) if error.kind() == io::ErrorKind::NotFound => {
            Err(GitReadError::NoTrustedGit)
        }
        Ok(Err(_)) => Err(command_failed(operation, /*exit_code*/ None)),
        Ok(Ok(output)) => Ok(output),
    }
}

fn command_failed(operation: &str, exit_code: Option<i32>) -> GitReadError {
    GitReadError::CommandFailed {
        operation: operation.to_string(),
        exit_code,
    }
}

fn invalid_output(operation: &str) -> GitReadError {
    GitReadError::InvalidOutput {
        operation: operation.to_string(),
    }
}
