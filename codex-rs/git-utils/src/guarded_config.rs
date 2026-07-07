use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::io;
use std::io::Seek;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use crate::FsmonitorOverride;
use crate::git_command::GitAsyncCommand;
use crate::git_command::GitCommand;
use crate::git_command::GitRunner;
use crate::git_command::IsolatedGitStorage;
use crate::git_command::MAX_INTERNAL_GIT_OUTPUT_BYTES;
use crate::git_config::GitConfigEntry;
use crate::git_config::GitConfigOrigin;
use crate::git_config::GitConfigValue;
use crate::git_config::MergeConfigRecord;
use crate::git_config::parse_config_entries;
use crate::git_config::parse_config_entries_with_origins;
use crate::git_config::read_effective_config_with_fallback;
use crate::git_config::read_effective_config_with_fallback_async;
use crate::git_config::read_effective_shared_repository_with_fallback;
use crate::git_config::read_merge_config_records_with_fallback;
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
use crate::safe_git::build_filter_policy_snapshot;
use crate::safe_git::classify_selected_filter;
use crate::safe_git::git_path_argument;
use crate::safe_git::parse_filter_attributes;
use crate::safe_git::parse_nul_paths;
use crate::safe_git::status_policy_filter_drivers;
use crate::safe_git::validate_executable_driver_count;
use crate::safe_git::write_nul_paths;

const MAX_STATUS_TRACKED_PATHS: usize = 250_000;
const MAX_STATUS_SENTINEL_PATHSPEC_BYTES: usize = 16 * 1024;
const MAX_STATUS_SENTINEL_STAGE_RECORDS: usize = 3;
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
mod status_context;
use status_context::SealedStatusReadContext;
mod status_index;
use status_index::status_core_symlinks_for_filter_screening;

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

    fn read_shared_repository(&self) -> io::Result<Option<GitConfigValue>> {
        read_effective_shared_repository_with_fallback(
            self.git,
            &self.canonical_root,
            &self.base_config_args,
        )
    }

    fn read_merge_config(&self) -> io::Result<Vec<MergeConfigRecord>> {
        read_merge_config_records_with_fallback(
            self.git,
            &self.canonical_root,
            &self.base_config_args,
        )
    }

    fn read_direct_common_config(
        &self,
        pattern: &str,
        probe: &str,
    ) -> io::Result<BTreeMap<String, GitConfigEntry>> {
        let (scoped, scoped_path) = self
            .git
            .read_active_common_config_without_includes(pattern, /*show_scope*/ true)?;
        let entries = if scoped
            .status
            .code()
            .is_some_and(|code| code == 0 || code == 1)
        {
            parse_config_entries(&scoped.stdout)?
        } else {
            let (legacy, legacy_path) = self
                .git
                .read_active_common_config_without_includes(pattern, /*show_scope*/ false)?;
            if legacy_path != scoped_path {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "active Git common config changed during format fallback",
                ));
            }
            if !legacy
                .status
                .code()
                .is_some_and(|code| code == 0 || code == 1)
            {
                return Err(io::Error::other(format!(
                    "git {probe} direct common config probe failed with status {}: {}",
                    legacy.status,
                    String::from_utf8_lossy(&legacy.stderr).trim()
                )));
            }
            parse_config_entries_with_origins(&legacy.stdout)?
        };
        for entry in &entries {
            let expected_origin = match &entry.origin {
                GitConfigOrigin::File(origin) => {
                    same_file::is_same_file(origin, &scoped_path).unwrap_or(false)
                }
                GitConfigOrigin::CommandLine => false,
            };
            if !expected_origin {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("git {probe} direct common config probe returned an unexpected origin"),
                ));
            }
        }
        Ok(entries
            .into_iter()
            .map(|entry| (entry.key.clone(), entry))
            .collect())
    }

    async fn read_direct_common_config_async(
        &self,
        pattern: &str,
        probe: &str,
    ) -> io::Result<BTreeMap<String, GitConfigEntry>> {
        let (scoped, scoped_path) = self
            .git
            .read_active_common_config_without_includes_async(pattern, /*show_scope*/ true)
            .await?;
        let entries = if scoped
            .status
            .code()
            .is_some_and(|code| code == 0 || code == 1)
        {
            parse_config_entries(&scoped.stdout)?
        } else {
            let (legacy, legacy_path) = self
                .git
                .read_active_common_config_without_includes_async(
                    pattern, /*show_scope*/ false,
                )
                .await?;
            if legacy_path != scoped_path {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "active Git common config changed during format fallback",
                ));
            }
            if !legacy
                .status
                .code()
                .is_some_and(|code| code == 0 || code == 1)
            {
                return Err(io::Error::other(format!(
                    "git {probe} direct common config probe failed with status {}: {}",
                    legacy.status,
                    String::from_utf8_lossy(&legacy.stderr).trim()
                )));
            }
            parse_config_entries_with_origins(&legacy.stdout)?
        };
        for entry in &entries {
            let expected_origin = match &entry.origin {
                GitConfigOrigin::File(origin) => {
                    same_file::is_same_file(origin, &scoped_path).unwrap_or(false)
                }
                GitConfigOrigin::CommandLine => false,
            };
            if !expected_origin {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("git {probe} direct common config probe returned an unexpected origin"),
                ));
            }
        }
        Ok(entries
            .into_iter()
            .map(|entry| (entry.key.clone(), entry))
            .collect())
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
    // Captured once for an apply operation and appended after every mutable
    // config source/overlay so these scalar inputs cannot change between a
    // nonmutating gate and a later index/worktree mutation.
    apply_policy: Option<ApplyPolicySnapshot>,
    // Set only by the fixed real-policy numstat gate. Fatal whitespace modes
    // may be neutralized for a later mutating child only after this proof is
    // recorded on the same operation capability.
    apply_policy_gate: ApplyPolicyGateState,
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
    // mutation overlay. The retained context freezes helper selection, HEAD,
    // and untracked presence for the final tracked-only Status sink.
    status: Option<StatusPolicySnapshot>,
    // Set only after proving the effective replacement-ref namespace empty
    // (or already disabled). Every later snapshot command then forces
    // replacements off so a late ref cannot affect the frozen context.
    status_replacements_disabled: bool,
}

struct StatusPolicySnapshot {
    context: SealedStatusReadContext,
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

/// Opaque operation identity for sealed plans implemented in sibling modules.
/// Its inner capability cannot be forged or rebound outside this module.
#[derive(Clone)]
pub(crate) struct GuardedOperationIdentity(Arc<CapabilityIdentity>);

impl GuardedOperationIdentity {
    pub(crate) fn same_operation(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

enum ApplyPolicyGateState {
    NotRun,
    Failed,
    Succeeded { revert: bool, patch_path: String },
}

const APPLY_POLICY_CONFIG_PATTERN: &str =
    r"^(apply\.(whitespace|ignorewhitespace)|core\.whitespace)$";
const DEFAULT_APPLY_WHITESPACE: &str = "warn";
const DEFAULT_APPLY_IGNORE_WHITESPACE: &str = "false";
const DEFAULT_CORE_WHITESPACE: &str = "blank-at-eol,blank-at-eof,space-before-tab";

struct ApplyPolicySnapshot {
    config_args: Box<[String]>,
    whitespace_mode: ApplyWhitespaceMode,
}

/// Normalized `apply.whitespace` behavior frozen for one apply operation.
///
/// Unknown or case-mismatched values stay invalid here and are left for Git's
/// authoritative policy gate to reject. The `strip` alias has the same byte-
/// correcting behavior as `fix`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ApplyWhitespaceMode {
    Warn,
    Nowarn,
    Error,
    ErrorAll,
    Fix,
    Invalid,
}

impl ApplyWhitespaceMode {
    fn normalize(value: &str) -> Self {
        match value {
            "warn" => Self::Warn,
            "nowarn" => Self::Nowarn,
            "error" => Self::Error,
            "error-all" => Self::ErrorAll,
            "fix" | "strip" => Self::Fix,
            _ => Self::Invalid,
        }
    }

    pub(crate) fn is_fatal(self) -> bool {
        matches!(self, Self::Error | Self::ErrorAll)
    }
}

impl ApplyPolicySnapshot {
    fn capture(sources: &ValidatedConfigSources<'_>) -> io::Result<Self> {
        let entries = sources.read_effective(APPLY_POLICY_CONFIG_PATTERN, "apply policy")?;
        let value = |key: &str, default: &str| {
            entries
                .get(key)
                .map(|entry| entry.value.clone())
                .unwrap_or_else(|| default.to_string())
        };
        let apply_whitespace = value("apply.whitespace", DEFAULT_APPLY_WHITESPACE);
        Ok(Self {
            config_args: vec![
                "-c".to_string(),
                format!("apply.whitespace={apply_whitespace}"),
                "-c".to_string(),
                format!(
                    "apply.ignoreWhitespace={}",
                    value("apply.ignorewhitespace", DEFAULT_APPLY_IGNORE_WHITESPACE)
                ),
                "-c".to_string(),
                format!(
                    "core.whitespace={}",
                    value("core.whitespace", DEFAULT_CORE_WHITESPACE)
                ),
            ]
            .into_boxed_slice(),
            whitespace_mode: ApplyWhitespaceMode::normalize(&apply_whitespace),
        })
    }

    fn append_to(&self, command: &mut GitCommand) {
        command.args(&self.config_args);
    }

    fn append_rendered(&self, parts: &mut Vec<String>) {
        parts.extend(self.config_args.iter().cloned());
    }
}

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

    async fn output_in_status_context(
        self,
        context: &SealedStatusReadContext,
    ) -> io::Result<std::process::Output> {
        let isolated = context.context(&self.operation.identity)?;
        let operation_identity = self.operation.operation_identity();
        self.operation
            .sources
            .git
            .output_async_in_isolated_read_context(
                self.inner,
                isolated,
                &operation_identity,
                MAX_INTERNAL_GIT_OUTPUT_BYTES,
            )
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

    pub(crate) fn output_in_merge_scratch(
        self,
        storage: &IsolatedGitStorage,
    ) -> io::Result<std::process::Output> {
        let merge = self.operation.merge.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "scratch index command requires an installed merge policy",
            )
        })?;
        let isolated = merge.common_dir(&self.operation.identity)?;
        self.operation
            .sources
            .git
            .output_in_isolated_scratch(self.inner, isolated, storage)
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
            apply_policy: None,
            apply_policy_gate: ApplyPolicyGateState::NotRun,
            filters: Vec::new(),
            merge: None,
            merge_policy_installed: false,
            status: None,
            status_replacements_disabled: false,
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
            apply_policy: None,
            apply_policy_gate: ApplyPolicyGateState::NotRun,
            filters: Vec::new(),
            merge: None,
            merge_policy_installed: false,
            status: None,
            status_replacements_disabled: false,
        })
    }

    pub(crate) fn canonical_root(&self) -> &Path {
        &self.sources.canonical_root
    }

    pub(crate) fn operation_identity(&self) -> GuardedOperationIdentity {
        GuardedOperationIdentity(Arc::clone(&self.identity))
    }

    pub(crate) fn ensure_operation_identity(
        &self,
        identity: &GuardedOperationIdentity,
    ) -> io::Result<()> {
        if Arc::ptr_eq(&self.identity, &identity.0) {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "sealed Git plan belongs to another operation",
            ))
        }
    }

    pub(crate) fn freeze_apply_policy(&mut self) -> io::Result<()> {
        if self.status.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "apply policy cannot coexist with status policy",
            ));
        }
        if self.apply_policy.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "apply policy is already frozen for this operation",
            ));
        }
        self.apply_policy = Some(ApplyPolicySnapshot::capture(&self.sources)?);
        Ok(())
    }

    /// Parse the patch's path inventory without consulting or mutating the
    /// index/worktree. The fixed whitespace override keeps path discovery from
    /// preempting the later authoritative apply-policy result.
    pub(crate) fn run_apply_numstat_path_inventory(
        &self,
        revert: bool,
        patch_path: &Path,
    ) -> io::Result<std::process::Output> {
        let mut command = self.apply_command()?;
        command.args(["--numstat", "--whitespace=nowarn", "-z"]);
        if revert {
            command.arg("-R");
        }
        command.arg("--").arg(patch_path);
        command.output()
    }

    /// Run the one fixed, nonmutating policy gate and bind a successful result
    /// to this operation. Keeping the proof here prevents a future caller from
    /// suppressing a fatal final whitespace check without first executing the
    /// authoritative frozen-policy gate.
    pub(crate) fn run_apply_policy_gate(
        &mut self,
        revert: bool,
        patch_path: &str,
    ) -> io::Result<(String, std::process::Output)> {
        if !matches!(self.apply_policy_gate, ApplyPolicyGateState::NotRun) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "apply policy gate may run only once per operation",
            ));
        }
        if self.apply_policy.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "apply policy must be frozen before its gate runs",
            ));
        }
        // Mark the one-shot gate consumed before any fallible launch step.
        self.apply_policy_gate = ApplyPolicyGateState::Failed;
        let mut args = vec![
            "apply".to_string(),
            "--numstat".to_string(),
            "-z".to_string(),
        ];
        if revert {
            args.push("-R".to_string());
        }
        args.push(patch_path.to_string());
        let rendered = self.render_apply_command_for_log(&args)?;
        let mut command = self.apply_command()?;
        command.args(&args[1..]);
        let output = command.output()?;
        if output.status.success() {
            self.apply_policy_gate = ApplyPolicyGateState::Succeeded {
                revert,
                patch_path: patch_path.to_string(),
            };
        }
        Ok((rendered, output))
    }

    /// Run the public preflight shape without exposing a generic `git apply`
    /// command on which a caller could append mutating options.
    pub(crate) fn run_apply_preflight_check(
        &self,
        revert: bool,
        patch_path: &str,
    ) -> io::Result<(String, std::process::Output)> {
        let mut args = vec!["apply".to_string(), "--check".to_string()];
        if revert {
            args.push("-R".to_string());
        }
        args.push(patch_path.to_string());
        let rendered = self.render_apply_command_for_log(&args)?;
        let mut command = self.apply_command()?;
        command.args(&args[1..]);
        Ok((rendered, command.output()?))
    }

    /// Check whether the final operation can use the direct strategy. This is
    /// deliberately fixed to `--check`; callers receive only the output, never
    /// the underlying apply command.
    pub(crate) fn run_apply_strategy_check(
        &self,
        revert: bool,
        patch_path: &str,
    ) -> io::Result<std::process::Output> {
        let mut command = self.apply_command()?;
        command.args(["--check", "--whitespace=nowarn"]);
        if !revert {
            command.arg("--index");
        }
        if revert {
            command.arg("-R");
        }
        command.arg(patch_path);
        command.output()
    }

    /// Return the typed frozen mode only after the authoritative gate has
    /// succeeded for this operation.
    pub(crate) fn final_apply_whitespace_mode(
        &self,
        revert: bool,
        patch_path: &str,
    ) -> io::Result<ApplyWhitespaceMode> {
        match &self.apply_policy_gate {
            ApplyPolicyGateState::Succeeded {
                revert: gated_revert,
                patch_path: gated_patch,
            } if *gated_revert == revert && gated_patch == patch_path => {}
            ApplyPolicyGateState::Succeeded { .. } => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "final apply does not match its successful policy gate",
                ));
            }
            ApplyPolicyGateState::NotRun | ApplyPolicyGateState::Failed => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "final apply whitespace policy requires a successful policy gate",
                ));
            }
        }
        self.apply_policy
            .as_ref()
            .map(|policy| policy.whitespace_mode)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "final apply whitespace policy is not frozen",
                )
            })
    }

    fn apply_command(&self) -> io::Result<GuardedGitCommand<'_, 'git>> {
        let mut command = self.command_with_attached_overlays()?;
        if let Some(policy) = &self.apply_policy {
            policy.append_to(&mut command);
        }
        BoundSubcommand::Apply.append_to(&mut command);
        Ok(GuardedGitCommand {
            operation: self,
            inner: command,
        })
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
        if self.apply_policy.is_some()
            || !matches!(self.apply_policy_gate, ApplyPolicyGateState::NotRun)
            || !self.filters.is_empty()
            || self.merge.is_some()
            || self.merge_policy_installed
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "status policy cannot coexist with apply, mutation filter, or merge policy",
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

    async fn read_status_untracked_presence_async(
        &self,
        neutralizer: Option<&SealedFilterConfigOverride>,
    ) -> io::Result<bool> {
        let mut command = self.pending_status_command(FsmonitorOverride::Disabled, neutralizer)?;
        command.disable_optional_locks().args([
            "ls-files",
            "-z",
            "--others",
            "--exclude-standard",
            "--directory",
            "--no-empty-directory",
            "--",
        ]);
        let output = command.output().await?;
        if !output.status.success() {
            return Err(command_failure(
                "status untracked-presence inventory",
                &output,
            ));
        }
        let paths = parse_nul_paths(&output.stdout)?;
        if paths.len() > MAX_STATUS_TRACKED_PATHS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "status untracked-presence inventory exceeds its path limit",
            ));
        }
        Ok(!paths.is_empty())
    }

    async fn read_status_head_oid_async(
        &self,
        neutralizer: Option<&SealedFilterConfigOverride>,
    ) -> io::Result<Option<String>> {
        let mut command = self.pending_status_command(FsmonitorOverride::Disabled, neutralizer)?;
        command.disable_optional_locks().args([
            "rev-parse",
            "--verify",
            "--quiet",
            "HEAD^{commit}",
        ]);
        let output = command.output().await?;
        if output.status.success() {
            return parse_status_head_oid(&output.stdout).map(Some);
        }
        if output.status.code() != Some(1) || !output.stdout.is_empty() || !output.stderr.is_empty()
        {
            return Err(command_failure("status HEAD snapshot", &output));
        }

        let mut symbolic = self.pending_status_command(FsmonitorOverride::Disabled, neutralizer)?;
        symbolic
            .disable_optional_locks()
            .args(["symbolic-ref", "--quiet", "HEAD"]);
        let symbolic = symbolic.output().await?;
        if !symbolic.status.success() {
            return Err(command_failure("status symbolic HEAD snapshot", &symbolic));
        }
        let target = parse_status_symbolic_ref(&symbolic.stdout)?;

        let mut verify = self.pending_status_command(FsmonitorOverride::Disabled, neutralizer)?;
        verify
            .disable_optional_locks()
            .args(["show-ref", "--verify", "--quiet", "--"])
            .arg(&target);
        let verify = verify.output().await?;
        if verify.status.code() == Some(1) && verify.stdout.is_empty() && verify.stderr.is_empty() {
            return Ok(None);
        }
        if verify.status.success() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Status HEAD target exists but does not resolve to a commit",
            ));
        }
        Err(command_failure("status unborn HEAD verification", &verify))
    }

    async fn ensure_no_effective_replacement_refs_async(
        &mut self,
        neutralizer: Option<&SealedFilterConfigOverride>,
    ) -> io::Result<()> {
        if self.sources.git.replacement_refs_are_disabled() {
            self.status_replacements_disabled = true;
            return Ok(());
        }
        if self.sources.git.replacement_ref_base_is_custom() {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "frozen Status is unavailable with a custom Git replacement-ref namespace",
            ));
        }
        let mut command = self.pending_status_command(FsmonitorOverride::Disabled, neutralizer)?;
        command.disable_optional_locks().args([
            "for-each-ref",
            "--format=%(refname)",
            "refs/replace/",
        ]);
        let output = command.output().await?;
        if !output.status.success() {
            return Err(command_failure("status replacement-ref inventory", &output));
        }
        if output.stdout.is_empty() {
            self.status_replacements_disabled = true;
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "frozen Status is unavailable with active Git replacement refs",
            ))
        }
    }

    pub(crate) async fn detect_status_fsmonitor_async(&mut self) -> FsmonitorOverride {
        if self.status.is_none() || self.ensure_status_exclusive_state().is_err() {
            return FsmonitorOverride::Disabled;
        }
        if let Some(fsmonitor) = self.status.as_ref().and_then(|status| status.fsmonitor) {
            return fsmonitor;
        }
        if let Some(status) = &mut self.status {
            // The synthetic owned Git directory has no stable fsmonitor daemon
            // identity or socket lifecycle. Disabling it is a performance-only
            // downgrade and avoids launching a daemon tied to temporary state.
            status.fsmonitor = Some(FsmonitorOverride::Disabled);
        }
        FsmonitorOverride::Disabled
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
        let mut command = self.pending_frozen_status_command(fsmonitor)?;
        command.disable_optional_locks().args([
            "status",
            "--porcelain",
            "--ignore-submodules=dirty",
            "--untracked-files=no",
        ]);
        command.output_in_status_context(&status.context).await
    }

    pub(crate) fn status_has_untracked_snapshot(&self) -> io::Result<bool> {
        let status = self.status.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "untracked status requires an installed status filter policy",
            )
        })?;
        status.context.has_untracked(&self.identity)
    }

    fn pending_frozen_status_command<'operation>(
        &'operation self,
        fsmonitor: FsmonitorOverride,
    ) -> io::Result<GuardedAsyncGitCommand<'operation, 'git>> {
        self.ensure_status_exclusive_state()?;
        if self.status.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "frozen Status command requires an installed context",
            ));
        }
        let mut command = self
            .sources
            .git
            .async_command_for_cwd(&self.sources.canonical_root)?;
        command.args([
            "-c",
            &format!("core.hooksPath={DISABLED_HOOKS_PATH}"),
            "-c",
            fsmonitor.git_config_arg(),
        ]);
        Ok(GuardedAsyncGitCommand {
            operation: self,
            inner: command,
        })
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
        if self.status_replacements_disabled {
            command.env("GIT_NO_REPLACE_OBJECTS", "1");
        }
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
        // The destination is operation-owned and absolute. Avoid repository
        // setup while serializing the already inventoried driver names: a
        // malformed repository setting must not preempt the typed policy
        // error that the guarded operation reports later.
        let mut command = self.sources.git.command();
        command.args(["config", "--file"]).arg(config_path).args([
            "--add",
            &format!("filter.{driver}.{name}"),
            value,
        ]);
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
                "reverse exact staging requires exactly one apply filter snapshot",
            ));
        };
        if apply.role() != FilterPolicyRole::Apply {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "reverse exact staging requires an apply filter snapshot",
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

    pub(crate) fn run_direct_apply(
        &self,
        revert: bool,
        patch_path: &str,
    ) -> io::Result<std::process::Output> {
        let mode = self.final_apply_whitespace_mode(revert, patch_path)?;
        let mut command = self.apply_command()?;
        command.arg("--index");
        if mode.is_fatal() {
            command.arg("--whitespace=nowarn");
        }
        if revert {
            command.arg("-R");
        }
        command.arg(patch_path);
        command.output()
    }

    pub(crate) fn render_direct_apply_for_log(
        &self,
        revert: bool,
        patch_path: &str,
    ) -> io::Result<String> {
        let mode = self.final_apply_whitespace_mode(revert, patch_path)?;
        let mut args = vec!["apply".to_string(), "--index".to_string()];
        if mode.is_fatal() {
            args.push("--whitespace=nowarn".to_string());
        }
        if revert {
            args.push("-R".to_string());
        }
        args.push(patch_path.to_string());
        self.render_apply_command_for_log(&args)
    }

    pub(crate) fn run_three_way_apply(
        &mut self,
        revert: bool,
        patch_path: &str,
    ) -> io::Result<std::process::Output> {
        let (apply, _git_add) = self.ordered_filter_snapshots()?;
        if apply.is_none() || !self.merge_policy_installed {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "three-way apply policy is not installed",
            ));
        }
        self.consume_three_way_merge_policy_proof(revert, patch_path)?;
        let merge = self.merge.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "isolated three-way config is unavailable",
            )
        })?;
        let isolated = merge.common_dir(&self.identity)?;
        let mut command = self
            .sources
            .git
            .command_for_cwd(&self.sources.canonical_root)?;
        append_safe_scalar_overrides(&mut command);
        self.apply_policy
            .as_ref()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "three-way apply policy scalars are not frozen",
                )
            })?
            .append_to(&mut command);
        BoundSubcommand::Apply.append_to(&mut command);
        command.arg("--3way");
        if self
            .final_apply_whitespace_mode(revert, patch_path)?
            .is_fatal()
        {
            command.arg("--whitespace=nowarn");
        }
        if revert {
            command.arg("-R");
        }
        command.arg(patch_path);
        self.sources
            .git
            .output_in_isolated_common_dir(command, isolated)
    }

    pub(crate) fn render_three_way_apply_for_log(
        &self,
        revert: bool,
        patch_path: &str,
    ) -> io::Result<String> {
        // Rendering may precede the final post-staging index revalidation,
        // but it must never render a selected-custom operation with no proof.
        self.ensure_three_way_merge_policy_proof_installed()?;
        let merge = self.merge.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "isolated three-way config is unavailable",
            )
        })?;
        let _ = merge.common_dir(&self.identity)?;
        let mut parts = vec![
            "env".to_string(),
            "GIT_COMMON_DIR=<isolated>".to_string(),
            "git".to_string(),
        ];
        parts.extend(safe_scalar_override_args());
        self.apply_policy
            .as_ref()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "three-way apply policy scalars are not frozen",
                )
            })?
            .append_rendered(&mut parts);
        parts.push("apply".to_string());
        parts.push("--3way".to_string());
        if self
            .final_apply_whitespace_mode(revert, patch_path)?
            .is_fatal()
        {
            parts.push("--whitespace=nowarn".to_string());
        }
        if revert {
            parts.push("-R".to_string());
        }
        parts.push(patch_path.to_string());
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

    #[cfg(test)]
    pub(crate) fn merge_common_config_path(&self) -> Option<PathBuf> {
        self.merge
            .as_ref()
            .and_then(|merge| merge.common_dir(&self.identity).ok())
            .map(crate::git_command::IsolatedGitCommonDir::config_path)
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
            [apply, git_add]
                if apply.role() == FilterPolicyRole::Apply
                    && git_add.role() == FilterPolicyRole::GitAdd
                    && git_add.checked_paths().into_iter().collect::<BTreeSet<_>>()
                        == paths.iter().cloned().collect::<BTreeSet<_>>() =>
            {
                return Ok(());
            }
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

    #[cfg(test)]
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

    pub(crate) fn render_apply_command_for_log(&self, args: &[String]) -> io::Result<String> {
        let mut parts = vec!["git".to_string()];
        parts.extend(self.sources.base_config_args.iter().cloned());
        parts.extend(safe_scalar_override_args());
        let (apply, git_add) = self.ordered_filter_snapshots()?;
        if let Some(neutralizer) = apply.and_then(FilterPolicySnapshot::neutralizer) {
            neutralizer.append_rendered_args(&self.identity, &mut parts)?;
        }
        if let Some(neutralizer) = git_add.and_then(FilterPolicySnapshot::neutralizer) {
            neutralizer.append_rendered_args(&self.identity, &mut parts)?;
        }
        if let Some(policy) = &self.apply_policy {
            policy.append_rendered(&mut parts);
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

fn parse_status_head_oid(output: &[u8]) -> io::Result<String> {
    let line = output.strip_suffix(b"\n").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "unterminated Status HEAD output",
        )
    })?;
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    if !matches!(line.len(), 40 | 64) || !line.iter().all(u8::is_ascii_hexdigit) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Status HEAD did not resolve to one full object ID",
        ));
    }
    String::from_utf8(line.to_ascii_lowercase()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Status HEAD output was not a valid hexadecimal object ID",
        )
    })
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

fn parse_status_symbolic_ref(output: &[u8]) -> io::Result<std::ffi::OsString> {
    let line = output.strip_suffix(b"\n").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "unterminated Status symbolic HEAD output",
        )
    })?;
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    if !line.starts_with(b"refs/heads/")
        || line.contains(&0)
        || line
            .iter()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Status HEAD did not resolve to a safe symbolic ref",
        ));
    }
    git_path_argument(line)
}

fn append_safe_scalar_overrides(command: &mut GitCommand) {
    command.args(safe_scalar_override_args());
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
