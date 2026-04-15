use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::OnceCell;
use tokio::sync::RwLock;

use crate::ExecServerClient;
use crate::ExecServerError;
use crate::ExecServerRuntimePaths;
use crate::RemoteExecServerConnectArgs;
use crate::file_system::ExecutorFileSystem;
use crate::local_file_system::LocalFileSystem;
use crate::local_process::LocalProcess;
use crate::process::ExecBackend;
use crate::remote_file_system::RemoteFileSystem;
use crate::remote_process::RemoteProcess;

pub const CODEX_EXEC_SERVER_URL_ENV_VAR: &str = "CODEX_EXEC_SERVER_URL";

pub type EnvironmentId = String;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EnvironmentConfig {
    pub exec_server_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisteredEnvironment {
    pub environment_id: EnvironmentId,
    pub config: EnvironmentConfig,
}

/// Lazily creates and caches the active environment for a session.
///
/// The manager keeps the session's environment selection stable so subagents
/// and follow-up turns preserve an explicit disabled state.
#[derive(Debug)]
pub struct EnvironmentManager {
    default_environment_config: EnvironmentConfig,
    local_runtime_paths: Option<ExecServerRuntimePaths>,
    default_disabled: bool,
    default_environment: OnceCell<Option<Arc<Environment>>>,
    environment_configs: RwLock<HashMap<EnvironmentId, EnvironmentConfig>>,
    environment_cache: RwLock<HashMap<EnvironmentId, Arc<OnceCell<Option<Arc<Environment>>>>>>,
}

impl Default for EnvironmentManager {
    fn default() -> Self {
        Self::new(/*exec_server_url*/ None)
    }
}

impl EnvironmentManager {
    /// Builds a manager from the raw `CODEX_EXEC_SERVER_URL` value.
    pub fn new(exec_server_url: Option<String>) -> Self {
        Self::new_with_runtime_paths(exec_server_url, /*local_runtime_paths*/ None)
    }

    /// Builds a manager from the raw `CODEX_EXEC_SERVER_URL` value and local
    /// runtime paths used when creating local filesystem helpers.
    pub fn new_with_runtime_paths(
        exec_server_url: Option<String>,
        local_runtime_paths: Option<ExecServerRuntimePaths>,
    ) -> Self {
        let (exec_server_url, default_disabled) = normalize_exec_server_url(exec_server_url);
        Self {
            default_environment_config: EnvironmentConfig { exec_server_url },
            local_runtime_paths,
            default_disabled,
            default_environment: OnceCell::new(),
            environment_configs: RwLock::new(HashMap::new()),
            environment_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Builds a manager from process environment variables.
    pub fn from_env() -> Self {
        Self::from_env_with_runtime_paths(/*local_runtime_paths*/ None)
    }

    /// Builds a manager from process environment variables and local runtime
    /// paths used when creating local filesystem helpers.
    pub fn from_env_with_runtime_paths(
        local_runtime_paths: Option<ExecServerRuntimePaths>,
    ) -> Self {
        Self::new_with_runtime_paths(
            std::env::var(CODEX_EXEC_SERVER_URL_ENV_VAR).ok(),
            local_runtime_paths,
        )
    }

    /// Builds a manager from the currently selected environment, or from the
    /// disabled mode when no environment is available.
    pub fn from_environment(environment: Option<&Environment>) -> Self {
        match environment {
            Some(environment) => Self::new_with_runtime_paths(
                environment.exec_server_url().map(str::to_owned),
                environment.local_runtime_paths().cloned(),
            ),
            None => Self {
                default_environment_config: EnvironmentConfig::default(),
                local_runtime_paths: None,
                default_disabled: true,
                default_environment: OnceCell::new(),
                environment_configs: RwLock::new(HashMap::new()),
                environment_cache: RwLock::new(HashMap::new()),
            },
        }
    }

    /// Returns the default remote exec-server URL when one is configured.
    pub fn exec_server_url(&self) -> Option<&str> {
        self.default_environment_config.exec_server_url.as_deref()
    }

    /// Returns true when the default environment is configured to use a remote exec server.
    pub fn is_remote(&self) -> bool {
        self.default_environment_config.exec_server_url.is_some()
    }

    pub async fn register_environment(
        &self,
        environment_id: EnvironmentId,
        config: EnvironmentConfig,
    ) -> Result<(), ExecServerError> {
        let environment_id = normalize_registered_environment_id(environment_id)?;
        let (exec_server_url, disabled) = normalize_exec_server_url(config.exec_server_url);
        if disabled {
            return Err(ExecServerError::Protocol(
                "named environments cannot use the reserved disabled value 'none'".to_string(),
            ));
        }

        let config = EnvironmentConfig { exec_server_url };
        self.environment_configs
            .write()
            .await
            .insert(environment_id.clone(), config);
        self.environment_cache
            .write()
            .await
            .insert(environment_id, Arc::new(OnceCell::new()));
        Ok(())
    }

    pub async fn list_environments(
        &self,
        cursor: Option<&str>,
        limit: Option<u32>,
    ) -> (Vec<RegisteredEnvironment>, Option<String>) {
        let cursor = normalize_environment_id(cursor).map(str::to_owned);
        let limit = usize::try_from(limit.unwrap_or(100)).unwrap_or(100);
        let configs = self.environment_configs.read().await;
        let mut environment_ids = configs.keys().cloned().collect::<Vec<_>>();
        environment_ids.sort();

        let start_index = cursor
            .as_ref()
            .and_then(|cursor| environment_ids.iter().position(|id| id == cursor))
            .map_or(0, |index| index.saturating_add(1));

        let selected_ids = environment_ids
            .into_iter()
            .skip(start_index)
            .take(limit.saturating_add(1))
            .collect::<Vec<_>>();
        let has_more = selected_ids.len() > limit;
        let next_cursor = if has_more {
            selected_ids.get(limit.saturating_sub(1)).cloned()
        } else {
            None
        };

        let data = selected_ids
            .into_iter()
            .take(limit)
            .filter_map(|environment_id| {
                configs
                    .get(&environment_id)
                    .cloned()
                    .map(|config| RegisteredEnvironment {
                        environment_id,
                        config,
                    })
            })
            .collect();

        (data, next_cursor)
    }

    /// Returns the cached environment, creating it on first access.
    pub async fn environment(
        &self,
        environment_id: Option<&str>,
    ) -> Result<Option<Arc<Environment>>, ExecServerError> {
        match normalize_environment_id(environment_id) {
            None => self.default_environment().await,
            Some(environment_id) => self.named_environment(environment_id).await,
        }
    }

    async fn default_environment(&self) -> Result<Option<Arc<Environment>>, ExecServerError> {
        self.default_environment
            .get_or_try_init(|| async {
                if self.default_disabled {
                    Ok(None)
                } else {
                    self.build_environment(&self.default_environment_config)
                        .await
                }
            })
            .await
            .map(Option::as_ref)
            .map(std::option::Option::<&Arc<Environment>>::cloned)
    }

    async fn named_environment(
        &self,
        environment_id: &str,
    ) -> Result<Option<Arc<Environment>>, ExecServerError> {
        let config = self
            .environment_configs
            .read()
            .await
            .get(environment_id)
            .cloned()
            .ok_or_else(|| {
                ExecServerError::Protocol(format!("unknown environment id: {environment_id}"))
            })?;
        let environment_cell = {
            let mut environment_cache = self.environment_cache.write().await;
            Arc::clone(
                environment_cache
                    .entry(environment_id.to_string())
                    .or_insert_with(|| Arc::new(OnceCell::new())),
            )
        };

        environment_cell
            .get_or_try_init(|| async { self.build_environment(&config).await })
            .await
            .map(Option::as_ref)
            .map(std::option::Option::<&Arc<Environment>>::cloned)
    }

    async fn build_environment(
        &self,
        config: &EnvironmentConfig,
    ) -> Result<Option<Arc<Environment>>, ExecServerError> {
        Ok(Some(Arc::new(
            Environment::create_with_runtime_paths(
                config.exec_server_url.clone(),
                self.local_runtime_paths.clone(),
            )
            .await?,
        )))
    }
}

/// Concrete execution/filesystem environment selected for a session.
///
/// This bundles the selected backend together with the corresponding remote
/// client, if any.
#[derive(Clone)]
pub struct Environment {
    exec_server_url: Option<String>,
    remote_exec_server_client: Option<ExecServerClient>,
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
    pub async fn create(exec_server_url: Option<String>) -> Result<Self, ExecServerError> {
        Self::create_with_runtime_paths(exec_server_url, /*local_runtime_paths*/ None).await
    }

    /// Builds an environment from the raw `CODEX_EXEC_SERVER_URL` value and
    /// local runtime paths used when creating local filesystem helpers.
    pub async fn create_with_runtime_paths(
        exec_server_url: Option<String>,
        local_runtime_paths: Option<ExecServerRuntimePaths>,
    ) -> Result<Self, ExecServerError> {
        let (exec_server_url, disabled) = normalize_exec_server_url(exec_server_url);
        if disabled {
            return Err(ExecServerError::Protocol(
                "disabled mode does not create an Environment".to_string(),
            ));
        }

        let remote_exec_server_client = if let Some(exec_server_url) = &exec_server_url {
            Some(
                ExecServerClient::connect_websocket(RemoteExecServerConnectArgs {
                    websocket_url: exec_server_url.clone(),
                    client_name: "codex-environment".to_string(),
                    connect_timeout: std::time::Duration::from_secs(5),
                    initialize_timeout: std::time::Duration::from_secs(5),
                    resume_session_id: None,
                })
                .await?,
            )
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

fn normalize_environment_id(environment_id: Option<&str>) -> Option<&str> {
    match environment_id.map(str::trim) {
        None | Some("") => None,
        Some(environment_id) if environment_id.eq_ignore_ascii_case("default") => None,
        Some(environment_id) => Some(environment_id),
    }
}

fn normalize_registered_environment_id(
    environment_id: String,
) -> Result<EnvironmentId, ExecServerError> {
    match normalize_environment_id(Some(&environment_id)) {
        None => Err(ExecServerError::Protocol(
            "environment id is reserved for the default environment".to_string(),
        )),
        Some(environment_id) => Ok(environment_id.to_string()),
    }
}
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::Environment;
    use super::EnvironmentConfig;
    use super::EnvironmentManager;
    use super::RegisteredEnvironment;
    use crate::ExecServerRuntimePaths;
    use crate::ProcessId;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn create_local_environment_does_not_connect() {
        let environment = Environment::create(/*exec_server_url*/ None)
            .await
            .expect("create environment");

        assert_eq!(environment.exec_server_url(), None);
        assert!(environment.remote_exec_server_client.is_none());
    }

    #[test]
    fn environment_manager_normalizes_empty_url() {
        let manager = EnvironmentManager::new(Some(String::new()));

        assert_eq!(manager.exec_server_url(), None);
        assert!(!manager.is_remote());
    }

    #[test]
    fn environment_manager_treats_none_value_as_disabled() {
        let manager = EnvironmentManager::new(Some("none".to_string()));

        assert_eq!(manager.exec_server_url(), None);
        assert!(!manager.is_remote());
    }

    #[test]
    fn environment_manager_reports_remote_url() {
        let manager = EnvironmentManager::new(Some("ws://127.0.0.1:8765".to_string()));

        assert!(manager.is_remote());
        assert_eq!(manager.exec_server_url(), Some("ws://127.0.0.1:8765"));
    }

    #[tokio::test]
    async fn environment_manager_default_environment_caches_environment() {
        let manager = EnvironmentManager::new(/*exec_server_url*/ None);

        let first = manager
            .environment(None)
            .await
            .expect("get default environment");
        let second = manager
            .environment(None)
            .await
            .expect("get default environment");

        let first = first.expect("local environment");
        let second = second.expect("local environment");

        assert!(Arc::ptr_eq(&first, &second));
    }

    #[tokio::test]
    async fn environment_manager_carries_local_runtime_paths() {
        let runtime_paths = ExecServerRuntimePaths::new(
            std::env::current_exe().expect("current exe"),
            /*codex_linux_sandbox_exe*/ None,
        )
        .expect("runtime paths");
        let manager = EnvironmentManager::new_with_runtime_paths(
            /*exec_server_url*/ None,
            Some(runtime_paths.clone()),
        );

        let environment = manager
            .environment(None)
            .await
            .expect("get default environment")
            .expect("local environment");

        assert_eq!(environment.local_runtime_paths(), Some(&runtime_paths));
        assert_eq!(
            EnvironmentManager::from_environment(Some(&environment)).local_runtime_paths,
            Some(runtime_paths)
        );
    }

    #[tokio::test]
    async fn disabled_environment_manager_has_no_default_environment() {
        let manager = EnvironmentManager::new(Some("none".to_string()));

        assert!(
            manager
                .environment(None)
                .await
                .expect("get default environment")
                .is_none()
        );
    }

    #[tokio::test]
    async fn environment_manager_named_environment_caches_environment() {
        let manager = EnvironmentManager::new(/*exec_server_url*/ None);
        manager
            .register_environment(
                "dev".to_string(),
                EnvironmentConfig {
                    exec_server_url: None,
                },
            )
            .await
            .expect("register environment");

        let first = manager
            .environment(Some("dev"))
            .await
            .expect("get named environment")
            .expect("local environment");
        let second = manager
            .environment(Some("dev"))
            .await
            .expect("get named environment")
            .expect("local environment");

        assert!(Arc::ptr_eq(&first, &second));
    }

    #[tokio::test]
    async fn environment_manager_rejects_unknown_environment_id() {
        let manager = EnvironmentManager::new(/*exec_server_url*/ None);

        let error = manager
            .environment(Some("missing"))
            .await
            .expect_err("unknown environment id should error");

        assert_eq!(
            error.to_string(),
            "exec-server protocol error: unknown environment id: missing"
        );
    }

    #[tokio::test]
    async fn environment_manager_treats_default_environment_id_as_default() {
        let manager = EnvironmentManager::new(/*exec_server_url*/ None);

        let first = manager
            .environment(None)
            .await
            .expect("get default environment")
            .expect("local environment");
        let second = manager
            .environment(Some("default"))
            .await
            .expect("get default environment")
            .expect("local environment");

        assert!(Arc::ptr_eq(&first, &second));
    }

    #[tokio::test]
    async fn environment_manager_rejects_registering_default_environment_id() {
        let manager = EnvironmentManager::new(/*exec_server_url*/ None);

        let error = manager
            .register_environment(
                "default".to_string(),
                EnvironmentConfig {
                    exec_server_url: None,
                },
            )
            .await
            .expect_err("default id should be reserved");

        assert_eq!(
            error.to_string(),
            "exec-server protocol error: environment id is reserved for the default environment"
        );
    }

    #[tokio::test]
    async fn environment_manager_rejects_registering_disabled_named_environment() {
        let manager = EnvironmentManager::new(/*exec_server_url*/ None);

        let error = manager
            .register_environment(
                "disabled".to_string(),
                EnvironmentConfig {
                    exec_server_url: Some("none".to_string()),
                },
            )
            .await
            .expect_err("named disabled environment should be rejected");

        assert_eq!(
            error.to_string(),
            "exec-server protocol error: named environments cannot use the reserved disabled value 'none'"
        );
    }

    #[tokio::test]
    async fn environment_manager_lists_named_environments_sorted_with_pagination() {
        let manager = EnvironmentManager::new(/*exec_server_url*/ None);
        manager
            .register_environment(
                "beta".to_string(),
                EnvironmentConfig {
                    exec_server_url: Some("ws://127.0.0.1:8765".to_string()),
                },
            )
            .await
            .expect("register beta environment");
        manager
            .register_environment(
                "alpha".to_string(),
                EnvironmentConfig {
                    exec_server_url: None,
                },
            )
            .await
            .expect("register alpha environment");
        manager
            .register_environment(
                "gamma".to_string(),
                EnvironmentConfig {
                    exec_server_url: None,
                },
            )
            .await
            .expect("register gamma environment");

        let (first_page, next_cursor) = manager.list_environments(/*cursor*/ None, Some(2)).await;
        assert_eq!(
            first_page,
            vec![
                RegisteredEnvironment {
                    environment_id: "alpha".to_string(),
                    config: EnvironmentConfig {
                        exec_server_url: None,
                    },
                },
                RegisteredEnvironment {
                    environment_id: "beta".to_string(),
                    config: EnvironmentConfig {
                        exec_server_url: Some("ws://127.0.0.1:8765".to_string()),
                    },
                },
            ]
        );
        assert_eq!(next_cursor, Some("beta".to_string()));

        let (second_page, next_cursor) = manager.list_environments(Some("beta"), Some(2)).await;
        assert_eq!(
            second_page,
            vec![RegisteredEnvironment {
                environment_id: "gamma".to_string(),
                config: EnvironmentConfig {
                    exec_server_url: None,
                },
            }]
        );
        assert_eq!(next_cursor, None);
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
                arg0: None,
            })
            .await
            .expect("start process");

        assert_eq!(response.process.process_id().as_str(), "default-env-proc");
    }
}
