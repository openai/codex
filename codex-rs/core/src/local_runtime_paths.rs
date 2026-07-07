use crate::config::Config;
use codex_exec_server::EnvironmentManager;
use codex_protocol::error::CodexErr;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_utils_absolute_path::AbsolutePathBuf;

/// Host-local paths available to a [`crate::ThreadManager`].
///
/// The capability is all-or-none: remote-only managers receive `None` instead
/// of placeholder paths that could accidentally target a shared host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalRuntimePaths {
    /// Stable host-local state root owned by the manager.
    pub codex_home: AbsolutePathBuf,
    /// Initial local workspace. Each local thread refreshes this from its
    /// effective [`Config`] so per-thread cwd overrides keep working.
    pub cwd: AbsolutePathBuf,
}

impl From<&Config> for LocalRuntimePaths {
    fn from(config: &Config) -> Self {
        Self {
            codex_home: config.codex_home.clone(),
            cwd: config.cwd.clone(),
        }
    }
}

/// Failure to construct a thread manager with a coherent runtime capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ThreadManagerInitError {
    #[error("the configured thread store requires local runtime paths")]
    ThreadStoreRequiresLocalRuntimePaths,
    #[error("the configured agent graph store requires local runtime paths")]
    AgentGraphStoreRequiresLocalRuntimePaths,
}

/// Invalid environment configuration for a remote-only thread.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ThreadStartError {
    #[error("a thread without local runtime paths requires at least one remote environment")]
    MissingRemoteEnvironment,
    #[error("unknown turn environment id `{environment_id}`")]
    UnknownEnvironment { environment_id: String },
    #[error(
        "a thread without local runtime paths cannot select local environment `{environment_id}`"
    )]
    LocalEnvironmentNotAllowed { environment_id: String },
    #[error(
        "remote environment `{environment_id}` cwd cannot be used as the thread's legacy cwd: {message}"
    )]
    InvalidRemoteCwd {
        environment_id: String,
        message: String,
    },
    #[error(
        "remote-only thread legacy cwd `{actual}` must match the first remote environment cwd `{expected}`"
    )]
    LegacyCwdMismatch {
        expected: AbsolutePathBuf,
        actual: AbsolutePathBuf,
    },
    #[error("a thread without local runtime paths cannot use command-backed model auth")]
    CommandModelAuthNotAllowed,
}

impl From<ThreadStartError> for CodexErr {
    fn from(error: ThreadStartError) -> Self {
        Self::InvalidRequest(error.to_string())
    }
}

pub(crate) fn validate_remote_environment_selections(
    environment_manager: &EnvironmentManager,
    selections: &[TurnEnvironmentSelection],
) -> Result<(), ThreadStartError> {
    if selections.is_empty() {
        return Err(ThreadStartError::MissingRemoteEnvironment);
    }

    for selection in selections {
        let environment = environment_manager
            .get_environment(&selection.environment_id)
            .ok_or_else(|| ThreadStartError::UnknownEnvironment {
                environment_id: selection.environment_id.clone(),
            })?;
        if !environment.is_remote() {
            return Err(ThreadStartError::LocalEnvironmentNotAllowed {
                environment_id: selection.environment_id.clone(),
            });
        }
    }

    Ok(())
}

pub(crate) fn remote_legacy_cwd(
    environment_manager: &EnvironmentManager,
    selections: &[TurnEnvironmentSelection],
) -> Result<AbsolutePathBuf, ThreadStartError> {
    validate_remote_environment_selections(environment_manager, selections)?;
    let selection = &selections[0];
    selection
        .cwd
        .to_abs_path()
        .map_err(|error| ThreadStartError::InvalidRemoteCwd {
            environment_id: selection.environment_id.clone(),
            message: error.to_string(),
        })
}
