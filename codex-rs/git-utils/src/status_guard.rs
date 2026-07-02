use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io;
use std::io::Seek;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;

#[cfg(test)]
use tokio::time::timeout;

use crate::GitReadError;
use crate::git_command::GitAsyncCommand;
use crate::git_command::GitRunner;
use crate::git_command::MAX_INTERNAL_GIT_OUTPUT_BYTES;
use crate::repository_authority::is_authority_refusal;
use crate::safe_git::DISABLED_HOOKS_PATH;
use crate::safe_git::FilterAttributeValue;
#[cfg(test)]
use crate::safe_git::GIT_COMMAND_TIMEOUT;
use crate::safe_git::GitFilterNeutralization;
use crate::safe_git::SelectedFilterPolicy;
use crate::safe_git::SentinelFilterProbeBudget;
use crate::safe_git::SentinelFilterProbeResolution;
use crate::safe_git::classify_sentinel_filter_probes;
use crate::safe_git::executable_filter_drivers;
use crate::safe_git::executable_filter_guard_async;
use crate::safe_git::git_path_argument;
use crate::safe_git::parse_filter_attributes;
use crate::safe_git::parse_git_filter_required_output;
use crate::safe_git::parse_nul_paths;
use crate::safe_git::read_filter_config_async;
use crate::safe_git::sentinel_filter_probe_config_args;
use crate::safe_git::write_nul_paths;

const MAX_STATUS_TRACKED_PATHS: usize = 250_000;

#[cfg(test)]
pub(crate) async fn prepare_status_filter_guard(
    git: &GitRunner,
    cwd: &Path,
) -> Result<(PathBuf, GitFilterNeutralization), GitReadError> {
    timeout(
        GIT_COMMAND_TIMEOUT,
        Box::pin(prepare_status_filter_guard_within_deadline(git, cwd)),
    )
    .await
    .map_err(|_| GitReadError::CommandTimedOut {
        operation: "statusFilterPreparation".to_string(),
    })?
}

pub(crate) async fn prepare_status_filter_guard_within_deadline(
    git: &GitRunner,
    cwd: &Path,
) -> Result<(PathBuf, GitFilterNeutralization), GitReadError> {
    let requested_cwd = std::fs::canonicalize(cwd).map_err(|_| GitReadError::NotRepository {
        path: cwd.to_path_buf(),
    })?;
    let authorized_root = git
        .active_worktree_root()
        .ok_or_else(|| GitReadError::NotRepository {
            path: requested_cwd.clone(),
        })?
        .to_path_buf();
    // Source authorization precedes the first config-consuming Git child and
    // is performed once for the retained active repository authority.
    let entries = read_filter_config_async(git, &authorized_root, &[])
        .await
        .map_err(|error| map_io_error("filterConfig", error))?;
    let git_root = resolve_git_root_async(git, &requested_cwd).await?;
    let executable_drivers =
        executable_filter_drivers(&entries).map_err(|_| invalid_output("filterConfig"))?;
    let filter_guard = executable_filter_guard_async(git, &git_root, entries, &executable_drivers)
        .await
        .map_err(|error| map_io_error("filterNeutralization", error))?;
    if executable_drivers.is_empty() {
        return Ok((git_root, filter_guard));
    }

    let paths = read_tracked_paths_async(git, &git_root).await?;
    let attributes = read_filter_attributes_async(git, &git_root, &paths).await?;
    let attributes = resolve_filter_attribute_sentinels_async(
        git,
        &git_root,
        attributes,
        &executable_drivers,
        &filter_guard,
    )
    .await?;
    let mut required_cache = BTreeMap::new();
    for (path, driver) in attributes {
        if !executable_drivers.contains(&driver) {
            continue;
        }
        let mut policy =
            filter_guard.selected_filter_policy(&driver, required_cache.get(&driver).copied());
        if policy == SelectedFilterPolicy::NeedsRequiredValue {
            let required = git_filter_required_async(git, &git_root, &driver).await?;
            required_cache.insert(driver.clone(), required);
            policy = filter_guard.selected_filter_policy(&driver, Some(required));
        }
        if policy == SelectedFilterPolicy::Refused {
            return Err(GitReadError::SelectedExecutableFilter {
                driver,
                path: String::from_utf8_lossy(&path).into_owned(),
            });
        }
    }
    Ok((git_root, filter_guard))
}

pub(crate) async fn resolve_git_root_async(
    git: &GitRunner,
    requested_cwd: &Path,
) -> Result<PathBuf, GitReadError> {
    let expected_root = git
        .active_worktree_root()
        .ok_or_else(|| GitReadError::NotRepository {
            path: requested_cwd.to_path_buf(),
        })?
        .to_path_buf();
    let mut command = git
        .async_command_for_cwd(requested_cwd)
        .map_err(|error| map_io_error("resolveGitRoot", error))?;
    command.env("GIT_OPTIONAL_LOCKS", "0").args([
        "-c",
        &format!("core.hooksPath={DISABLED_HOOKS_PATH}"),
        "-c",
        "core.fsmonitor=false",
        "rev-parse",
        "--show-toplevel",
    ]);
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

async fn read_tracked_paths_async(
    git: &GitRunner,
    cwd: &Path,
) -> Result<Vec<Vec<u8>>, GitReadError> {
    let mut command = git
        .async_command_for_cwd(cwd)
        .map_err(|error| map_io_error("trackedPaths", error))?;
    command.env("GIT_OPTIONAL_LOCKS", "0").args([
        "-c",
        &format!("core.hooksPath={DISABLED_HOOKS_PATH}"),
        "-c",
        "core.fsmonitor=false",
        "ls-files",
        "-z",
        "--cached",
    ]);
    let output = command_output(git, command, "trackedPaths").await?;
    if !output.status.success() {
        return Err(command_failed("trackedPaths", output.status.code()));
    }
    let paths = parse_nul_paths(&output.stdout).map_err(|_| invalid_output("trackedPaths"))?;
    if paths.len() > MAX_STATUS_TRACKED_PATHS {
        return Err(invalid_output("trackedPaths"));
    }
    Ok(paths)
}

async fn read_filter_attributes_async(
    git: &GitRunner,
    cwd: &Path,
    paths: &[Vec<u8>],
) -> Result<BTreeMap<Vec<u8>, FilterAttributeValue>, GitReadError> {
    if paths.is_empty() {
        return Ok(BTreeMap::new());
    }
    let mut input =
        tempfile::tempfile().map_err(|error| map_io_error("filterAttributes", error))?;
    write_nul_paths(&mut input, paths).map_err(|_| invalid_output("filterAttributes"))?;
    input
        .rewind()
        .map_err(|error| map_io_error("filterAttributes", error))?;

    let mut command = git
        .async_command_for_cwd(cwd)
        .map_err(|error| map_io_error("filterAttributes", error))?;
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
        .stdin(Stdio::from(input));
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
    filter_guard: &GitFilterNeutralization,
) -> Result<BTreeMap<Vec<u8>, String>, GitReadError> {
    let mut resolved = BTreeMap::new();
    let mut probe_budget = SentinelFilterProbeBudget::default();
    for (path, attribute) in attributes {
        match attribute {
            FilterAttributeValue::Driver(driver) => {
                resolved.insert(path, driver);
            }
            FilterAttributeValue::AmbiguousSentinel(driver) => {
                if executable_drivers.contains(&driver)
                    && sentinel_spelling_selects_filter_driver_async(
                        git,
                        cwd,
                        &path,
                        &driver,
                        filter_guard,
                        &mut probe_budget,
                    )
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
    filter_guard: &GitFilterNeutralization,
    probe_budget: &mut SentinelFilterProbeBudget,
) -> Result<bool, GitReadError> {
    let required = run_sentinel_selection_probe_async(
        git,
        cwd,
        path,
        driver,
        /*required*/ true,
        filter_guard,
        probe_budget,
    )
    .await?;
    if classify_sentinel_filter_probes(required.status.success(), /*optional_succeeded*/ None)
        == SentinelFilterProbeResolution::SpecialAttributeState
    {
        return Ok(false);
    }
    let optional = run_sentinel_selection_probe_async(
        git,
        cwd,
        path,
        driver,
        /*required*/ false,
        filter_guard,
        probe_budget,
    )
    .await?;
    if classify_sentinel_filter_probes(required.status.success(), Some(optional.status.success()))
        == SentinelFilterProbeResolution::LiteralDriver
    {
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
    filter_guard: &GitFilterNeutralization,
    probe_budget: &mut SentinelFilterProbeBudget,
) -> Result<std::process::Output, GitReadError> {
    probe_budget.ensure_probe_available().map_err(|_| {
        GitReadError::FilterSelectionProbeLimitExceeded {
            max_probes: SentinelFilterProbeBudget::max_probes(),
        }
    })?;
    let path = git_path_argument(path).map_err(|_| invalid_output("filterAttributeSelection"))?;
    let probe_config_args =
        sentinel_filter_probe_config_args(filter_guard.git_config_args(), driver, required)
            .map_err(|_| invalid_output("filterAttributeSelection"))?;
    let mut command = git
        .async_command_for_cwd(cwd)
        .map_err(|error| map_io_error("filterAttributeSelection", error))?;
    command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args([
            "-c",
            &format!("core.hooksPath={DISABLED_HOOKS_PATH}"),
            "-c",
            "core.fsmonitor=false",
        ])
        .args(&probe_config_args)
        .args(["hash-object", "--stdin"])
        .arg("--path")
        .arg(path)
        .stdin(Stdio::null());
    let output = command_output(git, command, "filterAttributeSelection").await?;
    probe_budget.record_completed_probe();
    Ok(output)
}

async fn git_filter_required_async(
    git: &GitRunner,
    cwd: &Path,
    driver: &str,
) -> Result<bool, GitReadError> {
    let mut command = git
        .async_command_for_cwd(cwd)
        .map_err(|error| map_io_error("filterRequired", error))?;
    command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(["config", "--type=bool", "--get"])
        .arg(format!("filter.{driver}.required"));
    let output = command_output(git, command, "filterRequired").await?;
    parse_git_filter_required_output(&output, driver)
        .map_err(|error| map_io_error("filterRequired", error))
}

async fn command_output(
    git: &GitRunner,
    command: GitAsyncCommand,
    operation: &str,
) -> Result<std::process::Output, GitReadError> {
    git.output_async_bounded(command, MAX_INTERNAL_GIT_OUTPUT_BYTES)
        .await
        .map_err(|error| map_io_error(operation, error))
}

pub(crate) fn map_io_error(operation: &str, error: io::Error) -> GitReadError {
    if is_authority_refusal(&error) {
        return GitReadError::AuthorityRefused {
            operation: operation.to_string(),
        };
    }
    match error.kind() {
        io::ErrorKind::TimedOut => GitReadError::CommandTimedOut {
            operation: operation.to_string(),
        },
        io::ErrorKind::InvalidData => invalid_output(operation),
        _ => command_failed(operation, /*exit_code*/ None),
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

#[cfg(test)]
#[path = "status_guard_tests.rs"]
mod tests;
