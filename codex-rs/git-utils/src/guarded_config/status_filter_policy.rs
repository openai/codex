use std::collections::BTreeMap;
use std::io;
use std::io::Seek;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;

use super::FILTER_NEUTRALIZATION_PLAN;
use super::GuardedGitConfig;
use super::SealedFilterConfigOverride;
use super::status_command::command_failure;
use super::status_index::status_core_symlinks_for_filter_screening;
use super::status_policy::StatusPolicySnapshot;
use crate::FsmonitorOverride;
use crate::git_command::MAX_INTERNAL_GIT_OUTPUT_BYTES;
use crate::safe_git::ExecutableFilterDrivers;
use crate::safe_git::FilterAttributeValue;
use crate::safe_git::MAX_EXECUTABLE_FILTER_DRIVERS;
use crate::safe_git::SelectedFilterPolicy;
use crate::safe_git::SentinelFilterProbeBudget;
use crate::safe_git::classify_selected_filter;
use crate::safe_git::git_path_argument;
use crate::safe_git::parse_filter_attributes;
use crate::safe_git::parse_nul_paths;
use crate::safe_git::status_policy_filter_drivers;
use crate::safe_git::validate_executable_driver_count;
use crate::safe_git::write_nul_paths;

const MAX_STATUS_SENTINEL_PATHSPEC_BYTES: usize = 16 * 1024;
const MAX_STATUS_SENTINEL_STAGE_RECORDS: usize = 3;

#[derive(Debug, thiserror::Error)]
#[error("executable filter {driver:?} is selected for {path:?}")]
pub(crate) struct SelectedStatusFilterRefusal {
    driver: String,
    path: Vec<u8>,
}

impl SelectedStatusFilterRefusal {
    pub(crate) fn driver(&self) -> &str {
        &self.driver
    }

    pub(crate) fn path(&self) -> &[u8] {
        &self.path
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Git filter attribute selection exceeded its {max_probes}-probe limit")]
pub(crate) struct StatusFilterProbeLimitExceeded {
    max_probes: usize,
}

impl StatusFilterProbeLimitExceeded {
    pub(crate) fn max_probes(&self) -> usize {
        self.max_probes
    }
}

impl GuardedGitConfig<'_> {
    /// Install the mutually exclusive Status filter policy and publish one
    /// operation-owned config, attribute, HEAD, and untracked-presence view.
    pub(crate) async fn install_status_policy_async(&mut self) -> io::Result<()> {
        self.ensure_status_exclusive_state()?;
        if self.status.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "status filter policy is already installed",
            ));
        }
        #[cfg(test)]
        FILTER_POLICY_READ_COUNT_FOR_STATUS.with(|count| count.set(count.get() + 1));
        let entries = self
            .sources
            .read_effective_async(crate::safe_git::EXECUTABLE_FILTER_CONFIG_PATTERN, "filter")
            .await?;
        let executable_drivers = status_policy_filter_drivers(&entries)?;
        validate_executable_driver_count(&executable_drivers)?;
        let configured_core_symlinks = self.sources.read_bool_async("core.symlinks").await?;
        let core_symlinks = status_core_symlinks_for_filter_screening(configured_core_symlinks);
        let paths = self.read_status_tracked_paths_async(core_symlinks).await?;
        let neutralizer = if executable_drivers.is_empty() {
            None
        } else {
            Some(
                self.build_filter_override_async(&executable_drivers)
                    .await?,
            )
        };
        self.ensure_no_effective_replacement_refs_async(neutralizer.as_ref())
            .await?;
        if let Some(neutralizer) = &neutralizer {
            let attributes = self
                .read_status_filter_attributes_async(&paths, &executable_drivers, neutralizer)
                .await?;
            let mut required_cache = BTreeMap::new();
            for (path, driver) in attributes {
                if !executable_drivers.contains(&driver) {
                    continue;
                }
                let required = required_cache.get(&driver).copied();
                let mut policy = classify_selected_filter(&entries, &driver, required);
                if policy == SelectedFilterPolicy::NeedsRequiredValue {
                    let required = self
                        .sources
                        .read_bool_async(&format!("filter.{driver}.required"))
                        .await
                        .map_err(|error| {
                            io::Error::new(
                                io::ErrorKind::Unsupported,
                                format!(
                                    "refusing selected Git filter {driver:?} with malformed required value: {error}"
                                ),
                            )
                        })?
                        .unwrap_or(false);
                    required_cache.insert(driver.clone(), required);
                    policy = classify_selected_filter(&entries, &driver, Some(required));
                }
                if policy == SelectedFilterPolicy::Refused {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        SelectedStatusFilterRefusal { driver, path },
                    ));
                }
            }
        }
        let has_untracked = self
            .read_status_untracked_presence_async(neutralizer.as_ref())
            .await?;
        let head_oid = self
            .read_status_head_oid_async(neutralizer.as_ref())
            .await?;
        let context = self
            .build_status_read_context_async(
                head_oid.as_deref(),
                has_untracked,
                configured_core_symlinks,
            )
            .await?;
        self.status = Some(StatusPolicySnapshot {
            context,
            fsmonitor: None,
        });
        Ok(())
    }

    async fn read_status_filter_attributes_async(
        &self,
        paths: &[Vec<u8>],
        executable_drivers: &ExecutableFilterDrivers,
        neutralizer: &SealedFilterConfigOverride,
    ) -> io::Result<BTreeMap<Vec<u8>, String>> {
        if paths.is_empty() {
            return Ok(BTreeMap::new());
        }
        let mut input = tempfile::tempfile()?;
        write_nul_paths(&mut input, paths)?;
        input.rewind()?;
        let mut command =
            self.pending_status_command(FsmonitorOverride::Disabled, Some(neutralizer))?;
        command
            .disable_optional_locks()
            .args(["check-attr", "--stdin", "-z", "filter"])
            .stdin(Stdio::from(input));
        let output = command.output().await?;
        if !output.status.success() {
            return Err(command_failure("status filter attribute probe", &output));
        }
        let attributes = parse_filter_attributes(&output.stdout, paths)?;
        self.resolve_status_filter_sentinels_async(attributes, executable_drivers, neutralizer)
            .await
    }

    async fn resolve_status_filter_sentinels_async(
        &self,
        attributes: BTreeMap<Vec<u8>, FilterAttributeValue>,
        executable_drivers: &ExecutableFilterDrivers,
        neutralizer: &SealedFilterConfigOverride,
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
                        && self
                            .status_sentinel_selects_driver_async(
                                &path,
                                &driver,
                                neutralizer,
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

    async fn status_sentinel_selects_driver_async(
        &self,
        path: &[u8],
        driver: &str,
        neutralizer: &SealedFilterConfigOverride,
        probe_budget: &mut SentinelFilterProbeBudget,
    ) -> io::Result<bool> {
        if !matches!(driver, "set" | "unset" | "unspecified") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Git filter sentinel probe requested for a non-sentinel driver",
            ));
        }
        probe_budget.ensure_probe_available().map_err(|_| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                StatusFilterProbeLimitExceeded {
                    max_probes: SentinelFilterProbeBudget::max_probes(),
                },
            )
        })?;
        let mut command =
            self.pending_status_command(FsmonitorOverride::Disabled, Some(neutralizer))?;
        command
            .disable_optional_locks()
            .args(["ls-files", "--cached", "--full-name", "-z", "--"])
            .arg(status_filter_special_pathspec(path, driver)?);
        let output = command.output().await?;
        probe_budget.record_completed_probe();
        if !output.status.success() {
            return Err(command_failure("status filter sentinel probe", &output));
        }
        status_filter_sentinel_probe_selects_driver(&output.stdout, path)
    }

    async fn build_filter_override_async(
        &self,
        executable_drivers: &ExecutableFilterDrivers,
    ) -> io::Result<SealedFilterConfigOverride> {
        if executable_drivers.len() > MAX_EXECUTABLE_FILTER_DRIVERS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "too many executable filter drivers for status neutralization",
            ));
        }
        let config_dir = tempfile::tempdir()?;
        let config_path = config_dir.path().join("filter-neutralization.gitconfig");
        self.ensure_owned_config_path(&config_path, "owned Git filter neutralization")?;
        std::fs::write(&config_path, [])?;
        for driver in executable_drivers.iter() {
            for (name, value) in FILTER_NEUTRALIZATION_PLAN {
                self.write_filter_override_value_async(&config_path, driver, name, value)
                    .await?;
            }
        }
        let config_path = config_path.to_str().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "non-UTF-8 filter guard path")
        })?;
        Ok(SealedFilterConfigOverride {
            owner: Arc::clone(&self.identity),
            include_arg: format!("include.path={config_path}"),
            _config_dir: config_dir,
        })
    }

    async fn write_filter_override_value_async(
        &self,
        config_path: &Path,
        driver: &str,
        name: &str,
        value: &str,
    ) -> io::Result<()> {
        debug_assert!(matches!(name, "clean" | "smudge" | "process" | "required"));
        let mut command = self
            .sources
            .git
            .async_command_for_cwd(&self.sources.canonical_root)?;
        command
            .args(&self.sources.base_config_args)
            .args(["config", "--file"])
            .arg(config_path)
            .args(["--add", &format!("filter.{driver}.{name}"), value]);
        let output = self
            .sources
            .git
            .output_async_bounded(command, MAX_INTERNAL_GIT_OUTPUT_BYTES)
            .await?;
        if !output.status.success() {
            return Err(command_failure(
                "status filter neutralization write",
                &output,
            ));
        }
        Ok(())
    }
}

fn status_filter_special_pathspec(path: &[u8], driver: &str) -> io::Result<std::ffi::OsString> {
    if path.is_empty() || path.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid Status filter sentinel path",
        ));
    }
    let requirement = match driver {
        "set" => "filter",
        "unset" => "-filter",
        "unspecified" => "!filter",
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Git filter sentinel probe requested for a non-sentinel driver",
            ));
        }
    };
    let mut pathspec = format!(":(top,literal,attr:{requirement})").into_bytes();
    pathspec.extend_from_slice(path);
    if pathspec.len() > MAX_STATUS_SENTINEL_PATHSPEC_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Git filter sentinel pathspec exceeds the Status byte limit",
        ));
    }
    git_path_argument(&pathspec)
}

fn status_filter_sentinel_probe_selects_driver(output: &[u8], path: &[u8]) -> io::Result<bool> {
    let matches = parse_nul_paths(output)?;
    if matches.is_empty() {
        // The public check-attr spelling was ambiguous at T0. If the one
        // authoritative T1 pathspec does not prove the corresponding special
        // state, conservatively retain the spelling as a literal driver
        // selection. This preserves the structured selected-filter refusal
        // and also fails closed for any intervening source drift.
        return Ok(true);
    }
    if matches.len() > MAX_STATUS_SENTINEL_STAGE_RECORDS
        || matches.iter().any(|matched| matched != path)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "status filter sentinel probe returned non-exact index paths",
        ));
    }
    Ok(false)
}

#[cfg(test)]
thread_local! {
    static FILTER_POLICY_READ_COUNT_FOR_STATUS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(super) fn reset_status_filter_policy_read_count() {
    FILTER_POLICY_READ_COUNT_FOR_STATUS.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn status_filter_policy_read_count() -> usize {
    FILTER_POLICY_READ_COUNT_FOR_STATUS.with(std::cell::Cell::get)
}

#[cfg(test)]
#[path = "status_filter_policy_tests.rs"]
mod tests;
