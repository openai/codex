use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use crate::FsmonitorOverride;
use crate::git_command::GitCommand;
use crate::git_command::GitRunner;
use crate::git_config::GitConfigEntry;
use crate::git_config::read_effective_config_with_fallback;
use crate::git_config_sources::ensure_no_worktree_config_sources;
use crate::safe_git::DISABLED_HOOKS_PATH;
use crate::safe_git::ExecutableFilterDrivers;
use crate::safe_git::FilterExecution;
use crate::safe_git::FilterPolicyRole;
use crate::safe_git::FilterPolicySnapshot;
use crate::safe_git::build_filter_policy_snapshot;

mod merge_overlay;
use merge_overlay::SealedMergeConfigOverride;
#[cfg(test)]
pub(crate) use merge_overlay::merge_attribute_read_count;
#[cfg(test)]
pub(crate) use merge_overlay::merge_config_read_count;
#[cfg(test)]
pub(crate) use merge_overlay::merge_overlay_count;
#[cfg(test)]
pub(crate) use merge_overlay::reset_merge_policy_counts;
mod reverse_probe;

/// Proof that one exact Git config invocation has no worktree-controlled
/// source routes for one runner and canonical repository root.
///
/// The capability deliberately owns the ordered base arguments and cannot be
/// cloned or rebound to another runner, root, or command environment.
pub(crate) struct ValidatedConfigSources<'git> {
    git: &'git GitRunner,
    canonical_root: PathBuf,
    base_config_args: Box<[String]>,
}

impl<'git> ValidatedConfigSources<'git> {
    fn authorize(
        git: &'git GitRunner,
        canonical_root: &Path,
        base_config_args: Vec<String>,
    ) -> io::Result<Self> {
        #[cfg(test)]
        CONFIG_SOURCE_AUTHORIZATION_COUNT.with(|count| count.set(count.get() + 1));

        validate_base_config_args(&base_config_args)?;
        let canonical_root = std::fs::canonicalize(canonical_root)?;
        git.ensure_active_worktree_root(&canonical_root)?;
        ensure_no_worktree_config_sources(git, &canonical_root, &base_config_args)?;
        Ok(Self {
            git,
            canonical_root,
            base_config_args: base_config_args.into_boxed_slice(),
        })
    }

    fn read_effective(
        &self,
        pattern: &str,
        probe: &str,
    ) -> io::Result<BTreeMap<String, GitConfigEntry>> {
        read_effective_config_with_fallback(
            self.git,
            &self.canonical_root,
            &self.base_config_args,
            pattern,
            probe,
        )
    }

    fn read_bool(&self, key: &str) -> io::Result<Option<bool>> {
        let mut command = self.git.command_for_cwd(&self.canonical_root)?;
        command
            .env("GIT_OPTIONAL_LOCKS", "0")
            .args(&self.base_config_args)
            .args(["config", "--type=bool", "--get", key]);
        let output = self.git.output(command)?;
        if output.status.code() == Some(1) && output.stdout.is_empty() && output.stderr.is_empty() {
            return Ok(None);
        }
        if !output.status.success() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Git boolean config probe for {key:?} failed with status {}: {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            ));
        }
        match String::from_utf8_lossy(&output.stdout).trim() {
            "true" => Ok(Some(true)),
            "false" => Ok(Some(false)),
            value => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unexpected normalized Git boolean value {value:?} for {key:?}"),
            )),
        }
    }
}

fn validate_base_config_args(args: &[String]) -> io::Result<()> {
    let mut pairs = args.chunks_exact(2);
    for pair in &mut pairs {
        let Some((key, _value)) = pair[1].split_once('=') else {
            return Err(invalid_base_config_args());
        };
        if pair[0] != "-c" || key.is_empty() {
            return Err(invalid_base_config_args());
        }
    }
    if !pairs.remainder().is_empty() {
        return Err(invalid_base_config_args());
    }
    Ok(())
}

fn invalid_base_config_args() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "guarded Git base config must contain only ordered -c key=value pairs",
    )
}

/// Operation-owned Git configuration capability.
///
/// All operation children are rooted at the authorized repository, inherit
/// the exact frozen base invocation, receive fixed library safety scalars, and
/// retain any sealed filter override for the capability lifetime.
pub(crate) struct GuardedGitConfig<'git> {
    sources: ValidatedConfigSources<'git>,
    identity: Arc<CapabilityIdentity>,
    // Ordered, typed snapshots keep each sealed filter overlay alive through
    // every later child. Downstream staging may attach a fresh Git-add policy
    // without rebuilding or weakening the source authorization.
    filters: Vec<FilterPolicySnapshot>,
    // At most one merge-driver neutralizer may be attached. Command assembly
    // places it between the apply and Git-add filter slots regardless of
    // attachment timing.
    merge: Option<SealedMergeConfigOverride>,
    merge_policy_installed: bool,
}

struct CapabilityIdentity;

#[derive(Clone, Copy)]
enum BoundSubcommand {
    Apply,
    CheckAttr,
    CheckIgnore,
    HashObject,
    ListBuiltinCommands,
    LsFiles,
    RevParse,
    SparseCheckout,
    UpdateIndexLiteralPathspecs,
}

impl BoundSubcommand {
    fn append_to(self, command: &mut GitCommand) {
        match self {
            Self::Apply => {
                command.arg("apply");
            }
            Self::CheckAttr => {
                command.arg("check-attr");
            }
            Self::CheckIgnore => {
                command.arg("check-ignore");
            }
            Self::HashObject => {
                command.arg("hash-object");
            }
            Self::ListBuiltinCommands => {
                command.arg("--list-cmds=builtins");
            }
            Self::LsFiles => {
                command.arg("ls-files");
            }
            Self::RevParse => {
                command.arg("rev-parse");
            }
            Self::SparseCheckout => {
                command.arg("sparse-checkout");
            }
            Self::UpdateIndexLiteralPathspecs => {
                command.args(["--literal-pathspecs", "update-index"]);
            }
        }
    }
}

/// A sealed filter-only include owned for the complete capability lifetime.
/// Its fields and include argument are intentionally inaccessible outside this
/// module; callers can neither forge one nor append it to another command.
pub(crate) struct SealedFilterConfigOverride {
    owner: Arc<CapabilityIdentity>,
    include_arg: String,
    _config_dir: tempfile::TempDir,
}

impl SealedFilterConfigOverride {
    fn append_to(
        &self,
        owner: &Arc<CapabilityIdentity>,
        command: &mut GitCommand,
    ) -> io::Result<()> {
        self.ensure_owner(owner)?;
        command.args(["-c", &self.include_arg]);
        Ok(())
    }

    fn append_rendered_args(
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
                "sealed Git filter override belongs to another operation",
            ));
        }
        Ok(())
    }
}

/// A command whose runner, root, config invocation, overlay lifetime, and
/// fixed subcommand are inseparably bound to one operation capability.
pub(crate) struct GuardedGitCommand<'operation, 'git> {
    operation: &'operation GuardedGitConfig<'git>,
    inner: GitCommand,
}

impl GuardedGitCommand<'_, '_> {
    pub(crate) fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
        self.inner.arg(arg);
        self
    }

    pub(crate) fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.inner.args(args);
        self
    }

    pub(crate) fn disable_optional_locks(&mut self) -> &mut Self {
        self.inner.env("GIT_OPTIONAL_LOCKS", "0");
        self
    }

    pub(crate) fn stdin(&mut self, config: impl Into<Stdio>) -> &mut Self {
        self.inner.stdin(config);
        self
    }

    pub(crate) fn output(self) -> io::Result<std::process::Output> {
        self.operation.sources.git.output(self.inner)
    }
}

impl<'git> GuardedGitConfig<'git> {
    pub(crate) fn authorize(
        git: &'git GitRunner,
        canonical_root: &Path,
        base_config_args: Vec<String>,
    ) -> io::Result<Self> {
        Ok(Self {
            sources: ValidatedConfigSources::authorize(git, canonical_root, base_config_args)?,
            identity: Arc::new(CapabilityIdentity),
            filters: Vec::new(),
            merge: None,
            merge_policy_installed: false,
        })
    }

    pub(crate) fn canonical_root(&self) -> &Path {
        &self.sources.canonical_root
    }

    pub(crate) fn apply_command(&self) -> io::Result<GuardedGitCommand<'_, 'git>> {
        self.guarded_command(BoundSubcommand::Apply)
    }

    pub(crate) fn rev_parse_command(&self) -> io::Result<GuardedGitCommand<'_, 'git>> {
        self.guarded_command(BoundSubcommand::RevParse)
    }

    pub(crate) fn check_ignore_command(&self) -> io::Result<GuardedGitCommand<'_, 'git>> {
        self.guarded_command(BoundSubcommand::CheckIgnore)
    }

    pub(crate) fn list_builtin_commands(&self) -> io::Result<std::process::Output> {
        let mut command = self.guarded_command(BoundSubcommand::ListBuiltinCommands)?;
        command.disable_optional_locks();
        command.output()
    }

    pub(crate) fn ls_files_command(&self) -> io::Result<GuardedGitCommand<'_, 'git>> {
        self.guarded_command(BoundSubcommand::LsFiles)
    }

    pub(crate) fn sparse_checkout_command(&self) -> io::Result<GuardedGitCommand<'_, 'git>> {
        self.guarded_command(BoundSubcommand::SparseCheckout)
    }

    pub(crate) fn update_index_literal_pathspecs_command(
        &self,
    ) -> io::Result<GuardedGitCommand<'_, 'git>> {
        self.guarded_command(BoundSubcommand::UpdateIndexLiteralPathspecs)
    }

    pub(crate) fn pending_filter_attribute_command<'operation>(
        &'operation self,
        neutralizer: &'operation SealedFilterConfigOverride,
    ) -> io::Result<GuardedGitCommand<'operation, 'git>> {
        let mut command = self.command_with_attached_overlays()?;
        neutralizer.append_to(&self.identity, &mut command)?;
        BoundSubcommand::CheckAttr.append_to(&mut command);
        Ok(GuardedGitCommand {
            operation: self,
            inner: command,
        })
    }

    fn guarded_command(
        &self,
        subcommand: BoundSubcommand,
    ) -> io::Result<GuardedGitCommand<'_, 'git>> {
        let mut command = self.command_with_attached_overlays()?;
        subcommand.append_to(&mut command);
        Ok(GuardedGitCommand {
            operation: self,
            inner: command,
        })
    }

    fn command_with_attached_overlays(&self) -> io::Result<GitCommand> {
        let mut command = self
            .sources
            .git
            .command_for_cwd(&self.sources.canonical_root)?;
        command.args(&self.sources.base_config_args);
        append_safe_scalar_overrides(&mut command);
        let (apply, git_add) = self.ordered_filter_snapshots()?;
        if let Some(neutralizer) = apply.and_then(FilterPolicySnapshot::neutralizer) {
            neutralizer.append_to(&self.identity, &mut command)?;
        }
        if let Some(merge) = &self.merge {
            merge.append_to(&self.identity, &mut command)?;
        }
        if let Some(neutralizer) = git_add.and_then(FilterPolicySnapshot::neutralizer) {
            neutralizer.append_to(&self.identity, &mut command)?;
        }
        Ok(command)
    }

    fn ordered_filter_snapshots(
        &self,
    ) -> io::Result<(Option<&FilterPolicySnapshot>, Option<&FilterPolicySnapshot>)> {
        let ordered = match self.filters.as_slice() {
            [] => (None, None),
            [apply] if apply.role() == FilterPolicyRole::Apply => (Some(apply), None),
            [git_add] if git_add.role() == FilterPolicyRole::GitAdd => (None, Some(git_add)),
            [apply, git_add]
                if apply.role() == FilterPolicyRole::Apply
                    && git_add.role() == FilterPolicyRole::GitAdd =>
            {
                (Some(apply), Some(git_add))
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "invalid ordered Git filter policy state",
                ));
            }
        };
        if self.merge_policy_installed && ordered.0.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "merge policy requires an apply filter snapshot",
            ));
        }
        Ok(ordered)
    }

    pub(crate) fn command_for_sentinel_filter_probe<'operation>(
        &'operation self,
        neutralizer: &'operation SealedFilterConfigOverride,
        driver: &str,
        required: bool,
    ) -> io::Result<GuardedGitCommand<'operation, 'git>> {
        if !matches!(driver, "set" | "unset" | "unspecified") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Git filter sentinel probe requested for a non-sentinel driver",
            ));
        }
        let mut command = self.command_with_attached_overlays()?;
        neutralizer.append_to(&self.identity, &mut command)?;
        command.args(["-c", &format!("filter.{driver}.required={required}")]);
        BoundSubcommand::HashObject.append_to(&mut command);
        Ok(GuardedGitCommand {
            operation: self,
            inner: command,
        })
    }

    pub(crate) fn read_effective(
        &self,
        pattern: &str,
        probe: &str,
    ) -> io::Result<BTreeMap<String, GitConfigEntry>> {
        self.sources.read_effective(pattern, probe)
    }

    pub(crate) fn read_bool(&self, key: &str) -> io::Result<Option<bool>> {
        self.sources.read_bool(key)
    }

    fn ensure_owned_config_path(&self, path: &Path, description: &str) -> io::Result<()> {
        self.sources
            .git
            .ensure_config_source_is_not_worktree_controlled(path, description)
    }

    pub(crate) fn build_filter_override(
        &self,
        executable_drivers: &ExecutableFilterDrivers,
    ) -> io::Result<SealedFilterConfigOverride> {
        let config_dir = tempfile::tempdir()?;
        let config_path = config_dir.path().join("filter-neutralization.gitconfig");
        self.ensure_owned_config_path(&config_path, "owned Git filter neutralization")?;
        std::fs::write(&config_path, [])?;
        for driver in executable_drivers.iter() {
            for name in ["clean", "smudge", "process"] {
                self.write_filter_override_value(&config_path, driver, name, "")?;
            }
            self.write_filter_override_value(&config_path, driver, "required", "false")?;
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

    fn write_filter_override_value(
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
            .command_for_cwd(&self.sources.canonical_root)?;
        command
            .args(&self.sources.base_config_args)
            .args(["config", "--file"])
            .arg(config_path)
            .args(["--add", &format!("filter.{driver}.{name}"), value]);
        let output = self.sources.git.output(command)?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "failed to write Git filter neutralization for {driver:?} (status {}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Ok(())
    }

    pub(crate) fn authorize_filter_paths(&mut self, paths: &[String]) -> io::Result<()> {
        if !self.filters.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "apply filter policy must be the first operation snapshot",
            ));
        }
        let filter =
            build_filter_policy_snapshot(self, paths, FilterExecution::AnyWorktreeOperation)?;
        self.filters.push(filter);
        Ok(())
    }

    pub(crate) fn ensure_apply_filter_path_subset(&self, paths: &[String]) -> io::Result<()> {
        let [apply] = self.filters.as_slice() else {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "composed exact staging requires exactly one apply filter snapshot",
            ));
        };
        if apply.role() != FilterPolicyRole::Apply {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "composed exact staging requires an apply filter snapshot",
            ));
        }
        if let Some(path) = paths
            .iter()
            .find(|path| !apply.contains_checked_path(path.as_str()))
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "content-filter staging path {path:?} was not authorized by the apply snapshot"
                ),
            ));
        }
        Ok(())
    }

    pub(crate) fn apply_filter_paths(&self) -> io::Result<Vec<String>> {
        let [apply] = self.filters.as_slice() else {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "merge policy requires exactly one apply filter snapshot",
            ));
        };
        if apply.role() != FilterPolicyRole::Apply {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "merge policy requires an apply filter snapshot",
            ));
        }
        Ok(apply.checked_paths())
    }

    #[cfg(test)]
    pub(crate) fn merge_include_arg(&self) -> Option<&str> {
        self.merge
            .as_ref()
            .map(SealedMergeConfigOverride::include_arg)
    }

    pub(crate) fn authorize_git_add_filter_paths(&mut self, paths: &[String]) -> io::Result<()> {
        match self.filters.as_slice() {
            [] => {}
            [apply] if apply.role() == FilterPolicyRole::Apply => {}
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "Git-add filter policy requires zero snapshots or exactly one apply snapshot",
                ));
            }
        }
        let filter = build_filter_policy_snapshot(self, paths, FilterExecution::GitAdd)?;
        self.filters.push(filter);
        Ok(())
    }

    pub(crate) fn render_command_for_log(&self, args: &[String]) -> io::Result<String> {
        let mut parts = vec!["git".to_string()];
        parts.extend(self.sources.base_config_args.iter().cloned());
        parts.extend(safe_scalar_override_args());
        let (apply, git_add) = self.ordered_filter_snapshots()?;
        if let Some(neutralizer) = apply.and_then(FilterPolicySnapshot::neutralizer) {
            neutralizer.append_rendered_args(&self.identity, &mut parts)?;
        }
        if let Some(merge) = &self.merge {
            merge.append_rendered_args(&self.identity, &mut parts)?;
        }
        if let Some(neutralizer) = git_add.and_then(FilterPolicySnapshot::neutralizer) {
            neutralizer.append_rendered_args(&self.identity, &mut parts)?;
        }
        parts.extend(args.iter().cloned());
        Ok(format!(
            "(cd {} && {})",
            quote_shell(&self.sources.canonical_root.display().to_string()),
            parts
                .into_iter()
                .map(|part| quote_shell(&part))
                .collect::<Vec<_>>()
                .join(" ")
        ))
    }
}

fn append_safe_scalar_overrides(command: &mut GitCommand) {
    command.args(safe_scalar_override_args());
}

fn safe_scalar_override_args() -> [String; 4] {
    [
        "-c".to_string(),
        format!("core.hooksPath={DISABLED_HOOKS_PATH}"),
        "-c".to_string(),
        FsmonitorOverride::Disabled.git_config_arg().to_string(),
    ]
}

fn quote_shell(value: &str) -> String {
    let simple = value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "-_.:/@%+".contains(character));
    if simple {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
thread_local! {
    static CONFIG_SOURCE_AUTHORIZATION_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_config_source_authorization_count() {
    CONFIG_SOURCE_AUTHORIZATION_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn config_source_authorization_count() -> usize {
    CONFIG_SOURCE_AUTHORIZATION_COUNT.with(std::cell::Cell::get)
}

#[cfg(test)]
#[path = "guarded_config_tests.rs"]
mod tests;
