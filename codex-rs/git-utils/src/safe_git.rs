use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io;
use std::io::Seek;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::process::Stdio;

use crate::git_command::GitRunner;
use crate::git_config::GitConfigEntry;
use crate::git_config::read_effective_config_with_fallback as read_effective_config_unchecked;
use crate::git_config_sources::ensure_no_worktree_config_sources;

#[path = "filter_sentinel.rs"]
mod filter_sentinel;
pub(crate) use filter_sentinel::SentinelFilterProbeBudget;
pub(crate) use filter_sentinel::SentinelFilterProbeResolution;
pub(crate) use filter_sentinel::classify_sentinel_filter_probes;
pub(crate) use filter_sentinel::sentinel_filter_probe_config_args;
pub(crate) const DISABLED_HOOKS_PATH: &str = if cfg!(windows) { "NUL" } else { "/dev/null" };
pub(crate) const EXECUTABLE_FILTER_CONFIG_PATTERN: &str =
    r"^filter\..*\.(clean|smudge|process|required)$";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FilterAttributeValue {
    Driver(String),
    AmbiguousSentinel(String),
}

pub(crate) struct GitFilterNeutralization {
    git_config_args: Vec<String>,
    _config_dir: Option<tempfile::TempDir>,
    filter_config: BTreeMap<String, GitConfigEntry>,
}

impl GitFilterNeutralization {
    pub(crate) fn git_config_args(&self) -> &[String] {
        &self.git_config_args
    }

    pub(crate) fn filter_value(&self, driver: &str, name: &str) -> Option<&str> {
        self.filter_config
            .get(&format!("filter.{driver}.{name}"))
            .map(|entry| entry.value.as_str())
    }
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

pub(crate) fn ensure_no_selected_executable_git_filters(
    git: &GitRunner,
    cwd: &Path,
    paths: &[String],
    git_config_args: &[String],
) -> io::Result<GitFilterNeutralization> {
    let entries = read_filter_config(git, cwd, git_config_args)?;
    let executable_drivers = executable_filter_drivers(&entries)?;
    if executable_drivers.is_empty() {
        return Ok(GitFilterNeutralization {
            git_config_args: Vec::new(),
            _config_dir: None,
            filter_config: entries,
        });
    }
    let guard = executable_filter_guard(git, cwd, entries, &executable_drivers)?;
    let paths = paths
        .iter()
        .map(|path| path.as_bytes().to_vec())
        .collect::<Vec<_>>();
    let attributes = read_filter_attributes(
        git,
        cwd,
        &paths,
        git_config_args,
        &executable_drivers,
        &guard,
    )?;
    if let Some((driver, path)) = selected_executable_filter(&guard.filter_config, &attributes)? {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "refusing to run an internal Git worktree operation with executable filter {driver:?} selected for {}",
                String::from_utf8_lossy(&path)
            ),
        ));
    }
    Ok(guard)
}

fn executable_filter_guard(
    git: &GitRunner,
    cwd: &Path,
    filter_config: BTreeMap<String, GitConfigEntry>,
    executable_drivers: &BTreeSet<String>,
) -> io::Result<GitFilterNeutralization> {
    let config_dir = tempfile::tempdir()?;
    let config_path = config_dir.path().join("filter-neutralization.gitconfig");
    std::fs::write(&config_path, [])?;
    let guard = GitFilterNeutralization {
        git_config_args: vec![
            "-c".to_string(),
            format!(
                "include.path={}",
                config_path
                    .to_str()
                    .ok_or_else(|| invalid_filter_output("non-UTF-8 filter guard path"))?
            ),
        ],
        _config_dir: Some(config_dir),
        filter_config,
    };
    for driver in executable_drivers {
        debug_assert!(["clean", "smudge", "process"].into_iter().any(|name| {
            guard
                .filter_value(driver, name)
                .is_some_and(|value| !value.is_empty())
        }));
        for command in ["clean", "smudge", "process"] {
            guard.write_config_value(git, cwd, &config_path, driver, command, "")?;
        }
        guard.write_config_value(git, cwd, &config_path, driver, "required", "false")?;
    }
    Ok(guard)
}

impl GitFilterNeutralization {
    fn write_config_value(
        &self,
        git: &GitRunner,
        cwd: &Path,
        config_path: &Path,
        driver: &str,
        name: &str,
        value: &str,
    ) -> io::Result<()> {
        let mut command = git.command_for_cwd(cwd)?;
        command.args(["config", "--file"]).arg(config_path).args([
            "--add",
            &format!("filter.{driver}.{name}"),
            value,
        ]);
        let output = git.output(command)?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "failed to write Git filter neutralization for {driver:?} (status {}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Ok(())
    }
}

fn read_filter_config(
    git: &GitRunner,
    cwd: &Path,
    git_config_args: &[String],
) -> io::Result<BTreeMap<String, GitConfigEntry>> {
    read_effective_config_with_fallback(
        git,
        cwd,
        git_config_args,
        EXECUTABLE_FILTER_CONFIG_PATTERN,
        "filter",
    )
}

pub(crate) fn read_effective_config_with_fallback(
    git: &GitRunner,
    cwd: &Path,
    git_config_args: &[String],
    pattern: &str,
    probe: &str,
) -> io::Result<BTreeMap<String, GitConfigEntry>> {
    ensure_no_worktree_config_sources(git, cwd, git_config_args)?;
    read_effective_config_unchecked(git, cwd, git_config_args, pattern, probe)
}

fn read_filter_attributes(
    git: &GitRunner,
    cwd: &Path,
    paths: &[Vec<u8>],
    git_config_args: &[String],
    executable_drivers: &BTreeSet<String>,
    neutralization: &GitFilterNeutralization,
) -> io::Result<BTreeMap<Vec<u8>, String>> {
    if paths.is_empty() {
        return Ok(BTreeMap::new());
    }
    let mut input = tempfile::tempfile()?;
    write_nul_paths(&mut input, paths)?;
    input.rewind()?;

    let mut command = git.command_for_cwd(cwd)?;
    command
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
        .stdin(Stdio::from(input));
    let output = git.output(command)?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "git filter attribute probe failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let attributes = parse_filter_attributes(&output.stdout, paths)?;
    resolve_filter_attribute_sentinels(
        git,
        cwd,
        attributes,
        git_config_args,
        executable_drivers,
        neutralization,
    )
}

fn resolve_filter_attribute_sentinels(
    git: &GitRunner,
    cwd: &Path,
    attributes: BTreeMap<Vec<u8>, FilterAttributeValue>,
    git_config_args: &[String],
    executable_drivers: &BTreeSet<String>,
    neutralization: &GitFilterNeutralization,
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
                        git,
                        cwd,
                        &path,
                        &driver,
                        git_config_args,
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
    git: &GitRunner,
    cwd: &Path,
    path: &[u8],
    driver: &str,
    git_config_args: &[String],
    neutralization: &GitFilterNeutralization,
    probe_budget: &mut SentinelFilterProbeBudget,
) -> io::Result<bool> {
    let probe = SentinelSelectionProbe {
        git,
        cwd,
        path,
        driver,
        git_config_args,
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
    git: &'a GitRunner,
    cwd: &'a Path,
    path: &'a [u8],
    driver: &'a str,
    git_config_args: &'a [String],
    neutralization: &'a GitFilterNeutralization,
}

impl SentinelSelectionProbe<'_> {
    fn run(
        &self,
        required: bool,
        probe_budget: &mut SentinelFilterProbeBudget,
    ) -> io::Result<std::process::Output> {
        let probe_config_args = sentinel_filter_probe_config_args(
            self.neutralization.git_config_args(),
            self.driver,
            required,
        )?;
        let path = git_path_argument(self.path)?;
        let mut command = self.git.command_for_cwd(self.cwd)?;
        command
            .env("GIT_OPTIONAL_LOCKS", "0")
            .args(self.git_config_args)
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
        probe_budget.ensure_probe_available()?;
        let output = self.git.output(command)?;
        probe_budget.record_completed_probe();
        Ok(output)
    }
}

#[cfg(unix)]
fn git_path_argument(path: &[u8]) -> io::Result<std::ffi::OsString> {
    use std::os::unix::ffi::OsStringExt;

    Ok(std::ffi::OsString::from_vec(path.to_vec()))
}

#[cfg(not(unix))]
fn git_path_argument(path: &[u8]) -> io::Result<std::ffi::OsString> {
    let path = std::str::from_utf8(path)
        .map_err(|_| invalid_filter_output("non-UTF-8 Git filter attribute path"))?;
    Ok(path.into())
}

fn selected_executable_filter(
    entries: &BTreeMap<String, GitConfigEntry>,
    attributes: &BTreeMap<Vec<u8>, String>,
) -> io::Result<Option<(String, Vec<u8>)>> {
    let executable_drivers = executable_filter_drivers(entries)?;
    for (path, driver) in attributes {
        if executable_drivers.contains(driver) {
            return Ok(Some((driver.clone(), path.clone())));
        }
    }
    Ok(None)
}

pub(crate) fn executable_filter_drivers(
    entries: &BTreeMap<String, GitConfigEntry>,
) -> io::Result<BTreeSet<String>> {
    let mut executable_drivers = BTreeSet::new();
    for entry in entries.values() {
        if entry.key.ends_with(".required") {
            continue;
        }
        let driver = filter_driver_name(&entry.key)?;
        if !entry.value.is_empty() {
            executable_drivers.insert(driver);
        }
    }
    Ok(executable_drivers)
}

fn filter_driver_name(key: &str) -> io::Result<String> {
    let Some(remainder) = key.strip_prefix("filter.") else {
        return Err(invalid_filter_output("malformed filter config key"));
    };
    let driver = [".clean", ".smudge", ".process"]
        .into_iter()
        .find_map(|suffix| remainder.strip_suffix(suffix))
        .ok_or_else(|| invalid_filter_output("malformed filter config key"))?;
    Ok(driver.to_string())
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
#[path = "safe_git_tests.rs"]
mod tests;
