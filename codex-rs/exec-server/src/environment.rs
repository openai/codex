use std::collections::HashMap;
use std::sync::Arc;

use crate::ExecServerError;
use crate::ExecServerRuntimePaths;
use crate::RemoteExecServerTransport;
use crate::client::LazyRemoteExecServerClient;
use crate::file_system::ExecutorFileSystem;
use crate::local_file_system::LocalFileSystem;
use crate::local_process::LocalProcess;
use crate::process::ExecBackend;
use crate::remote_file_system::RemoteFileSystem;
use crate::remote_process::RemoteProcess;

pub const CODEX_EXEC_SERVER_URL_ENV_VAR: &str = "CODEX_EXEC_SERVER_URL";

/// Owns the execution/filesystem environments available to the Codex runtime.
///
/// `EnvironmentManager` is a shared registry for concrete environments. It
/// always creates a local environment under [`LOCAL_ENVIRONMENT_ID`]. Additional
/// environments are remote exec-server endpoints that can connect over either a
/// websocket URL or a command-backed stdio JSON-RPC transport.
///
/// In legacy mode, when `CODEX_EXEC_SERVER_URL` is set to a websocket URL, it
/// also creates a remote environment under [`REMOTE_ENVIRONMENT_ID`] and makes
/// that the default environment. Otherwise the local environment is the default.
///
/// Setting `CODEX_EXEC_SERVER_URL=none` disables environment access by leaving
/// the default environment unset while still keeping the local environment
/// available for internal callers by id. Callers use
/// `default_environment().is_some()` as the signal for model-facing
/// shell/filesystem tool availability.
///
/// Remote environments create remote filesystem and execution backends that
/// lazy-connect to the configured exec-server on first use. The websocket is
/// not opened when the manager or environment is constructed.
#[derive(Debug)]
pub struct EnvironmentManager {
    default_environment: Option<String>,
    environments: HashMap<String, Arc<Environment>>,
}

pub const LOCAL_ENVIRONMENT_ID: &str = "local";
pub const REMOTE_ENVIRONMENT_ID: &str = "remote";

#[derive(Clone, Debug)]
pub struct EnvironmentManagerArgs {
    pub exec_server_url: Option<String>,
    pub local_runtime_paths: ExecServerRuntimePaths,
}

impl EnvironmentManagerArgs {
    pub fn new(local_runtime_paths: ExecServerRuntimePaths) -> Self {
        Self {
            exec_server_url: None,
            local_runtime_paths,
        }
    }

    pub fn from_env(local_runtime_paths: ExecServerRuntimePaths) -> Self {
        Self {
            exec_server_url: std::env::var(CODEX_EXEC_SERVER_URL_ENV_VAR).ok(),
            local_runtime_paths,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ConfiguredEnvironmentManagerArgs {
    pub default_environment: Option<String>,
    pub environments: Vec<ConfiguredEnvironmentSpec>,
    pub local_runtime_paths: ExecServerRuntimePaths,
}

#[derive(Clone, Debug)]
pub struct ConfiguredEnvironmentSpec {
    pub id: String,
    pub transport: RemoteExecServerTransport,
}

#[derive(Clone, Debug)]
pub enum EnvironmentManagerConfig {
    ExecServerUrl(EnvironmentManagerArgs),
    Configured(ConfiguredEnvironmentManagerArgs),
}

impl From<EnvironmentManagerArgs> for EnvironmentManagerConfig {
    fn from(args: EnvironmentManagerArgs) -> Self {
        Self::ExecServerUrl(args)
    }
}

impl From<ConfiguredEnvironmentManagerArgs> for EnvironmentManagerConfig {
    fn from(args: ConfiguredEnvironmentManagerArgs) -> Self {
        Self::Configured(args)
    }
}

impl EnvironmentManager {
    /// Builds a test-only manager without configured sandbox helper paths.
    pub fn default_for_tests() -> Self {
        Self {
            default_environment: Some(LOCAL_ENVIRONMENT_ID.to_string()),
            environments: HashMap::from([(
                LOCAL_ENVIRONMENT_ID.to_string(),
                Arc::new(Environment::default_for_tests()),
            )]),
        }
    }

    /// Builds a manager from the raw `CODEX_EXEC_SERVER_URL` value and local
    /// runtime paths used when creating local filesystem helpers.
    pub fn new(config: impl Into<EnvironmentManagerConfig>) -> Self {
        match config.into() {
            EnvironmentManagerConfig::ExecServerUrl(args) => Self::from_exec_server_url_args(args),
            EnvironmentManagerConfig::Configured(args) => Self::from_configured_args(args)
                .unwrap_or_else(|err| panic!("invalid environment manager config: {err}")),
        }
    }

    fn from_exec_server_url_args(args: EnvironmentManagerArgs) -> Self {
        let EnvironmentManagerArgs {
            exec_server_url,
            local_runtime_paths,
        } = args;
        let (exec_server_url, environment_disabled) = normalize_exec_server_url(exec_server_url);
        let mut environments = HashMap::from([(
            LOCAL_ENVIRONMENT_ID.to_string(),
            Arc::new(Environment::local(local_runtime_paths.clone())),
        )]);
        let default_environment = if environment_disabled {
            None
        } else {
            match exec_server_url {
                Some(exec_server_url) => {
                    environments.insert(
                        REMOTE_ENVIRONMENT_ID.to_string(),
                        Arc::new(Environment::remote_with_runtime_paths(
                            RemoteExecServerTransport::WebSocket {
                                url: exec_server_url,
                            },
                            Some(local_runtime_paths),
                        )),
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

    pub fn try_new(config: impl Into<EnvironmentManagerConfig>) -> Result<Self, ExecServerError> {
        match config.into() {
            EnvironmentManagerConfig::ExecServerUrl(args) => {
                Ok(Self::from_exec_server_url_args(args))
            }
            EnvironmentManagerConfig::Configured(args) => Self::from_configured_args(args),
        }
    }

    fn from_configured_args(
        args: ConfiguredEnvironmentManagerArgs,
    ) -> Result<Self, ExecServerError> {
        let ConfiguredEnvironmentManagerArgs {
            default_environment,
            environments: configured_environments,
            local_runtime_paths,
        } = args;
        let mut environments = HashMap::from([(
            LOCAL_ENVIRONMENT_ID.to_string(),
            Arc::new(Environment::local(local_runtime_paths.clone())),
        )]);

        for environment in configured_environments {
            let id = environment.id.trim();
            if id.is_empty() {
                return Err(ExecServerError::Protocol(
                    "environment id must not be empty".to_string(),
                ));
            }
            if id == LOCAL_ENVIRONMENT_ID || id.eq_ignore_ascii_case("none") {
                return Err(ExecServerError::Protocol(format!(
                    "environment id `{id}` is reserved"
                )));
            }
            if environments.contains_key(id) {
                return Err(ExecServerError::Protocol(format!(
                    "duplicate environment id `{id}`"
                )));
            }
            environments.insert(
                id.to_string(),
                Arc::new(Environment::remote_with_runtime_paths(
                    environment.transport,
                    Some(local_runtime_paths.clone()),
                )),
            );
        }

        let default_environment = match default_environment.as_deref().map(str::trim) {
            None | Some("") => Some(LOCAL_ENVIRONMENT_ID.to_string()),
            Some(default_environment) if default_environment.eq_ignore_ascii_case("none") => None,
            Some(default_environment) => {
                if !environments.contains_key(default_environment) {
                    return Err(ExecServerError::Protocol(format!(
                        "default_environment `{default_environment}` is not a configured environment id"
                    )));
                }
                Some(default_environment.to_string())
            }
        };

        Ok(Self {
            default_environment,
            environments,
        })
    }

    /// Returns the default environment instance.
    pub fn default_environment(&self) -> Option<Arc<Environment>> {
        self.default_environment
            .as_deref()
            .and_then(|environment_id| self.get_environment(environment_id))
    }

    /// Returns the local environment instance used for internal runtime work.
    pub fn local_environment(&self) -> Arc<Environment> {
        match self.get_environment(LOCAL_ENVIRONMENT_ID) {
            Some(environment) => environment,
            None => unreachable!("EnvironmentManager always has a local environment"),
        }
    }

    /// Returns a named environment instance.
    pub fn get_environment(&self, environment_id: &str) -> Option<Arc<Environment>> {
        self.environments.get(environment_id).cloned()
    }
}

/// Concrete execution/filesystem environment selected for a session.
///
/// This bundles the selected backend metadata together with the local runtime
/// paths used by filesystem helpers.
#[derive(Clone)]
pub struct Environment {
    remote_transport: Option<RemoteExecServerTransport>,
    exec_backend: Arc<dyn ExecBackend>,
    filesystem: Arc<dyn ExecutorFileSystem>,
    local_runtime_paths: Option<ExecServerRuntimePaths>,
}

impl Environment {
    /// Builds a test-only local environment without configured sandbox helper paths.
    pub fn default_for_tests() -> Self {
        Self {
            remote_transport: None,
            exec_backend: Arc::new(LocalProcess::default()),
            filesystem: Arc::new(LocalFileSystem::unsandboxed()),
            local_runtime_paths: None,
        }
    }
}

impl std::fmt::Debug for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Environment")
            .field("remote_transport", &self.remote_transport)
            .finish_non_exhaustive()
    }
}

impl Environment {
    /// Builds an environment from the raw `CODEX_EXEC_SERVER_URL` value.
    pub fn create(
        exec_server_url: Option<String>,
        local_runtime_paths: ExecServerRuntimePaths,
    ) -> Result<Self, ExecServerError> {
        Self::create_inner(exec_server_url, Some(local_runtime_paths))
    }

    /// Builds a test-only environment without configured sandbox helper paths.
    pub fn create_for_tests(exec_server_url: Option<String>) -> Result<Self, ExecServerError> {
        Self::create_inner(exec_server_url, /*local_runtime_paths*/ None)
    }

    /// Builds an environment from the raw `CODEX_EXEC_SERVER_URL` value and
    /// local runtime paths used when creating local filesystem helpers.
    fn create_inner(
        exec_server_url: Option<String>,
        local_runtime_paths: Option<ExecServerRuntimePaths>,
    ) -> Result<Self, ExecServerError> {
        let (exec_server_url, disabled) = normalize_exec_server_url(exec_server_url);
        if disabled {
            return Err(ExecServerError::Protocol(
                "disabled mode does not create an Environment".to_string(),
            ));
        }

        Ok(match exec_server_url {
            Some(exec_server_url) => Self::remote_with_runtime_paths(
                RemoteExecServerTransport::WebSocket {
                    url: exec_server_url,
                },
                local_runtime_paths,
            ),
            None => match local_runtime_paths {
                Some(local_runtime_paths) => Self::local(local_runtime_paths),
                None => Self::default_for_tests(),
            },
        })
    }

    fn local(local_runtime_paths: ExecServerRuntimePaths) -> Self {
        Self {
            remote_transport: None,
            exec_backend: Arc::new(LocalProcess::default()),
            filesystem: Arc::new(LocalFileSystem::with_runtime_paths(
                local_runtime_paths.clone(),
            )),
            local_runtime_paths: Some(local_runtime_paths),
        }
    }

    fn remote_with_runtime_paths(
        transport: RemoteExecServerTransport,
        local_runtime_paths: Option<ExecServerRuntimePaths>,
    ) -> Self {
        let client = LazyRemoteExecServerClient::new(transport.clone());
        let exec_backend: Arc<dyn ExecBackend> = Arc::new(RemoteProcess::new(client.clone()));
        let filesystem: Arc<dyn ExecutorFileSystem> = Arc::new(RemoteFileSystem::new(client));

        Self {
            remote_transport: Some(transport),
            exec_backend,
            filesystem,
            local_runtime_paths,
        }
    }

    pub fn is_remote(&self) -> bool {
        self.remote_transport.is_some()
    }

    /// Returns the remote exec-server URL when this environment is remote.
    pub fn exec_server_url(&self) -> Option<&str> {
        match self.remote_transport.as_ref() {
            Some(RemoteExecServerTransport::WebSocket { url }) => Some(url.as_str()),
            Some(RemoteExecServerTransport::Command { .. }) | None => None,
        }
    }

    pub fn exec_server_command(&self) -> Option<&str> {
        match self.remote_transport.as_ref() {
            Some(RemoteExecServerTransport::Command { command }) => Some(command.as_str()),
            Some(RemoteExecServerTransport::WebSocket { .. }) | None => None,
        }
    }

    pub fn local_runtime_paths(&self) -> Option<&ExecServerRuntimePaths> {
        self.local_runtime_paths.as_ref()
    }

    pub fn get_exec_backend(&self) -> Arc<dyn ExecBackend> {
        Arc::clone(&self.exec_backend)
    }

    pub fn get_filesystem(&self) -> Arc<dyn ExecutorFileSystem> {
        Arc::clone(&self.filesystem)
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

    fn test_runtime_paths() -> ExecServerRuntimePaths {
        ExecServerRuntimePaths::new(
            std::env::current_exe().expect("current exe"),
            /*codex_linux_sandbox_exe*/ None,
        )
        .expect("runtime paths")
    }

    #[tokio::test]
    async fn create_local_environment_does_not_connect() {
        let environment = Environment::create(/*exec_server_url*/ None, test_runtime_paths())
            .expect("create environment");

        assert_eq!(environment.exec_server_url(), None);
        assert!(!environment.is_remote());
    }

    #[tokio::test]
    async fn environment_manager_normalizes_empty_url() {
        let manager = EnvironmentManager::new(EnvironmentManagerArgs {
            exec_server_url: Some(String::new()),
            local_runtime_paths: test_runtime_paths(),
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

    #[tokio::test]
    async fn environment_manager_treats_none_value_as_disabled() {
        let manager = EnvironmentManager::new(EnvironmentManagerArgs {
            exec_server_url: Some("none".to_string()),
            local_runtime_paths: test_runtime_paths(),
        });

        assert!(manager.default_environment().is_none());
        assert!(
            !manager
                .get_environment(LOCAL_ENVIRONMENT_ID)
                .expect("local environment")
                .is_remote()
        );
        assert!(manager.get_environment(REMOTE_ENVIRONMENT_ID).is_none());
    }

    #[tokio::test]
    async fn environment_manager_reports_remote_url() {
        let manager = EnvironmentManager::new(EnvironmentManagerArgs {
            exec_server_url: Some("ws://127.0.0.1:8765".to_string()),
            local_runtime_paths: test_runtime_paths(),
        });

        let environment = manager.default_environment().expect("default environment");
        assert!(environment.is_remote());
        assert_eq!(environment.exec_server_url(), Some("ws://127.0.0.1:8765"));
        assert!(Arc::ptr_eq(
            &environment,
            &manager
                .get_environment(REMOTE_ENVIRONMENT_ID)
                .expect("remote environment")
        ));
        assert!(
            !manager
                .get_environment(LOCAL_ENVIRONMENT_ID)
                .expect("local environment")
                .is_remote()
        );
    }

    #[tokio::test]
    async fn environment_manager_default_environment_caches_environment() {
        let manager = EnvironmentManager::default_for_tests();

        let first = manager.default_environment().expect("default environment");
        let second = manager.default_environment().expect("default environment");

        assert!(Arc::ptr_eq(&first, &second));
        assert!(Arc::ptr_eq(
            &first.get_filesystem(),
            &second.get_filesystem()
        ));
    }

    #[tokio::test]
    async fn environment_manager_carries_local_runtime_paths() {
        let runtime_paths = test_runtime_paths();
        let manager = EnvironmentManager::new(EnvironmentManagerArgs {
            exec_server_url: None,
            local_runtime_paths: runtime_paths.clone(),
        });

        let environment = manager.default_environment().expect("default environment");

        assert_eq!(environment.local_runtime_paths(), Some(&runtime_paths));
        let manager = EnvironmentManager::new(EnvironmentManagerArgs {
            exec_server_url: environment.exec_server_url().map(str::to_owned),
            local_runtime_paths: environment
                .local_runtime_paths()
                .expect("local runtime paths")
                .clone(),
        });
        let environment = manager.default_environment().expect("default environment");
        assert_eq!(environment.local_runtime_paths(), Some(&runtime_paths));
    }

    #[tokio::test]
    async fn disabled_environment_manager_has_no_default_environment() {
        let manager = EnvironmentManager::new(EnvironmentManagerArgs {
            exec_server_url: Some("none".to_string()),
            local_runtime_paths: test_runtime_paths(),
        });

        assert!(manager.default_environment().is_none());
    }

    #[tokio::test]
    async fn environment_manager_keeps_local_lookup_when_default_disabled() {
        let manager = EnvironmentManager::new(EnvironmentManagerArgs {
            exec_server_url: Some("none".to_string()),
            local_runtime_paths: test_runtime_paths(),
        });

        assert!(manager.default_environment().is_none());
        assert!(
            !manager
                .get_environment(LOCAL_ENVIRONMENT_ID)
                .expect("local environment")
                .is_remote()
        );
        assert!(manager.get_environment(REMOTE_ENVIRONMENT_ID).is_none());
    }

    #[tokio::test]
    async fn get_environment_returns_none_for_unknown_id() {
        let manager = EnvironmentManager::default_for_tests();

        assert!(manager.get_environment("does-not-exist").is_none());
    }

    #[tokio::test]
    async fn default_environment_has_ready_local_executor() {
        let environment = Environment::default_for_tests();

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

    #[tokio::test]
    async fn test_environment_rejects_sandboxed_filesystem_without_runtime_paths() {
        let environment = Environment::default_for_tests();
        let path = codex_utils_absolute_path::AbsolutePathBuf::from_absolute_path(
            std::env::current_exe().expect("current exe").as_path(),
        )
        .expect("absolute current exe");
        let sandbox = crate::FileSystemSandboxContext::new(
            codex_protocol::protocol::SandboxPolicy::new_read_only_policy(),
        );

        let err = environment
            .get_filesystem()
            .read_file(&path, Some(&sandbox))
            .await
            .expect_err("sandboxed read should require runtime paths");

        assert_eq!(
            err.to_string(),
            "sandboxed filesystem operations require configured runtime paths"
        );
    }
}
