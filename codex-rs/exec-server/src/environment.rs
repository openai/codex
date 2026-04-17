use std::collections::HashMap;
use std::sync::Arc;

use crate::ExecServerError;
use crate::ExecServerRuntimePaths;
use crate::client::LazyRemoteExecServerClient;
use crate::file_system::ExecutorFileSystem;
use crate::local_file_system::LocalFileSystem;
use crate::local_process::LocalProcess;
use crate::process::ExecBackend;
use crate::remote_file_system::RemoteFileSystem;
use crate::remote_process::RemoteProcess;

pub const CODEX_EXEC_SERVER_URL_ENV_VAR: &str = "CODEX_EXEC_SERVER_URL";

/// Owns the execution/filesystem environments available to a session.
///
/// `EnvironmentManager` is the session-scoped registry for concrete
/// environments. It creates a local environment under [`LOCAL_ENVIRONMENT_ID`]
/// unless environment access is disabled. When `CODEX_EXEC_SERVER_URL` is set to
/// a websocket URL, it also creates a remote environment under
/// [`REMOTE_ENVIRONMENT_ID`] and makes that the default environment. Otherwise
/// the local environment is the default.
///
/// Setting `CODEX_EXEC_SERVER_URL=none` disables environment access by leaving
/// the default environment unset. Callers use `default_environment().is_some()`
/// as the signal for model-facing shell/filesystem tool availability.
///
/// Remote environments hold a lazy exec-server client handle. The websocket is
/// not opened when the manager or environment is constructed; it connects on the
/// first remote exec or filesystem operation.
#[derive(Debug)]
pub struct EnvironmentManager {
    default_environment: Option<String>,
    environments: HashMap<String, Arc<Environment>>,
}

pub const LOCAL_ENVIRONMENT_ID: &str = "local";
pub const REMOTE_ENVIRONMENT_ID: &str = "remote";

#[derive(Clone, Debug, Default)]
pub struct EnvironmentManagerArgs {
    pub exec_server_url: Option<String>,
    pub local_runtime_paths: Option<ExecServerRuntimePaths>,
}

impl Default for EnvironmentManager {
    fn default() -> Self {
        Self::new(EnvironmentManagerArgs::default())
    }
}

impl EnvironmentManager {
    /// Builds a manager from process environment variables.
    pub fn from_env() -> Self {
        Self::new(EnvironmentManagerArgs {
            exec_server_url: std::env::var(CODEX_EXEC_SERVER_URL_ENV_VAR).ok(),
            local_runtime_paths: None,
        })
    }

    /// Builds a manager from the raw `CODEX_EXEC_SERVER_URL` value and local
    /// runtime paths used when creating local filesystem helpers.
    pub fn new(args: EnvironmentManagerArgs) -> Self {
        let EnvironmentManagerArgs {
            exec_server_url,
            local_runtime_paths,
        } = args;
        let (exec_server_url, environment_disabled) = normalize_exec_server_url(exec_server_url);
        let mut environments = HashMap::new();
        let default_environment = if environment_disabled {
            None
        } else {
            environments.insert(
                LOCAL_ENVIRONMENT_ID.to_string(),
                Arc::new(
                    Environment::create_with_runtime_paths(
                        /*exec_server_url*/ None,
                        local_runtime_paths.clone(),
                    )
                    .expect("valid local environment"),
                ),
            );
            match exec_server_url {
                Some(exec_server_url) => {
                    environments.insert(
                        REMOTE_ENVIRONMENT_ID.to_string(),
                        Arc::new(
                            Environment::create_with_runtime_paths(
                                Some(exec_server_url),
                                local_runtime_paths,
                            )
                            .expect("valid remote environment"),
                        ),
                    );
                    Some(REMOTE_ENVIRONMENT_ID.to_string())
                }
                None => Some(LOCAL_ENVIRONMENT_ID.to_string()),
            }
        };

        Self {
            default_environment,
            environments,
        }
    }

    /// Returns the default environment instance.
    pub fn default_environment(&self) -> Option<Arc<Environment>> {
        self.default_environment
            .as_deref()
            .and_then(|environment_id| self.get_environment(environment_id))
    }

    /// Returns a named environment instance.
    pub fn get_environment(&self, environment_id: &str) -> Option<Arc<Environment>> {
        self.environments.get(environment_id).cloned()
    }
}

/// Concrete execution/filesystem environment selected for a session.
///
/// This bundles the selected backend together with the corresponding remote
/// client, if any.
#[derive(Clone)]
pub struct Environment {
    exec_server_url: Option<String>,
    remote_exec_server_client: Option<LazyRemoteExecServerClient>,
    exec_backend: Arc<dyn ExecBackend>,
    local_runtime_paths: Option<ExecServerRuntimePaths>,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            exec_server_url: None,
            remote_exec_server_client: None,
            exec_backend: Arc::new(LocalProcess::default()),
            local_runtime_paths: None,
        }
    }
}

impl std::fmt::Debug for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Environment")
            .field("exec_server_url", &self.exec_server_url)
            .finish_non_exhaustive()
    }
}

impl Environment {
    /// Builds an environment from the raw `CODEX_EXEC_SERVER_URL` value.
    pub fn create(exec_server_url: Option<String>) -> Result<Self, ExecServerError> {
        Self::create_with_runtime_paths(exec_server_url, /*local_runtime_paths*/ None)
    }

    /// Builds an environment from the raw `CODEX_EXEC_SERVER_URL` value and
    /// local runtime paths used when creating local filesystem helpers.
    pub(crate) fn create_with_runtime_paths(
        exec_server_url: Option<String>,
        local_runtime_paths: Option<ExecServerRuntimePaths>,
    ) -> Result<Self, ExecServerError> {
        let (exec_server_url, disabled) = normalize_exec_server_url(exec_server_url);
        if disabled {
            return Err(ExecServerError::Protocol(
                "disabled mode does not create an Environment".to_string(),
            ));
        }

        let remote_exec_server_client = if let Some(exec_server_url) = exec_server_url.clone() {
            Some(LazyRemoteExecServerClient::new(exec_server_url))
        } else {
            None
        };

        let exec_backend: Arc<dyn ExecBackend> =
            if let Some(client) = remote_exec_server_client.clone() {
                Arc::new(RemoteProcess::new(client))
            } else {
                Arc::new(LocalProcess::default())
            };

        Ok(Self {
            exec_server_url,
            remote_exec_server_client,
            exec_backend,
            local_runtime_paths,
        })
    }

    pub fn is_remote(&self) -> bool {
        self.exec_server_url.is_some()
    }

    /// Returns the remote exec-server URL when this environment is remote.
    pub fn exec_server_url(&self) -> Option<&str> {
        self.exec_server_url.as_deref()
    }

    pub fn local_runtime_paths(&self) -> Option<&ExecServerRuntimePaths> {
        self.local_runtime_paths.as_ref()
    }

    pub fn get_exec_backend(&self) -> Arc<dyn ExecBackend> {
        Arc::clone(&self.exec_backend)
    }

    pub fn get_filesystem(&self) -> Arc<dyn ExecutorFileSystem> {
        match self.remote_exec_server_client.clone() {
            Some(client) => Arc::new(RemoteFileSystem::new(client)),
            None => match self.local_runtime_paths.clone() {
                Some(runtime_paths) => Arc::new(LocalFileSystem::with_runtime_paths(runtime_paths)),
                None => Arc::new(LocalFileSystem::unsandboxed()),
            },
        }
    }
}

fn normalize_exec_server_url(exec_server_url: Option<String>) -> (Option<String>, bool) {
    match exec_server_url.as_deref().map(str::trim) {
        None | Some("") => (None, false),
        Some(url) if url.eq_ignore_ascii_case("none") => (None, true),
        Some(url) => (Some(url.to_string()), false),
    }
}
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::Environment;
    use super::EnvironmentManager;
    use super::EnvironmentManagerArgs;
    use super::LOCAL_ENVIRONMENT_ID;
    use super::REMOTE_ENVIRONMENT_ID;
    use crate::ExecServerRuntimePaths;
    use crate::ProcessId;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn create_local_environment_does_not_connect() {
        let environment =
            Environment::create(/*exec_server_url*/ None).expect("create environment");

        assert_eq!(environment.exec_server_url(), None);
        assert!(environment.remote_exec_server_client.is_none());
    }

    #[tokio::test]
    async fn environment_manager_normalizes_empty_url() {
        let manager = EnvironmentManager::new(EnvironmentManagerArgs {
            exec_server_url: Some(String::new()),
            local_runtime_paths: None,
        });

        let environment = manager.default_environment().expect("default environment");
        assert!(!environment.is_remote());
        assert!(
            !manager
                .get_environment(LOCAL_ENVIRONMENT_ID)
                .expect("local environment")
                .is_remote()
        );
        assert!(manager.get_environment(REMOTE_ENVIRONMENT_ID).is_none());
    }

    #[test]
    fn environment_manager_treats_none_value_as_disabled() {
        let manager = EnvironmentManager::new(EnvironmentManagerArgs {
            exec_server_url: Some("none".to_string()),
            local_runtime_paths: None,
        });

        assert!(manager.default_environment().is_none());
        assert!(manager.get_environment(LOCAL_ENVIRONMENT_ID).is_none());
        assert!(manager.get_environment(REMOTE_ENVIRONMENT_ID).is_none());
    }

    #[tokio::test]
    async fn environment_manager_reports_remote_url() {
        let manager = EnvironmentManager::new(EnvironmentManagerArgs {
            exec_server_url: Some("ws://127.0.0.1:8765".to_string()),
            local_runtime_paths: None,
        });

        let environment = manager.default_environment().expect("default environment");
        assert!(environment.is_remote());
        assert_eq!(environment.exec_server_url(), Some("ws://127.0.0.1:8765"));
        assert!(
            !manager
                .get_environment(LOCAL_ENVIRONMENT_ID)
                .expect("local environment")
                .is_remote()
        );
        assert_eq!(
            manager
                .get_environment(REMOTE_ENVIRONMENT_ID)
                .expect("remote environment")
                .exec_server_url(),
            Some("ws://127.0.0.1:8765")
        );
    }

    #[tokio::test]
    async fn environment_manager_default_environment_caches_environment() {
        let manager = EnvironmentManager::new(EnvironmentManagerArgs::default());

        let first = manager.default_environment().expect("default environment");
        let second = manager.default_environment().expect("default environment");

        assert!(Arc::ptr_eq(&first, &second));
    }

    #[tokio::test]
    async fn environment_manager_carries_local_runtime_paths() {
        let runtime_paths = ExecServerRuntimePaths::new(
            std::env::current_exe().expect("current exe"),
            /*codex_linux_sandbox_exe*/ None,
        )
        .expect("runtime paths");
        let manager = EnvironmentManager::new(EnvironmentManagerArgs {
            exec_server_url: None,
            local_runtime_paths: Some(runtime_paths.clone()),
        });

        let environment = manager.default_environment().expect("default environment");

        assert_eq!(environment.local_runtime_paths(), Some(&runtime_paths));
        let manager = EnvironmentManager::new(EnvironmentManagerArgs {
            exec_server_url: environment.exec_server_url().map(str::to_owned),
            local_runtime_paths: environment.local_runtime_paths().cloned(),
        });
        let environment = manager.default_environment().expect("default environment");
        assert_eq!(environment.local_runtime_paths(), Some(&runtime_paths));
    }

    #[tokio::test]
    async fn disabled_environment_manager_has_no_default_environment() {
        let manager = EnvironmentManager::new(EnvironmentManagerArgs {
            exec_server_url: Some("none".to_string()),
            local_runtime_paths: None,
        });

        assert!(manager.default_environment().is_none());
    }

    #[tokio::test]
    async fn environment_manager_omits_environment_lookup_when_disabled() {
        let manager = EnvironmentManager::new(EnvironmentManagerArgs {
            exec_server_url: Some("none".to_string()),
            local_runtime_paths: None,
        });

        assert!(manager.default_environment().is_none());
        assert!(manager.get_environment(LOCAL_ENVIRONMENT_ID).is_none());
        assert!(manager.get_environment(REMOTE_ENVIRONMENT_ID).is_none());
    }

    #[tokio::test]
    async fn get_environment_returns_none_for_unknown_id() {
        let manager = EnvironmentManager::new(EnvironmentManagerArgs::default());

        assert!(manager.get_environment("does-not-exist").is_none());
    }

    #[tokio::test]
    async fn default_environment_has_ready_local_executor() {
        let environment = Environment::default();

        let response = environment
            .get_exec_backend()
            .start(crate::ExecParams {
                process_id: ProcessId::from("default-env-proc"),
                argv: vec!["true".to_string()],
                cwd: std::env::current_dir().expect("read current dir"),
                env_policy: None,
                env: Default::default(),
                tty: false,
                pipe_stdin: false,
                arg0: None,
            })
            .await
            .expect("start process");

        assert_eq!(response.process.process_id().as_str(), "default-env-proc");
    }
}
