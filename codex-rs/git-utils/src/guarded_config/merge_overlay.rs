use std::collections::BTreeMap;
use std::io;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;

use super::BoundSubcommand;
use super::CapabilityIdentity;
use super::GuardedGitConfig;
use crate::git_config::GitConfigEntry;

const MERGE_CONFIG_PATTERN: &str = r"^(merge\.default|merge\..*\.driver)$";

/// A complete, fresh merge-config read bound to one authorized operation.
///
/// The fields are private so callers cannot mint a partial driver inventory
/// and then ask the capability to treat its neutralizer as complete.
struct MergeConfigSnapshot {
    owner: Arc<CapabilityIdentity>,
    entries: BTreeMap<String, GitConfigEntry>,
}

impl MergeConfigSnapshot {
    fn entries(&self) -> &BTreeMap<String, GitConfigEntry> {
        &self.entries
    }

    fn ensure_owner(&self, owner: &Arc<CapabilityIdentity>) -> io::Result<()> {
        if !Arc::ptr_eq(&self.owner, owner) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "merge config snapshot belongs to another operation",
            ));
        }
        Ok(())
    }

    fn nonempty_driver_keys(&self) -> io::Result<Vec<&str>> {
        let mut keys = Vec::new();
        for entry in self.entries.values() {
            if entry.key == "merge.default" {
                continue;
            }
            let _driver = entry
                .key
                .strip_prefix("merge.")
                .and_then(|key| key.strip_suffix(".driver"))
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "malformed effective Git merge-driver key",
                    )
                })?;
            if !entry.value.is_empty() {
                keys.push(entry.key.as_str());
            }
        }
        Ok(keys)
    }
}

/// A sealed merge-driver-only include bound to one operation.
///
/// Construction accepts only a complete `MergeConfigSnapshot`, and the type
/// exposes neither raw keys nor its include argument outside this module.
pub(super) struct SealedMergeConfigOverride {
    owner: Arc<CapabilityIdentity>,
    include_arg: String,
    _config_dir: tempfile::TempDir,
}

impl SealedMergeConfigOverride {
    pub(super) fn append_rendered_args(
        &self,
        owner: &Arc<CapabilityIdentity>,
        args: &mut Vec<String>,
    ) -> io::Result<()> {
        self.ensure_owner(owner)?;
        args.push("-c".to_string());
        args.push(self.include_arg.clone());
        Ok(())
    }

    fn ensure_owner(&self, owner: &Arc<CapabilityIdentity>) -> io::Result<()> {
        if !Arc::ptr_eq(&self.owner, owner) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "sealed Git merge override belongs to another operation",
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn include_arg(&self) -> &str {
        &self.include_arg
    }
}

impl<'git> GuardedGitConfig<'git> {
    /// Install the complete fallback merge policy in one non-bypassable step.
    /// This is the only crate-visible merge-overlay API: callers cannot mark
    /// the policy complete without the fixed fresh config and attribute reads.
    pub(crate) fn install_three_way_merge_policy(&mut self) -> io::Result<()> {
        let paths = self.apply_filter_paths()?;
        let snapshot = self.read_merge_config_snapshot()?;
        let input = merge_attribute_input(&paths)?;
        let output = self.query_merge_attributes(&snapshot, input)?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "git merge attribute probe failed with status {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        let attributes = crate::merge_driver::parse_merge_attributes(&output.stdout, &paths)?;
        if let Some((driver, path)) =
            crate::merge_driver::untrusted_driver_selection(snapshot.entries(), &attributes)?
        {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!(
                    "refusing to run an internal Git three-way apply with merge driver {driver:?} selected for {path:?}"
                ),
            ));
        }
        let neutralizer = self.build_merge_override(&snapshot)?;
        self.attach_merge_override(neutralizer)
    }

    /// Read merge-driver policy from the frozen, authorized base invocation.
    /// Attached neutralizers are deliberately excluded so this is a fresh
    /// view of the user's effective policy at fallback time.
    fn read_merge_config_snapshot(&self) -> io::Result<MergeConfigSnapshot> {
        let _ = self.apply_filter_paths()?;
        if self.merge_policy_installed {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "merge policy may be read only before its neutralizer is attached",
            ));
        }
        #[cfg(test)]
        MERGE_CONFIG_READ_COUNT.with(|count| count.set(count.get() + 1));
        Ok(MergeConfigSnapshot {
            owner: Arc::clone(&self.identity),
            entries: self.sources.read_effective(MERGE_CONFIG_PATTERN, "merge")?,
        })
    }

    /// Run the one fixed fresh merge-attribute query while the merge
    /// neutralizer is still absent. Existing apply-filter policy remains
    /// attached, and callers cannot change the framing or attribute name.
    fn query_merge_attributes(
        &self,
        snapshot: &MergeConfigSnapshot,
        input: std::fs::File,
    ) -> io::Result<std::process::Output> {
        let _ = self.apply_filter_paths()?;
        snapshot.ensure_owner(&self.identity)?;
        if self.merge_policy_installed {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "merge attributes must be read before attaching the merge neutralizer",
            ));
        }
        #[cfg(test)]
        MERGE_ATTRIBUTE_READ_COUNT.with(|count| count.set(count.get() + 1));
        let mut command = self.command_with_attached_overlays()?;
        BoundSubcommand::CheckAttr.append_to(&mut command);
        command
            .env("GIT_OPTIONAL_LOCKS", "0")
            .args(["--stdin", "-z", "merge"])
            .stdin(Stdio::from(input));
        self.sources.git.output(command)
    }

    fn build_merge_override(
        &self,
        snapshot: &MergeConfigSnapshot,
    ) -> io::Result<Option<SealedMergeConfigOverride>> {
        snapshot.ensure_owner(&self.identity)?;
        if self.merge_policy_installed {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "a merge neutralizer is already attached",
            ));
        }
        let driver_keys = snapshot.nonempty_driver_keys()?;
        if driver_keys.is_empty() {
            return Ok(None);
        }

        let config_dir = tempfile::tempdir()?;
        let config_path = config_dir
            .path()
            .join("merge-driver-neutralization.gitconfig");
        self.ensure_owned_config_path(&config_path, "owned Git merge-driver neutralization")?;
        std::fs::write(&config_path, [])?;
        for key in driver_keys {
            self.write_merge_override_value(&config_path, key)?;
        }
        let config_path = config_path.to_str().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "non-UTF-8 merge guard path")
        })?;
        #[cfg(test)]
        MERGE_OVERLAY_COUNT.with(|count| count.set(count.get() + 1));
        Ok(Some(SealedMergeConfigOverride {
            owner: Arc::clone(&self.identity),
            include_arg: format!("include.path={config_path}"),
            _config_dir: config_dir,
        }))
    }

    fn attach_merge_override(
        &mut self,
        neutralizer: Option<SealedMergeConfigOverride>,
    ) -> io::Result<()> {
        if self.merge_policy_installed {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "a second merge neutralizer is not permitted",
            ));
        }
        let [apply] = self.filters.as_slice() else {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "merge policy requires exactly one apply filter snapshot",
            ));
        };
        if apply.role() != crate::safe_git::FilterPolicyRole::Apply {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "merge policy requires an apply filter snapshot",
            ));
        }
        if let Some(neutralizer) = &neutralizer {
            neutralizer.ensure_owner(&self.identity)?;
        }
        self.merge = neutralizer;
        self.merge_policy_installed = true;
        Ok(())
    }

    fn write_merge_override_value(&self, config_path: &Path, key: &str) -> io::Result<()> {
        let mut command = self
            .sources
            .git
            .command_for_cwd(&self.sources.canonical_root)?;
        command
            .args(&self.sources.base_config_args)
            .args(["config", "--file"])
            .arg(config_path)
            .args(["--add", key, ""]);
        let output = self.sources.git.output(command)?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "failed to write Git merge-driver neutralization for {key:?} (status {}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Ok(())
    }
}

fn merge_attribute_input(paths: &[String]) -> io::Result<std::fs::File> {
    use std::io::Seek;
    use std::io::Write;

    if paths.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "refusing to inspect merge attributes for an empty patch path set",
        ));
    }
    let mut input = tempfile::tempfile()?;
    for path in paths {
        if path.as_bytes().contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "merge attribute path contains NUL",
            ));
        }
        input.write_all(path.as_bytes())?;
        input.write_all(&[0])?;
    }
    input.rewind()?;
    Ok(input)
}

#[cfg(test)]
thread_local! {
    static MERGE_CONFIG_READ_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static MERGE_ATTRIBUTE_READ_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static MERGE_OVERLAY_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_merge_policy_counts() {
    MERGE_CONFIG_READ_COUNT.with(|count| count.set(0));
    MERGE_ATTRIBUTE_READ_COUNT.with(|count| count.set(0));
    MERGE_OVERLAY_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn merge_config_read_count() -> usize {
    MERGE_CONFIG_READ_COUNT.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(crate) fn merge_attribute_read_count() -> usize {
    MERGE_ATTRIBUTE_READ_COUNT.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(crate) fn merge_overlay_count() -> usize {
    MERGE_OVERLAY_COUNT.with(std::cell::Cell::get)
}

#[cfg(test)]
#[path = "merge_overlay_tests.rs"]
mod tests;
