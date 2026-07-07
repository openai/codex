use std::ffi::OsStr;
use std::io;
use std::path::PathBuf;
use std::process::Stdio;

use super::ApplyPolicyGateState;
use super::GuardedGitConfig;
use super::SealedFilterConfigOverride;
use crate::FsmonitorOverride;
use crate::git_command::GitAsyncCommand;
use crate::git_command::MAX_INTERNAL_GIT_OUTPUT_BYTES;
use crate::safe_git::DISABLED_HOOKS_PATH;
use crate::safe_git::git_path_argument;

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

/// Closed async command assembly for Status preparation and the final sink.
/// The raw Tokio command never leaves this module, so callers cannot change
/// its cwd, config invocation, overlay order, or final metadata revalidation.
pub(super) struct GuardedAsyncGitCommand<'operation, 'git> {
    operation: &'operation GuardedGitConfig<'git>,
    inner: GitAsyncCommand,
}

impl GuardedAsyncGitCommand<'_, '_> {
    pub(super) fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
        self.inner.arg(arg);
        self
    }

    pub(super) fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.inner.args(args);
        self
    }

    pub(super) fn disable_optional_locks(&mut self) -> &mut Self {
        self.inner.env("GIT_OPTIONAL_LOCKS", "0");
        self
    }

    pub(super) fn stdin(&mut self, config: impl Into<Stdio>) -> &mut Self {
        self.inner.stdin(config);
        self
    }

    pub(super) async fn output(self) -> io::Result<std::process::Output> {
        self.operation
            .sources
            .git
            .output_async_bounded(self.inner, MAX_INTERNAL_GIT_OUTPUT_BYTES)
            .await
    }

    pub(super) async fn output_in_status_context(
        self,
        context: &super::status_context::SealedStatusReadContext,
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

impl<'git> GuardedGitConfig<'git> {
    pub(super) fn status_config_args(
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

    pub(super) fn ensure_status_exclusive_state(&self) -> io::Result<()> {
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

    pub(super) fn pending_frozen_status_command<'operation>(
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

    pub(super) fn pending_status_command<'operation>(
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
}

pub(super) fn command_failure(description: &str, output: &std::process::Output) -> io::Error {
    io::Error::other(StatusPolicyCommandFailure {
        description: description.to_string(),
        exit_code: output.status.code(),
    })
}

pub(super) fn git_path_from_line_output(output: &[u8]) -> io::Result<PathBuf> {
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

pub(super) fn parse_status_head_oid(output: &[u8]) -> io::Result<String> {
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

pub(super) fn parse_status_symbolic_ref(output: &[u8]) -> io::Result<std::ffi::OsString> {
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
