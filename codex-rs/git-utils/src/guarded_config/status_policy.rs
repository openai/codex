use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use super::ApplyPolicyGateState;
use super::CapabilityIdentity;
use super::GuardedGitConfig;
use super::status_command::command_failure;
use super::status_command::git_path_from_line_output;
use super::status_command::parse_status_head_oid;
use super::status_command::parse_status_symbolic_ref;
use super::status_context::SealedStatusReadContext;
use crate::FsmonitorOverride;
use crate::git_command::GitRunner;
use crate::git_command::MAX_INTERNAL_GIT_OUTPUT_BYTES;
use crate::safe_git::parse_nul_paths;

pub(super) const MAX_STATUS_TRACKED_PATHS: usize = 250_000;

pub(super) struct StatusPolicySnapshot {
    pub(super) context: SealedStatusReadContext,
    pub(super) fsmonitor: Option<FsmonitorOverride>,
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

impl StatusRootMismatch {
    pub(crate) fn expected(&self) -> &Path {
        &self.expected
    }

    pub(crate) fn reported(&self) -> &Path {
        &self.reported
    }
}

impl<'git> GuardedGitConfig<'git> {
    pub(crate) async fn authorize_status_async(git: &'git GitRunner) -> io::Result<Self> {
        let root = git
            .active_worktree_root()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, NoActiveStatusWorktree))?;
        git.ensure_repository_root_route(root)?;
        Ok(Self {
            sources: super::ValidatedConfigSources::authorize_async(git, root, Vec::new()).await?,
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

    pub(super) async fn read_status_untracked_presence_async(
        &self,
        neutralizer: Option<&super::SealedFilterConfigOverride>,
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

    pub(super) async fn read_status_head_oid_async(
        &self,
        neutralizer: Option<&super::SealedFilterConfigOverride>,
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

    pub(super) async fn ensure_no_effective_replacement_refs_async(
        &mut self,
        neutralizer: Option<&super::SealedFilterConfigOverride>,
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
}

#[cfg(test)]
#[path = "status_policy_tests.rs"]
mod tests;
