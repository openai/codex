use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::OnceCell;

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
const LOCAL_ENVIRONMENT_ID: &str = "local";
const REMOTE_ENVIRONMENT_ID: &str = "remote";
type EnvironmentId = String;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnvironmentConfig {
    id: EnvironmentId,
    exec_server_url: Option<String>,
}

impl EnvironmentConfig {
    pub fn id(&self) -> &str {
        self.id.as_str()
    }

    pub fn exec_server_url(&self) -> Option<&str> {
        self.exec_server_url.as_deref()
    }

    pub fn is_remote(&self) -> bool {
        self.exec_server_url.is_some()
    }
}

/// Registry of environments available to the process.
///
/// The manager owns environment metadata and lazy environment construction. A
/// thread or turn carries its selected environment id separately.
#[derive(Debug)]
pub struct EnvironmentManager {
    default_environment_id: Option<EnvironmentId>,
    environment_configs: HashMap<EnvironmentId, EnvironmentConfig>,
    environment_cache: HashMap<EnvironmentId, Arc<OnceCell<Arc<Environment>>>>,
    local_runtime_paths: Option<ExecServerRuntimePaths>,
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
        let (default_environment_id, environment_configs) =
            bootstrap_environment_set(exec_server_url);
        Self {
            default_environment_id,
            environment_cache: build_environment_cache(&environment_configs),
            environment_configs,
            local_runtime_paths,
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

    pub fn default_config(&self) -> Option<&EnvironmentConfig> {
        self.default_environment_id
            .as_ref()
            .and_then(|environment_id| self.environment_configs.get(environment_id))
    }

    fn is_disabled(&self) -> bool {
        self.default_environment_id.is_none() && self.environment_configs.is_empty()
    }

    /// Returns the cached default environment, creating it on first access.
    pub async fn default_environment(&self) -> Result<Option<Arc<Environment>>, ExecServerError> {
        match self.default_environment_id.as_deref() {
            Some(environment_id) => self.environment_by_id(environment_id).await.map(Some),
            None => Ok(None),
        }
    }

    pub async fn environment(
        &self,
        environment_id: Option<&str>,
    ) -> Result<Option<Arc<Environment>>, ExecServerError> {
        match normalize_environment_id(environment_id) {
            None => self.default_environment().await,
            Some(environment_id) => self.environment_by_id(&environment_id).await.map(Some),
        }
    }

    /// Validates that the referenced environment id is configured without
    /// instantiating the underlying environment or connecting any remote
    /// client.
    pub fn validate_environment(
        &self,
        environment_id: Option<&str>,
    ) -> Result<(), ExecServerError> {
        let Some(environment_id) = normalize_environment_id(environment_id) else {
            return Ok(());
        };
        if self.is_disabled() {
            return Err(ExecServerError::Protocol(
                "environments are disabled for this session".to_string(),
            ));
        }
        if self.environment_configs.contains_key(&environment_id) {
            Ok(())
        } else {
            Err(ExecServerError::Protocol(format!(
                "unknown environment id: {environment_id}"
            )))
        }
    }

    async fn environment_by_id(
        &self,
        environment_id: &str,
    ) -> Result<Arc<Environment>, ExecServerError> {
        if self.is_disabled() {
            return Err(ExecServerError::Protocol(
                "environments are disabled for this session".to_string(),
            ));
        }
        let Some(environment_config) = self.environment_configs.get(environment_id).cloned() else {
            return Err(ExecServerError::Protocol(format!(
                "unknown environment id: {environment_id}"
            )));
        };
        let Some(environment_cell) = self.environment_cache.get(environment_id) else {
            return Err(ExecServerError::Protocol(format!(
                "unknown environment id: {environment_id}"
            )));
        };

        environment_cell
            .get_or_try_init(|| async {
                Environment::create_with_config(
                    environment_config,
                    self.local_runtime_paths.clone(),
                )
                .await
                .map(Arc::new)
            })
            .await
            .map(Arc::clone)
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
        let id = if disabled {
            LOCAL_ENVIRONMENT_ID.to_string()
        } else if exec_server_url.is_some() {
            REMOTE_ENVIRONMENT_ID.to_string()
        } else {
            LOCAL_ENVIRONMENT_ID.to_string()
        };
        Self::create_with_config(
            EnvironmentConfig {
                id,
                exec_server_url,
            },
            local_runtime_paths,
        )
        .await
    }

    async fn create_with_config(
        environment_config: EnvironmentConfig,
        local_runtime_paths: Option<ExecServerRuntimePaths>,
    ) -> Result<Self, ExecServerError> {
        let (exec_server_url, disabled) =
            normalize_exec_server_url(environment_config.exec_server_url);
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

fn normalize_environment_id(environment_id: Option<&str>) -> Option<EnvironmentId> {
    match environment_id.map(str::trim) {
        None | Some("") => None,
        Some(environment_id) => Some(environment_id.to_ascii_lowercase()),
    }
}

/// Bootstraps the built-in environment registry from `CODEX_EXEC_SERVER_URL`.
///
/// Supported modes:
/// - unset or empty: register only `local` and make it default
/// - `none`: register nothing and leave the manager disabled
/// - websocket URL: register both `local` and `remote`, and make `remote`
///   default
///
/// The returned map is the authoritative environment registry for the manager;
/// the default environment id is just the startup selection over that map.
fn bootstrap_environment_set(
    exec_server_url: Option<String>,
) -> (
    Option<EnvironmentId>,
    HashMap<EnvironmentId, EnvironmentConfig>,
) {
    let (exec_server_url, disabled) = normalize_exec_server_url(exec_server_url);
    if disabled {
        return (None, HashMap::new());
    }

    let mut environment_configs = HashMap::from([(
        LOCAL_ENVIRONMENT_ID.to_string(),
        EnvironmentConfig {
            id: LOCAL_ENVIRONMENT_ID.to_string(),
            exec_server_url: None,
        },
    )]);
    let default_environment_id = if let Some(exec_server_url) = exec_server_url {
        environment_configs.insert(
            REMOTE_ENVIRONMENT_ID.to_string(),
            EnvironmentConfig {
                id: REMOTE_ENVIRONMENT_ID.to_string(),
                exec_server_url: Some(exec_server_url),
            },
        );
        Some(REMOTE_ENVIRONMENT_ID.to_string())
    } else {
        Some(LOCAL_ENVIRONMENT_ID.to_string())
    };

    (default_environment_id, environment_configs)
}

fn build_environment_cache(
    environment_configs: &HashMap<EnvironmentId, EnvironmentConfig>,
) -> HashMap<EnvironmentId, Arc<OnceCell<Arc<Environment>>>> {
    environment_configs
        .keys()
        .cloned()
        .map(|environment_id| (environment_id, Arc::new(OnceCell::new())))
        .collect()
}
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::Environment;
    use super::EnvironmentConfig;
    use super::EnvironmentManager;
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

        assert_eq!(
            manager.default_config().map(EnvironmentConfig::id),
            Some("local")
        );
        assert_eq!(
            manager
                .default_config()
                .and_then(EnvironmentConfig::exec_server_url),
            None
        );
        assert!(
            !manager
                .default_config()
                .is_some_and(EnvironmentConfig::is_remote)
        );
    }

    #[test]
    fn environment_manager_treats_none_value_as_disabled() {
        let manager = EnvironmentManager::new(Some("none".to_string()));

        assert_eq!(manager.default_config().map(EnvironmentConfig::id), None);
        assert_eq!(
            manager
                .default_config()
                .and_then(EnvironmentConfig::exec_server_url),
            None
        );
        assert!(
            !manager
                .default_config()
                .is_some_and(EnvironmentConfig::is_remote)
        );
    }

    #[test]
    fn environment_manager_reports_remote_url() {
        let manager = EnvironmentManager::new(Some("ws://127.0.0.1:8765".to_string()));

        assert!(
            manager
                .default_config()
                .is_some_and(EnvironmentConfig::is_remote)
        );
        assert_eq!(
            manager
                .default_config()
                .and_then(EnvironmentConfig::exec_server_url),
            Some("ws://127.0.0.1:8765")
        );
    }

    #[test]
    fn environment_manager_bootstraps_local_and_remote_entries() {
        let manager = EnvironmentManager::new(Some("ws://127.0.0.1:8765".to_string()));

        assert_eq!(
            manager.default_config().map(EnvironmentConfig::id),
            Some("remote")
        );
        assert_eq!(manager.environment_configs.len(), 2);
        assert_eq!(
            manager.environment_configs.get("local"),
            Some(&EnvironmentConfig {
                id: "local".to_string(),
                exec_server_url: None
            })
        );
        assert_eq!(
            manager.environment_configs.get("remote"),
            Some(&EnvironmentConfig {
                id: "remote".to_string(),
                exec_server_url: Some("ws://127.0.0.1:8765".to_string()),
            })
        );
    }

    #[tokio::test]
    async fn environment_manager_default_environment_caches_environment() {
        let manager = EnvironmentManager::new(/*exec_server_url*/ None);

        let first = manager
            .default_environment()
            .await
            .expect("get default environment");
        let second = manager
            .default_environment()
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
            .default_environment()
            .await
            .expect("get default environment")
            .expect("local environment");

        assert_eq!(environment.local_runtime_paths(), Some(&runtime_paths));
        assert_eq!(manager.local_runtime_paths, Some(runtime_paths));
    }

    #[tokio::test]
    async fn disabled_environment_manager_has_no_current_environment() {
        let manager = EnvironmentManager::new(Some("none".to_string()));

        assert!(
            manager
                .default_environment()
                .await
                .expect("get default environment")
                .is_none()
        );
    }

    #[tokio::test]
    async fn environment_manager_explicit_local_selection_bypasses_remote_default() {
        let manager = EnvironmentManager::new(Some("ws://127.0.0.1:8765".to_string()));

        let environment = manager
            .environment(Some("local"))
            .await
            .expect("get explicit local environment")
            .expect("local environment");

        assert!(!environment.is_remote());
        assert_eq!(environment.exec_server_url(), None);
    }

    #[tokio::test]
    async fn environment_manager_rejects_remote_selection_when_not_configured() {
        let manager = EnvironmentManager::new(/*exec_server_url*/ None);

        let err = manager
            .environment(Some("remote"))
            .await
            .expect_err("remote selection should fail");

        assert_eq!(
            err.to_string(),
            "exec-server protocol error: unknown environment id: remote"
        );
    }

    #[tokio::test]
    async fn environment_manager_rejects_explicit_selection_when_disabled() {
        let manager = EnvironmentManager::new(Some("none".to_string()));

        let err = manager
            .environment(Some("local"))
            .await
            .expect_err("explicit local selection should fail");

        assert_eq!(
            err.to_string(),
            "exec-server protocol error: environments are disabled for this session"
        );
    }

    #[tokio::test]
    async fn environment_manager_rejects_unknown_environment_id() {
        let manager = EnvironmentManager::new(/*exec_server_url*/ None);

        let err = manager
            .environment(Some("mystery"))
            .await
            .expect_err("unknown environment should fail");

        assert_eq!(
            err.to_string(),
            "exec-server protocol error: unknown environment id: mystery"
        );
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
