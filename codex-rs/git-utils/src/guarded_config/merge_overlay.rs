use std::collections::BTreeMap;
use std::io;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;

use super::BoundSubcommand;
use super::CapabilityIdentity;
use super::GuardedGitConfig;
use crate::git_command::IsolatedGitCommonDir;
use crate::git_config::GitConfigEntry;

const MERGE_CONFIG_PATTERN: &str = r"^(merge\.default|merge\..*\.driver)$";
const SANITIZED_CONFIG_PATTERN: &str = r"^(core\.(repositoryformatversion|filemode|symlinks|ignorecase|precomposeunicode|protecthfs|protectntfs|trustctime|checkstat|longpaths|fscache|splitindex|sparsecheckout|sparsecheckoutcone|autocrlf|eol|safecrlf|checkroundtripencoding|bigfilethreshold|whitespace)|extensions\.(objectformat|compatobjectformat)|index\.(sparse|version)|apply\.(whitespace|ignorewhitespace)|merge\.conflictstyle)$";

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

    fn sanitized_default_driver(&self) -> &str {
        match self
            .entries
            .get("merge.default")
            .map(|entry| entry.value.as_str())
        {
            Some("binary") => "binary",
            Some("union") => "union",
            _ => "text",
        }
    }
}

/// A sealed, helper-free common repository view bound to one operation.
///
/// The real Git directory, index, worktree, and object store remain selected;
/// only common config and attributes are replaced for the final three-way
/// child. Construction accepts a complete merge snapshot so a caller cannot
/// attach an unreviewed or partial view.
pub(super) struct SealedMergeConfigOverride {
    owner: Arc<CapabilityIdentity>,
    common_dir: IsolatedGitCommonDir,
}

impl SealedMergeConfigOverride {
    pub(super) fn common_dir(
        &self,
        owner: &Arc<CapabilityIdentity>,
    ) -> io::Result<&IsolatedGitCommonDir> {
        self.ensure_owner(owner)?;
        Ok(&self.common_dir)
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
        let isolated = self.build_merge_override(&snapshot)?;
        self.attach_merge_override(isolated)
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
    ) -> io::Result<SealedMergeConfigOverride> {
        snapshot.ensure_owner(&self.identity)?;
        if self.merge_policy_installed {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "an isolated three-way config is already attached",
            ));
        }

        let common_dir = self.sources.git.create_isolated_common_dir()?;
        let config_path = common_dir.config_path();
        self.ensure_owned_config_path(&config_path, "owned isolated Git common config")?;
        let entries = self
            .sources
            .read_effective(SANITIZED_CONFIG_PATTERN, "three-way allowlist")?;
        for entry in entries.values() {
            self.write_sanitized_config_value(&config_path, &entry.key, &entry.value)?;
        }
        self.write_sanitized_config_value(&config_path, "core.bare", "false")?;
        self.write_sanitized_config_value(
            &config_path,
            "merge.default",
            snapshot.sanitized_default_driver(),
        )?;
        #[cfg(test)]
        MERGE_OVERLAY_COUNT.with(|count| count.set(count.get() + 1));
        Ok(SealedMergeConfigOverride {
            owner: Arc::clone(&self.identity),
            common_dir,
        })
    }

    fn attach_merge_override(&mut self, isolated: SealedMergeConfigOverride) -> io::Result<()> {
        if self.merge_policy_installed {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "a second isolated three-way config is not permitted",
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
        isolated.ensure_owner(&self.identity)?;
        self.merge = Some(isolated);
        self.merge_policy_installed = true;
        Ok(())
    }

    fn write_sanitized_config_value(
        &self,
        config_path: &Path,
        key: &str,
        value: &str,
    ) -> io::Result<()> {
        let mut command = self
            .sources
            .git
            .command_for_cwd(&self.sources.canonical_root)?;
        command
            .args(["config", "--file"])
            .arg(config_path)
            .args(["--add", key, value]);
        let output = self.sources.git.output(command)?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "failed to write isolated Git config value {key:?} (status {}): {}",
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
