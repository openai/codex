use std::io;
use std::path::Path;

use crate::FsmonitorOverride;
use crate::GitReadError;
use crate::git_command::GitRunner;
use crate::guarded_config::GuardedGitConfig;
use crate::guarded_config::NoActiveStatusWorktree;
use crate::guarded_config::SelectedStatusFilterRefusal;
use crate::guarded_config::StatusFilterProbeLimitExceeded;
use crate::guarded_config::StatusPolicyCommandFailure;
use crate::guarded_config::StatusRootMismatch;
use crate::repository_authority::is_authority_refusal;

/// One retained capability owns source authorization, filter policy, fsmonitor
/// selection, and the final bounded status child. The caller supplies the
/// phase transitions so the one outer deadline can report precise metadata.
pub(crate) async fn prepare_status_config<'git>(
    git: &'git GitRunner,
    requested_cwd: &Path,
) -> Result<GuardedGitConfig<'git>, GitReadError> {
    let mut config = GuardedGitConfig::authorize_status_async(git)
        .await
        .map_err(|error| {
            if error
                .get_ref()
                .and_then(|source| source.downcast_ref::<NoActiveStatusWorktree>())
                .is_some()
            {
                GitReadError::NotRepository {
                    path: requested_cwd.to_path_buf(),
                }
            } else {
                map_io_error("statusFilterPreparation", error)
            }
        })?;
    config
        .verify_status_root_async(requested_cwd)
        .await
        .map_err(|error| map_io_error("resolveGitRoot", error))?;
    config
        .install_status_policy_async()
        .await
        .map_err(|error| map_io_error("statusFilterPreparation", error))?;
    Ok(config)
}

pub(crate) async fn detect_status_fsmonitor(
    config: &mut GuardedGitConfig<'_>,
) -> FsmonitorOverride {
    config.detect_status_fsmonitor_async().await
}

pub(crate) async fn read_status(config: &GuardedGitConfig<'_>) -> Result<bool, GitReadError> {
    let output = config
        .status_output_async()
        .await
        .map_err(|error| map_io_error("status", error))?;
    if !output.status.success() {
        return Err(GitReadError::CommandFailed {
            operation: "status".to_string(),
            exit_code: output.status.code(),
        });
    }
    Ok(!output.stdout.is_empty())
}

pub(crate) fn map_io_error(operation: &str, error: io::Error) -> GitReadError {
    if let Some(refusal) = error
        .get_ref()
        .and_then(|source| source.downcast_ref::<SelectedStatusFilterRefusal>())
    {
        return GitReadError::SelectedExecutableFilter {
            driver: refusal.driver().to_string(),
            path: String::from_utf8_lossy(refusal.path()).into_owned(),
        };
    }
    if let Some(mismatch) = error
        .get_ref()
        .and_then(|source| source.downcast_ref::<StatusRootMismatch>())
    {
        return GitReadError::RepositoryRootMismatch {
            expected_root: mismatch.expected().to_path_buf(),
            reported_root: mismatch.reported().to_path_buf(),
        };
    }
    if let Some(limit) = error
        .get_ref()
        .and_then(|source| source.downcast_ref::<StatusFilterProbeLimitExceeded>())
    {
        return GitReadError::FilterSelectionProbeLimitExceeded {
            max_probes: limit.max_probes(),
        };
    }
    if let Some(failure) = error
        .get_ref()
        .and_then(|source| source.downcast_ref::<StatusPolicyCommandFailure>())
    {
        return GitReadError::CommandFailed {
            operation: operation.to_string(),
            exit_code: failure.exit_code(),
        };
    }
    if is_authority_refusal(&error) {
        return GitReadError::AuthorityRefused {
            operation: operation.to_string(),
        };
    }
    match error.kind() {
        io::ErrorKind::TimedOut => GitReadError::CommandTimedOut {
            operation: operation.to_string(),
        },
        io::ErrorKind::InvalidData => GitReadError::InvalidOutput {
            operation: operation.to_string(),
        },
        _ => GitReadError::CommandFailed {
            operation: operation.to_string(),
            exit_code: None,
        },
    }
}

#[cfg(test)]
#[path = "status_guard_tests.rs"]
mod tests;
