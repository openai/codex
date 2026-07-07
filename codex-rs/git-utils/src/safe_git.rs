use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io;
use std::io::Seek;
use std::io::Write;
use std::process::Command;
use std::process::Stdio;
use tokio::time::Duration;

use crate::git_config::GitConfigEntry;
use crate::guarded_config::GuardedGitConfig;
use crate::guarded_config::SealedFilterConfigOverride;

#[path = "filter_sentinel.rs"]
mod filter_sentinel;
pub(crate) use filter_sentinel::SentinelFilterProbeBudget;
pub(crate) use filter_sentinel::SentinelFilterProbeResolution;
pub(crate) use filter_sentinel::classify_sentinel_filter_probes;
#[cfg(test)]
pub(crate) use filter_sentinel::sentinel_filter_probe_config_args;
pub(crate) const DISABLED_HOOKS_PATH: &str = if cfg!(windows) { "NUL" } else { "/dev/null" };
pub(crate) const EXECUTABLE_FILTER_CONFIG_PATTERN: &str =
    r"^filter\..*\.(clean|smudge|process|required)$";
pub(crate) const MAX_EXECUTABLE_FILTER_DRIVERS: usize = 256;
/// Timeout for internal Git commands to prevent freezing on large repositories.
pub(crate) const GIT_COMMAND_TIMEOUT: Duration = Duration::from_secs(5);
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FilterAttributeValue {
    Driver(String),
    AmbiguousSentinel(String),
}

/// Complete executable-filter inventory derived from one authorized effective
/// config read. The private field prevents a caller from minting an incomplete
/// inventory and treating its neutralizer as complete.
pub(crate) struct ExecutableFilterDrivers(BTreeSet<String>);

impl ExecutableFilterDrivers {
    pub(crate) fn iter(&self) -> impl Iterator<Item = &String> {
        self.0.iter()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn contains(&self, driver: &str) -> bool {
        self.0.contains(driver)
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FilterPolicyRole {
    Apply,
    GitAdd,
}

pub(crate) struct FilterPolicySnapshot {
    role: FilterPolicyRole,
    checked_paths: BTreeSet<String>,
    neutralizer: Option<SealedFilterConfigOverride>,
}

impl FilterPolicySnapshot {
    pub(crate) fn neutralizer(&self) -> Option<&SealedFilterConfigOverride> {
        self.neutralizer.as_ref()
    }

    pub(crate) fn role(&self) -> FilterPolicyRole {
        self.role
    }

    pub(crate) fn contains_checked_path(&self, path: &str) -> bool {
        self.checked_paths.contains(path)
    }

    pub(crate) fn checked_paths(&self) -> Vec<String> {
        self.checked_paths.iter().cloned().collect()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SelectedFilterPolicy {
    Refused,
    NeedsRequiredValue,
    Allowed,
}

pub(crate) fn validate_executable_driver_count(
    executable_drivers: &ExecutableFilterDrivers,
) -> io::Result<()> {
    if executable_drivers.len() > MAX_EXECUTABLE_FILTER_DRIVERS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Git filter driver count {} exceeds the status guard limit {}",
                executable_drivers.len(),
                MAX_EXECUTABLE_FILTER_DRIVERS
            ),
        ));
    }
    Ok(())
}

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

pub(crate) fn build_filter_policy_snapshot(
    config: &GuardedGitConfig<'_>,
    paths: &[String],
    execution: FilterExecution,
) -> io::Result<FilterPolicySnapshot> {
    let entries = read_filter_config(config).map_err(|error| {
        if matches!(execution, FilterExecution::GitAdd)
            && error.kind() == io::ErrorKind::InvalidData
        {
            io::Error::new(
                io::ErrorKind::Unsupported,
                format!("refusing malformed Git filter configuration: {error}"),
            )
        } else {
            error
        }
    })?;
    let executable_drivers = executable_filter_drivers(&entries)?;
    let role = match execution {
        FilterExecution::AnyWorktreeOperation => FilterPolicyRole::Apply,
        FilterExecution::GitAdd => FilterPolicyRole::GitAdd,
    };
    let checked_paths = paths.iter().cloned().collect();
    if executable_drivers.is_empty() {
        return Ok(FilterPolicySnapshot {
            role,
            checked_paths,
            neutralizer: None,
        });
    }
    let neutralizer = config.build_filter_override(&executable_drivers)?;
    #[cfg(test)]
    FILTER_POLICY_OVERLAY_COUNT.with(|count| count.set(count.get() + 1));
    let paths = paths
        .iter()
        .map(|path| path.as_bytes().to_vec())
        .collect::<Vec<_>>();
    let attributes = read_filter_attributes(config, &paths, &executable_drivers, &neutralizer)?;
    let mut required_cache = BTreeMap::new();
    for (path, driver) in &attributes {
        if !executable_drivers.contains(driver) {
            continue;
        }
        let refused = match execution {
            FilterExecution::AnyWorktreeOperation => true,
            FilterExecution::GitAdd => {
                git_add_filter_is_refused(config, &entries, driver, &mut required_cache)?
            }
        };
        if refused {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!(
                    "refusing to run an internal Git worktree operation with executable filter {driver:?} selected for {}",
                    String::from_utf8_lossy(path)
                ),
            ));
        }
    }
    Ok(FilterPolicySnapshot {
        role,
        checked_paths,
        neutralizer: Some(neutralizer),
    })
}

fn git_add_filter_is_refused(
    config: &GuardedGitConfig<'_>,
    entries: &BTreeMap<String, GitConfigEntry>,
    driver: &str,
    required_cache: &mut BTreeMap<String, bool>,
) -> io::Result<bool> {
    let required = required_cache.get(driver).copied();
    match classify_selected_filter(entries, driver, required) {
        SelectedFilterPolicy::Refused => return Ok(true),
        SelectedFilterPolicy::Allowed => return Ok(false),
        SelectedFilterPolicy::NeedsRequiredValue => {}
    }
    let required = config
        .read_bool(&format!("filter.{driver}.required"))
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::Unsupported,
                format!(
                    "refusing selected Git filter {driver:?} with malformed required value: {error}"
                ),
            )
        })?
        .unwrap_or(false);
    required_cache.insert(driver.to_string(), required);
    Ok(matches!(
        classify_selected_filter(entries, driver, Some(required)),
        SelectedFilterPolicy::Refused
    ))
}

#[derive(Clone, Copy)]
pub(crate) enum FilterExecution {
    AnyWorktreeOperation,
    GitAdd,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FilterCommand {
    Clean,
    Smudge,
    Process,
}

fn read_filter_config(
    config: &GuardedGitConfig<'_>,
) -> io::Result<BTreeMap<String, GitConfigEntry>> {
    #[cfg(test)]
    FILTER_POLICY_READ_COUNT.with(|count| count.set(count.get() + 1));
    config.read_effective(EXECUTABLE_FILTER_CONFIG_PATTERN, "filter")
}

fn read_filter_attributes(
    config: &GuardedGitConfig<'_>,
    paths: &[Vec<u8>],
    executable_drivers: &ExecutableFilterDrivers,
    neutralization: &SealedFilterConfigOverride,
) -> io::Result<BTreeMap<Vec<u8>, String>> {
    if paths.is_empty() {
        return Ok(BTreeMap::new());
    }
    let mut input = tempfile::tempfile()?;
    write_nul_paths(&mut input, paths)?;
    input.rewind()?;

    let mut command = config.pending_filter_attribute_command(neutralization)?;
    command
        .disable_optional_locks()
        .args(["--stdin", "-z", "filter"])
        .stdin(Stdio::from(input));
    let output = command.output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "git filter attribute probe failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let attributes = parse_filter_attributes(&output.stdout, paths)?;
    resolve_filter_attribute_sentinels(config, attributes, executable_drivers, neutralization)
}

fn resolve_filter_attribute_sentinels(
    config: &GuardedGitConfig<'_>,
    attributes: BTreeMap<Vec<u8>, FilterAttributeValue>,
    executable_drivers: &ExecutableFilterDrivers,
    neutralization: &SealedFilterConfigOverride,
) -> io::Result<BTreeMap<Vec<u8>, String>> {
    let mut resolved = BTreeMap::new();
    let mut probe_budget = SentinelFilterProbeBudget::default();
    for (path, attribute) in attributes {
        match attribute {
            FilterAttributeValue::Driver(driver) => {
                resolved.insert(path, driver);
            }
            FilterAttributeValue::AmbiguousSentinel(driver) => {
                if executable_drivers.contains(&driver)
                    && sentinel_spelling_selects_filter_driver(
                        config,
                        &path,
                        &driver,
                        neutralization,
                        &mut probe_budget,
                    )?
                {
                    resolved.insert(path, driver);
                }
            }
        }
    }
    Ok(resolved)
}

/// Disambiguate Git's sentinel spellings with required/optional probes. The
/// shared guard blanks every known executable driver before either probe.
fn sentinel_spelling_selects_filter_driver(
    config: &GuardedGitConfig<'_>,
    path: &[u8],
    driver: &str,
    neutralization: &SealedFilterConfigOverride,
    probe_budget: &mut SentinelFilterProbeBudget,
) -> io::Result<bool> {
    let probe = SentinelSelectionProbe {
        config,
        path,
        driver,
        neutralization,
    };
    let required = probe.run(/*required*/ true, probe_budget)?;
    if classify_sentinel_filter_probes(required.status.success(), /*optional_succeeded*/ None)
        == SentinelFilterProbeResolution::SpecialAttributeState
    {
        return Ok(false);
    }
    let optional = probe.run(/*required*/ false, probe_budget)?;
    if classify_sentinel_filter_probes(
        required.status.success(),
        /*optional_succeeded*/ Some(optional.status.success()),
    ) == SentinelFilterProbeResolution::LiteralDriver
    {
        return Ok(true);
    }
    Err(io::Error::other(format!(
        "git filter attribute selection probe failed with required status {} and optional status {}: {}",
        required.status,
        optional.status,
        String::from_utf8_lossy(&optional.stderr).trim()
    )))
}

struct SentinelSelectionProbe<'a> {
    config: &'a GuardedGitConfig<'a>,
    path: &'a [u8],
    driver: &'a str,
    neutralization: &'a SealedFilterConfigOverride,
}

impl SentinelSelectionProbe<'_> {
    fn run(
        &self,
        required: bool,
        probe_budget: &mut SentinelFilterProbeBudget,
    ) -> io::Result<std::process::Output> {
        if !matches!(self.driver, "set" | "unset" | "unspecified") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Git filter sentinel probe requested for a non-sentinel driver",
            ));
        }
        let path = git_path_argument(self.path)?;
        let mut command = self.config.command_for_sentinel_filter_probe(
            self.neutralization,
            self.driver,
            required,
        )?;
        command
            .disable_optional_locks()
            .arg("--stdin")
            .arg("--path")
            .arg(path)
            .stdin(Stdio::null());
        probe_budget.ensure_probe_available()?;
        let output = command.output()?;
        probe_budget.record_completed_probe();
        Ok(output)
    }
}

#[cfg(unix)]
pub(crate) fn git_path_argument(path: &[u8]) -> io::Result<std::ffi::OsString> {
    use std::os::unix::ffi::OsStringExt;

    Ok(std::ffi::OsString::from_vec(path.to_vec()))
}

#[cfg(not(unix))]
pub(crate) fn git_path_argument(path: &[u8]) -> io::Result<std::ffi::OsString> {
    let path = std::str::from_utf8(path)
        .map_err(|_| invalid_filter_output("non-UTF-8 Git filter attribute path"))?;
    Ok(path.into())
}

#[cfg(test)]
fn selected_executable_filter_for(
    entries: &BTreeMap<String, GitConfigEntry>,
    attributes: &BTreeMap<Vec<u8>, String>,
    execution: FilterExecution,
) -> io::Result<Option<(String, Vec<u8>)>> {
    let executable_drivers = executable_filter_drivers_for(entries, execution)?;
    Ok(selected_filter(&executable_drivers, attributes))
}

#[cfg(test)]
fn executable_filter_drivers_for(
    entries: &BTreeMap<String, GitConfigEntry>,
    execution: FilterExecution,
) -> io::Result<BTreeSet<String>> {
    let mut drivers = BTreeSet::new();
    for entry in entries.values() {
        if entry.key.ends_with(".required") {
            continue;
        }
        let (driver, command) = filter_driver_and_command(&entry.key)?;
        let relevant = match execution {
            FilterExecution::AnyWorktreeOperation => true,
            FilterExecution::GitAdd => command != FilterCommand::Smudge,
        };
        if relevant && !entry.value.is_empty() {
            drivers.insert(driver);
        }
    }
    Ok(drivers)
}

#[cfg(test)]
fn selected_filter(
    drivers: &BTreeSet<String>,
    attributes: &BTreeMap<Vec<u8>, String>,
) -> Option<(String, Vec<u8>)> {
    for (path, driver) in attributes {
        if drivers.contains(driver) {
            return Some((driver.clone(), path.clone()));
        }
    }
    None
}

#[cfg(test)]
fn selected_executable_filter(
    entries: &BTreeMap<String, GitConfigEntry>,
    attributes: &BTreeMap<Vec<u8>, String>,
) -> io::Result<Option<(String, Vec<u8>)>> {
    selected_executable_filter_for(entries, attributes, FilterExecution::AnyWorktreeOperation)
}

pub(crate) fn executable_filter_drivers(
    entries: &BTreeMap<String, GitConfigEntry>,
) -> io::Result<ExecutableFilterDrivers> {
    let mut executable_drivers = BTreeSet::new();
    for entry in entries.values() {
        if entry.key.ends_with(".required") {
            continue;
        }
        let (driver, _command) = filter_driver_and_command(&entry.key)?;
        if !entry.value.is_empty() {
            executable_drivers.insert(driver);
        }
    }
    Ok(ExecutableFilterDrivers(executable_drivers))
}

/// Status must also inspect required-only namespaces. A selected
/// `filter.<name>.required=true` with no clean/process command makes native
/// status fail closed, so silently resetting it would turn an unavailable
/// result into a potentially misleading Boolean.
pub(crate) fn status_policy_filter_drivers(
    entries: &BTreeMap<String, GitConfigEntry>,
) -> io::Result<ExecutableFilterDrivers> {
    let mut drivers = executable_filter_drivers(entries)?.0;
    for entry in entries.values() {
        let Some(remainder) = entry.key.strip_prefix("filter.") else {
            return Err(invalid_filter_output("malformed filter config key"));
        };
        if let Some(driver) = remainder.strip_suffix(".required") {
            drivers.insert(driver.to_string());
        }
    }
    Ok(ExecutableFilterDrivers(drivers))
}

fn effective_filter_value<'a>(
    entries: &'a BTreeMap<String, GitConfigEntry>,
    driver: &str,
    name: &str,
) -> Option<&'a str> {
    entries
        .get(&format!("filter.{driver}.{name}"))
        .map(|entry| entry.value.as_str())
}

pub(crate) fn classify_selected_filter(
    entries: &BTreeMap<String, GitConfigEntry>,
    driver: &str,
    required: Option<bool>,
) -> SelectedFilterPolicy {
    if ["clean", "process"].into_iter().any(|name| {
        effective_filter_value(entries, driver, name).is_some_and(|value| !value.is_empty())
    }) {
        return SelectedFilterPolicy::Refused;
    }
    match required {
        None => SelectedFilterPolicy::NeedsRequiredValue,
        Some(true) => SelectedFilterPolicy::Refused,
        Some(false) => SelectedFilterPolicy::Allowed,
    }
}
#[cfg(test)]
fn filter_driver_name(key: &str) -> io::Result<String> {
    filter_driver_and_command(key).map(|(driver, _command)| driver)
}

fn filter_driver_and_command(key: &str) -> io::Result<(String, FilterCommand)> {
    let Some(remainder) = key.strip_prefix("filter.") else {
        return Err(invalid_filter_output("malformed filter config key"));
    };
    let (driver, command) = [
        (".clean", FilterCommand::Clean),
        (".smudge", FilterCommand::Smudge),
        (".process", FilterCommand::Process),
    ]
    .into_iter()
    .find_map(|(suffix, command)| {
        remainder
            .strip_suffix(suffix)
            .map(|driver| (driver, command))
    })
    .ok_or_else(|| invalid_filter_output("malformed filter config key"))?;
    Ok((driver.to_string(), command))
}

pub(crate) fn parse_nul_paths(output: &[u8]) -> io::Result<Vec<Vec<u8>>> {
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

pub(crate) fn write_nul_paths(input: &mut std::fs::File, paths: &[Vec<u8>]) -> io::Result<()> {
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

pub(crate) fn parse_filter_attributes(
    output: &[u8],
    expected_paths: &[Vec<u8>],
) -> io::Result<BTreeMap<Vec<u8>, FilterAttributeValue>> {
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
        let value = match driver {
            "set" | "unset" | "unspecified" => {
                FilterAttributeValue::AmbiguousSentinel(driver.to_string())
            }
            _ => FilterAttributeValue::Driver(driver.to_string()),
        };
        if attributes.insert(record[0].to_vec(), value).is_some() {
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
thread_local! {
    static FILTER_POLICY_READ_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static FILTER_POLICY_OVERLAY_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_filter_policy_counts() {
    FILTER_POLICY_READ_COUNT.with(|count| count.set(0));
    FILTER_POLICY_OVERLAY_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn filter_policy_read_count() -> usize {
    FILTER_POLICY_READ_COUNT.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(crate) fn filter_policy_overlay_count() -> usize {
    FILTER_POLICY_OVERLAY_COUNT.with(std::cell::Cell::get)
}

#[cfg(test)]
#[path = "safe_git_tests.rs"]
mod tests;
