use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::io;
use std::io::Seek;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use crate::FsmonitorOverride;
use crate::FsmonitorProbeRunner;
use crate::git_command::GitAsyncCommand;
use crate::git_command::GitCommand;
use crate::git_command::GitRunner;
use crate::git_command::MAX_INTERNAL_GIT_OUTPUT_BYTES;
use crate::git_config::GitConfigEntry;
use crate::git_config::read_effective_config_with_fallback;
use crate::git_config::read_effective_config_with_fallback_async;
use crate::git_config_sources::ensure_no_worktree_config_sources;
use crate::git_config_sources::ensure_no_worktree_config_sources_async;
use crate::safe_git::DISABLED_HOOKS_PATH;
use crate::safe_git::ExecutableFilterDrivers;
use crate::safe_git::FilterAttributeValue;
use crate::safe_git::FilterExecution;
use crate::safe_git::FilterPolicyRole;
use crate::safe_git::FilterPolicySnapshot;
use crate::safe_git::MAX_EXECUTABLE_FILTER_DRIVERS;
use crate::safe_git::SelectedFilterPolicy;
use crate::safe_git::SentinelFilterProbeBudget;
use crate::safe_git::SentinelFilterProbeResolution;
use crate::safe_git::build_filter_policy_snapshot;
use crate::safe_git::classify_selected_filter;
use crate::safe_git::classify_sentinel_filter_probes;
use crate::safe_git::executable_filter_drivers;
use crate::safe_git::git_path_argument;
use crate::safe_git::parse_filter_attributes;
use crate::safe_git::parse_nul_paths;
use crate::safe_git::validate_executable_driver_count;
use crate::safe_git::write_nul_paths;

const MAX_STATUS_TRACKED_PATHS: usize = 250_000;
const FILTER_NEUTRALIZATION_PLAN: [(&str, &str); 4] = [
    ("clean", ""),
    ("smudge", ""),
    ("process", ""),
    ("required", "false"),
];

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

    async fn authorize_async(
        git: &'git GitRunner,
        canonical_root: &Path,
        base_config_args: Vec<String>,
    ) -> io::Result<Self> {
        #[cfg(test)]
        CONFIG_SOURCE_AUTHORIZATION_COUNT.with(|count| count.set(count.get() + 1));

        validate_base_config_args(&base_config_args)?;
        let canonical_root = std::fs::canonicalize(canonical_root)?;
        git.ensure_active_worktree_root(&canonical_root)?;
        ensure_no_worktree_config_sources_async(git, &canonical_root, &base_config_args).await?;
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
        parse_bool_output(&output, key)
    }

    async fn read_effective_async(
        &self,
        pattern: &str,
        probe: &str,
    ) -> io::Result<BTreeMap<String, GitConfigEntry>> {
        read_effective_config_with_fallback_async(
            self.git,
            &self.canonical_root,
            &self.base_config_args,
            pattern,
            probe,
        )
        .await
    }

    async fn read_bool_async(&self, key: &str) -> io::Result<Option<bool>> {
        let mut command = self.git.async_command_for_cwd(&self.canonical_root)?;
        command
            .env("GIT_OPTIONAL_LOCKS", "0")
            .args(&self.base_config_args)
            .args(["config", "--type=bool", "--get", key]);
        let output = self
            .git
            .output_async_bounded(command, MAX_INTERNAL_GIT_OUTPUT_BYTES)
            .await?;
        parse_bool_output(&output, key)
    }
}

fn parse_bool_output(output: &std::process::Output, key: &str) -> io::Result<Option<bool>> {
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
    // Status is a mutually exclusive operation state rather than another
    // mutation overlay. `Some` with no neutralizer records the zero-driver
    // case and is still required before the final status sink can run.
    status: Option<StatusFilterPolicySnapshot>,
}

struct StatusFilterPolicySnapshot {
    neutralizer: Option<SealedFilterConfigOverride>,
    fsmonitor: Option<FsmonitorOverride>,
}

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
#[error("Git resolved {reported:?} instead of expected root {expected:?}")]
pub(crate) struct StatusRootMismatch {
    expected: PathBuf,
    reported: PathBuf,
}

#[derive(Debug, thiserror::Error)]
#[error("selected Git authority has no active worktree root")]
pub(crate) struct NoActiveStatusWorktree;

#[derive(Debug, thiserror::Error)]
#[error("Git filter attribute selection exceeded its {max_probes}-probe limit")]
pub(crate) struct StatusFilterProbeLimitExceeded {
    max_probes: usize,
}

#[derive(Debug, thiserror::Error)]
#[error("{description} failed with exit code {exit_code:?}")]
pub(crate) struct StatusPolicyCommandFailure {
    description: String,
    exit_code: Option<i32>,
}

impl StatusPolicyCommandFailure {
    pub(crate) fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }
}

impl StatusFilterProbeLimitExceeded {
    pub(crate) fn max_probes(&self) -> usize {
        self.max_probes
    }
}

impl StatusRootMismatch {
    pub(crate) fn expected(&self) -> &Path {
        &self.expected
    }

    pub(crate) fn reported(&self) -> &Path {
        &self.reported
    }
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

/// Async counterpart to [`GuardedGitCommand`]. The raw Tokio command never
/// leaves this module, so status callers cannot change its cwd, config
/// invocation, overlay order, or final metadata revalidation.
struct GuardedAsyncGitCommand<'operation, 'git> {
    operation: &'operation GuardedGitConfig<'git>,
    inner: GitAsyncCommand,
}

impl GuardedAsyncGitCommand<'_, '_> {
    fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
        self.inner.arg(arg);
        self
    }

    fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.inner.args(args);
        self
    }

    fn disable_optional_locks(&mut self) -> &mut Self {
        self.inner.env("GIT_OPTIONAL_LOCKS", "0");
        self
    }

    fn stdin(&mut self, config: impl Into<Stdio>) -> &mut Self {
        self.inner.stdin(config);
        self
    }

    async fn output(self) -> io::Result<std::process::Output> {
        self.operation
            .sources
            .git
            .output_async_bounded(self.inner, MAX_INTERNAL_GIT_OUTPUT_BYTES)
            .await
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
            status: None,
        })
    }

    pub(crate) async fn authorize_status_async(git: &'git GitRunner) -> io::Result<Self> {
        let root = git
            .active_worktree_root()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, NoActiveStatusWorktree))?;
        git.ensure_repository_root_route(root)?;
        Ok(Self {
            sources: ValidatedConfigSources::authorize_async(git, root, Vec::new()).await?,
            identity: Arc::new(CapabilityIdentity),
            filters: Vec::new(),
            merge: None,
            merge_policy_installed: false,
            status: None,
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
        command.args(self.mutation_config_args()?);
        Ok(command)
    }

    fn mutation_config_args(&self) -> io::Result<Vec<String>> {
        if self.status.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "status policy cannot be used by a mutation command",
            ));
        }
        let mut args = self.sources.base_config_args.to_vec();
        args.extend(safe_scalar_override_args());
        let (apply, git_add) = self.ordered_filter_snapshots()?;
        if let Some(neutralizer) = apply.and_then(FilterPolicySnapshot::neutralizer) {
            neutralizer.append_rendered_args(&self.identity, &mut args)?;
        }
        if let Some(merge) = &self.merge {
            merge.append_rendered_args(&self.identity, &mut args)?;
        }
        if let Some(neutralizer) = git_add.and_then(FilterPolicySnapshot::neutralizer) {
            neutralizer.append_rendered_args(&self.identity, &mut args)?;
        }
        Ok(args)
    }

    fn status_config_args(
        &self,
        fsmonitor: FsmonitorOverride,
        neutralizer: Option<&SealedFilterConfigOverride>,
    ) -> io::Result<Vec<String>> {
        self.ensure_status_exclusive_state()?;
        let mut args = self.sources.base_config_args.to_vec();
        args.extend([
            "-c".to_string(),
            format!("core.hooksPath={DISABLED_HOOKS_PATH}"),
            "-c".to_string(),
            fsmonitor.git_config_arg().to_string(),
        ]);
        if let Some(neutralizer) = neutralizer {
            neutralizer.append_rendered_args(&self.identity, &mut args)?;
        }
        Ok(args)
    }

    fn ensure_status_exclusive_state(&self) -> io::Result<()> {
        if !self.filters.is_empty() || self.merge.is_some() || self.merge_policy_installed {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "status policy cannot coexist with mutation filter or merge policy",
            ));
        }
        Ok(())
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

    /// Confirm selected Git agrees with the retained authority about the
    /// repository root before any status worktree read.
    pub(crate) async fn verify_status_root_async(&self, requested_cwd: &Path) -> io::Result<()> {
        let requested_cwd = std::fs::canonicalize(requested_cwd)?;
        let mut command = self.sources.git.async_command_for_cwd(&requested_cwd)?;
        command
            .env("GIT_OPTIONAL_LOCKS", "0")
            .args(self.status_config_args(FsmonitorOverride::Disabled, /*neutralizer*/ None)?)
            .args(["rev-parse", "--show-toplevel"]);
        let output = self
            .sources
            .git
            .output_async_bounded(command, MAX_INTERNAL_GIT_OUTPUT_BYTES)
            .await?;
        if !output.status.success() {
            return Err(command_failure("status repository-root probe", &output));
        }
        let reported = git_path_from_line_output(&output.stdout)?;
        let reported = std::fs::canonicalize(reported).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Git repository-root output did not resolve to an existing path",
            )
        })?;
        if reported != self.sources.canonical_root {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                StatusRootMismatch {
                    expected: self.sources.canonical_root.clone(),
                    reported,
                },
            ));
        }
        Ok(())
    }

    /// Return the exact tracked-path byte strings for the Status policy.
    async fn read_status_tracked_paths_async(&self) -> io::Result<Vec<Vec<u8>>> {
        if self.status.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "tracked paths must be read before status policy installation",
            ));
        }
        let mut command =
            self.pending_status_command(FsmonitorOverride::Disabled, /*neutralizer*/ None)?;
        command
            .disable_optional_locks()
            .args(["ls-files", "-z", "--cached"]);
        let output = command.output().await?;
        if !output.status.success() {
            return Err(command_failure("tracked-path probe", &output));
        }
        let paths = parse_nul_paths(&output.stdout)?;
        if paths.len() > MAX_STATUS_TRACKED_PATHS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "tracked path count {} exceeds the status limit {MAX_STATUS_TRACKED_PATHS}",
                    paths.len()
                ),
            ));
        }
        Ok(paths)
    }

    /// Install the mutually exclusive Status filter policy. The common
    /// zero-driver path occupies the slot without scanning tracked paths.
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
        let executable_drivers = executable_filter_drivers(&entries)?;
        validate_executable_driver_count(&executable_drivers)?;
        if executable_drivers.is_empty() {
            self.status = Some(StatusFilterPolicySnapshot {
                neutralizer: None,
                fsmonitor: None,
            });
            return Ok(());
        }

        let paths = self.read_status_tracked_paths_async().await?;
        let neutralizer = self
            .build_filter_override_async(&executable_drivers)
            .await?;
        let attributes = self
            .read_status_filter_attributes_async(&paths, &executable_drivers, &neutralizer)
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
        self.status = Some(StatusFilterPolicySnapshot {
            neutralizer: Some(neutralizer),
            fsmonitor: None,
        });
        Ok(())
    }

    pub(crate) async fn detect_status_fsmonitor_async(&mut self) -> FsmonitorOverride {
        if self.status.is_none() || self.ensure_status_exclusive_state().is_err() {
            return FsmonitorOverride::Disabled;
        }
        if let Some(fsmonitor) = self.status.as_ref().and_then(|status| status.fsmonitor) {
            return fsmonitor;
        }
        let fsmonitor = {
            let mut runner = StatusFsmonitorProbeRunner { config: self };
            crate::detect_fsmonitor_override(&mut runner).await
        };
        if let Some(status) = &mut self.status {
            status.fsmonitor = Some(fsmonitor);
        }
        fsmonitor
    }

    pub(crate) async fn status_output_async(&self) -> io::Result<std::process::Output> {
        let status = self.status.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "status output requires an installed status filter policy",
            )
        })?;
        let fsmonitor = status.fsmonitor.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "status output requires a retained fsmonitor decision",
            )
        })?;
        let mut command = self.pending_status_command(fsmonitor, status.neutralizer.as_ref())?;
        command.disable_optional_locks().args([
            "status",
            "--porcelain",
            "--ignore-submodules=dirty",
        ]);
        command.output().await
    }

    fn pending_status_command<'operation>(
        &'operation self,
        fsmonitor: FsmonitorOverride,
        neutralizer: Option<&'operation SealedFilterConfigOverride>,
    ) -> io::Result<GuardedAsyncGitCommand<'operation, 'git>> {
        let mut command = self
            .sources
            .git
            .async_command_for_cwd(&self.sources.canonical_root)?;
        command.args(self.status_config_args(fsmonitor, neutralizer)?);
        Ok(GuardedAsyncGitCommand {
            operation: self,
            inner: command,
        })
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
        let required = self
            .run_status_sentinel_probe_async(
                path,
                driver,
                /*required*/ true,
                neutralizer,
                probe_budget,
            )
            .await?;
        if classify_sentinel_filter_probes(
            required.status.success(),
            /*optional_succeeded*/ None,
        ) == SentinelFilterProbeResolution::SpecialAttributeState
        {
            return Ok(false);
        }
        let optional = self
            .run_status_sentinel_probe_async(
                path,
                driver,
                /*required*/ false,
                neutralizer,
                probe_budget,
            )
            .await?;
        if classify_sentinel_filter_probes(
            required.status.success(),
            Some(optional.status.success()),
        ) == SentinelFilterProbeResolution::LiteralDriver
        {
            return Ok(true);
        }
        Err(command_failure("status filter sentinel probe", &optional))
    }

    async fn run_status_sentinel_probe_async(
        &self,
        path: &[u8],
        driver: &str,
        required: bool,
        neutralizer: &SealedFilterConfigOverride,
        probe_budget: &mut SentinelFilterProbeBudget,
    ) -> io::Result<std::process::Output> {
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
            .args(["-c", &format!("filter.{driver}.required={required}")])
            .args(["hash-object", "--stdin"])
            .arg("--path")
            .arg(git_path_argument(path)?)
            .stdin(Stdio::null());
        let output = command.output().await?;
        probe_budget.record_completed_probe();
        Ok(output)
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
            for (name, value) in FILTER_NEUTRALIZATION_PLAN {
                self.write_filter_override_value(&config_path, driver, name, value)?;
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

    pub(crate) fn authorize_filter_paths(&mut self, paths: &[String]) -> io::Result<()> {
        if self.status.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "apply filter policy cannot coexist with status policy",
            ));
        }
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
        if self.status.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Git-add filter policy cannot coexist with status policy",
            ));
        }
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
        parts.extend(self.mutation_config_args()?);
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

struct StatusFsmonitorProbeRunner<'a, 'git> {
    config: &'a GuardedGitConfig<'git>,
}

impl FsmonitorProbeRunner for StatusFsmonitorProbeRunner<'_, '_> {
    async fn run_probe(&mut self, args: &[&str]) -> Option<Vec<u8>> {
        let allowed = matches!(
            args,
            ["config", "--null", "--get", "core.fsmonitor"]
                | [
                    "config",
                    "--null",
                    "--type=bool",
                    "--fixed-value",
                    "--get",
                    "core.fsmonitor",
                    _,
                ]
                | ["version", "--build-options"]
        );
        if !allowed {
            return None;
        }
        let mut command = self
            .config
            .sources
            .git
            .async_command_for_cwd(&self.config.sources.canonical_root)
            .ok()?;
        command
            .env("GIT_OPTIONAL_LOCKS", "0")
            .args(&self.config.sources.base_config_args)
            .args(args);
        match self
            .config
            .sources
            .git
            .output_async_bounded(command, MAX_INTERNAL_GIT_OUTPUT_BYTES)
            .await
        {
            Ok(output) if output.status.success() => Some(output.stdout),
            Ok(_) | Err(_) => None,
        }
    }
}

fn command_failure(description: &str, output: &std::process::Output) -> io::Error {
    io::Error::other(StatusPolicyCommandFailure {
        description: description.to_string(),
        exit_code: output.status.code(),
    })
}

fn git_path_from_line_output(output: &[u8]) -> io::Result<PathBuf> {
    let output = output.strip_suffix(b"\n").unwrap_or(output);
    #[cfg(windows)]
    let output = output.strip_suffix(b"\r").unwrap_or(output);
    if output.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "empty Git repository-root output",
        ));
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
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "non-UTF-8 Git repository-root output",
                )
            })
    }
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
    static FILTER_POLICY_READ_COUNT_FOR_STATUS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_config_source_authorization_count() {
    CONFIG_SOURCE_AUTHORIZATION_COUNT.with(|count| count.set(0));
    FILTER_POLICY_READ_COUNT_FOR_STATUS.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn config_source_authorization_count() -> usize {
    CONFIG_SOURCE_AUTHORIZATION_COUNT.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(crate) fn status_filter_policy_read_count() -> usize {
    FILTER_POLICY_READ_COUNT_FOR_STATUS.with(std::cell::Cell::get)
}

#[cfg(test)]
#[path = "guarded_config_tests.rs"]
mod tests;
