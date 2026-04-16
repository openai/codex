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

pub type EnvironmentId = String;

const LOCAL_ENVIRONMENT_ID: &str = "local";
const REMOTE_ENVIRONMENT_ID: &str = "remote";

/// Configuration for a named environment registration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnvironmentConfig {
    exec_server_url: Option<String>,
}

/// Produces the named environment registrations available to an
/// `EnvironmentManager`.
///
/// Implementations own the policy for which environment IDs exist and which
/// registered environment ID, if any, is used when callers request the
/// compatibility default via `environment(None)`.
pub trait EnvironmentProvider: Send + Sync + std::fmt::Debug {
    fn default_environment_id(&self) -> Option<EnvironmentId>;

    fn environment_configs(&self) -> HashMap<EnvironmentId, EnvironmentConfig>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExecServerUrlEnvironmentProvider {
    exec_server_url: Option<String>,
}

impl ExecServerUrlEnvironmentProvider {
    fn new(exec_server_url: Option<String>) -> Self {
        Self { exec_server_url }
    }

    fn from_env() -> Self {
        Self::new(std::env::var(CODEX_EXEC_SERVER_URL_ENV_VAR).ok())
    }
}

impl EnvironmentProvider for ExecServerUrlEnvironmentProvider {
    fn default_environment_id(&self) -> Option<EnvironmentId> {
        match normalize_exec_server_url(self.exec_server_url.clone()) {
            NormalizedExecServerUrl::Disabled => None,
            NormalizedExecServerUrl::LocalOnly => Some(LOCAL_ENVIRONMENT_ID.to_string()),
            NormalizedExecServerUrl::LocalAndRemote(_) => Some(REMOTE_ENVIRONMENT_ID.to_string()),
        }
    }

    fn environment_configs(&self) -> HashMap<EnvironmentId, EnvironmentConfig> {
        match normalize_exec_server_url(self.exec_server_url.clone()) {
            NormalizedExecServerUrl::Disabled => HashMap::new(),
            NormalizedExecServerUrl::LocalOnly => HashMap::from([(
                LOCAL_ENVIRONMENT_ID.to_string(),
                EnvironmentConfig {
                    exec_server_url: None,
                },
            )]),
            NormalizedExecServerUrl::LocalAndRemote(exec_server_url) => HashMap::from([
                (
                    LOCAL_ENVIRONMENT_ID.to_string(),
                    EnvironmentConfig {
                        exec_server_url: None,
                    },
                ),
                (
                    REMOTE_ENVIRONMENT_ID.to_string(),
                    EnvironmentConfig {
                        exec_server_url: Some(exec_server_url),
                    },
                ),
            ]),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum NormalizedExecServerUrl {
    LocalOnly,
    Disabled,
    LocalAndRemote(String),
}

/// Lazily creates and caches the active environment for a session.
///
/// The manager keeps the session's environment selection stable so subagents
/// and follow-up turns preserve an explicit disabled state.
#[derive(Debug)]
pub struct EnvironmentManager {
    default_environment_id: Option<EnvironmentId>,
    environment_configs: HashMap<EnvironmentId, EnvironmentConfig>,
    environment_cache: HashMap<EnvironmentId, Arc<OnceCell<Option<Arc<Environment>>>>>,
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
        Self::new_with_provider(
            ExecServerUrlEnvironmentProvider::new(exec_server_url),
            local_runtime_paths,
        )
    }

    /// Builds a manager from a provider that supplies named environment
    /// registrations plus the compatibility default selection.
    pub fn new_with_provider(
        provider: impl EnvironmentProvider,
        local_runtime_paths: Option<ExecServerRuntimePaths>,
    ) -> Self {
        let default_environment_id = provider.default_environment_id();
        let environment_configs = provider.environment_configs();
        let environment_cache = environment_configs
            .keys()
            .cloned()
            .map(|environment_id| (environment_id, Arc::new(OnceCell::new())))
            .collect();
        Self {
            default_environment_id,
            environment_configs,
            environment_cache,
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
        Self::new_with_provider(
            ExecServerUrlEnvironmentProvider::from_env(),
            local_runtime_paths,
        )
    }

    /// Builds a manager from the currently selected environment, or from the
    /// disabled mode when no environment is available.
    pub fn from_environment(environment: Option<&Environment>) -> Self {
        match environment {
            Some(environment) => {
                let mut environment_configs = HashMap::from([(
                    LOCAL_ENVIRONMENT_ID.to_string(),
                    EnvironmentConfig {
                        exec_server_url: None,
                    },
                )]);
                if let Some(exec_server_url) = environment.exec_server_url().map(str::to_owned) {
                    environment_configs.insert(
                        REMOTE_ENVIRONMENT_ID.to_string(),
                        EnvironmentConfig {
                            exec_server_url: Some(exec_server_url),
                        },
                    );
                }
                let environment_cache = environment_configs
                    .keys()
                    .cloned()
                    .map(|environment_id| (environment_id, Arc::new(OnceCell::new())))
                    .collect();
                Self {
                    default_environment_id: Some(
                        if environment.is_remote() {
                            REMOTE_ENVIRONMENT_ID
                        } else {
                            LOCAL_ENVIRONMENT_ID
                        }
                        .to_string(),
                    ),
                    environment_configs,
                    environment_cache,
                    local_runtime_paths: environment.local_runtime_paths().cloned(),
                }
            }
            None => Self {
                default_environment_id: None,
                environment_configs: HashMap::new(),
                environment_cache: HashMap::new(),
                local_runtime_paths: None,
            },
        }
    }

    /// Returns the default remote exec-server URL when one is configured.
    pub fn exec_server_url(&self) -> Option<&str> {
        self.environment_configs
            .get(REMOTE_ENVIRONMENT_ID)
            .and_then(|config| config.exec_server_url.as_deref())
    }

    /// Returns true when the default environment is configured to use a remote exec server.
    pub fn is_remote(&self) -> bool {
        self.exec_server_url().is_some()
    }

    /// Returns the cached environment, creating it on first access.
    pub async fn environment(
        &self,
        environment_id: Option<&str>,
    ) -> Result<Option<Arc<Environment>>, ExecServerError> {
        let Some(environment_id) = normalized_environment_id(environment_id)
            .or_else(|| self.default_environment_id.clone())
        else {
            return Ok(None);
        };

        self.named_environment(&environment_id).await
    }

    async fn named_environment(
        &self,
        environment_id: &str,
    ) -> Result<Option<Arc<Environment>>, ExecServerError> {
        let Some(environment_config) = self.environment_configs.get(environment_id) else {
            return Err(ExecServerError::Protocol(format!(
                "unknown environment id: {environment_id}"
            )));
        };
        let cache = self.environment_cache.get(environment_id).ok_or_else(|| {
            ExecServerError::Protocol(format!("missing environment cache: {environment_id}"))
        })?;
        cache
            .get_or_try_init(|| async {
                self.build_environment(environment_config.exec_server_url.clone())
                    .await
                    .map(Some)
            })
            .await
            .map(Option::as_ref)
            .map(std::option::Option::<&Arc<Environment>>::cloned)
    }

    async fn build_environment(
        &self,
        exec_server_url: Option<String>,
    ) -> Result<Arc<Environment>, ExecServerError> {
        Ok(Arc::new(
            Environment::create_with_runtime_paths(
                exec_server_url,
                self.local_runtime_paths.clone(),
            )
            .await?,
        ))
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

fn normalize_exec_server_url(exec_server_url: Option<String>) -> NormalizedExecServerUrl {
    match exec_server_url.as_deref().map(str::trim) {
        None | Some("") => NormalizedExecServerUrl::LocalOnly,
        Some(url) if url.eq_ignore_ascii_case("none") => NormalizedExecServerUrl::Disabled,
        Some(url) => NormalizedExecServerUrl::LocalAndRemote(url.to_string()),
    }
}

fn normalized_environment_id(environment_id: Option<&str>) -> Option<EnvironmentId> {
    match environment_id.map(str::trim) {
        None | Some("") => None,
        Some(environment_id) => Some(environment_id.to_ascii_lowercase()),
    }
}
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::Environment;
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
    async fn disabled_environment_manager_has_no_local_registration() {
        let manager = EnvironmentManager::new(Some("none".to_string()));

        let error = manager
            .environment(Some("local"))
            .await
            .expect_err("disabled mode should not register local");

        assert_eq!(
            error.to_string(),
            "exec-server protocol error: unknown environment id: local"
        );
    }

    #[tokio::test]
    async fn environment_manager_defaults_to_local_when_unset() {
        let manager = EnvironmentManager::new(/*exec_server_url*/ None);

        let environment = manager
            .environment(None)
            .await
            .expect("get default environment")
            .expect("local environment");

        assert!(!environment.is_remote());
    }

    #[tokio::test]
    async fn local_environment_caches_environment() {
        let manager = EnvironmentManager::new(/*exec_server_url*/ None);

        let first = manager
            .environment(Some("local"))
            .await
            .expect("get local environment")
            .expect("local environment");
        let second = manager
            .environment(Some("local"))
            .await
            .expect("get local environment")
            .expect("local environment");

        assert!(Arc::ptr_eq(&first, &second));
    }

    #[tokio::test]
    async fn remote_environment_caches_environment() {
        let manager = EnvironmentManager::new(Some("ws://127.0.0.1:8765".to_string()));

        let first = manager
            .environment(Some("remote"))
            .await
            .expect("get remote environment")
            .expect("remote environment");
        let second = manager
            .environment(Some("remote"))
            .await
            .expect("get remote environment")
            .expect("remote environment");

        assert!(first.is_remote());
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[tokio::test]
    async fn remote_environment_requires_registration() {
        let manager = EnvironmentManager::new(/*exec_server_url*/ None);

        let error = manager
            .environment(Some("remote"))
            .await
            .expect_err("remote environment should require registration");

        assert_eq!(
            error.to_string(),
            "exec-server protocol error: unknown environment id: remote"
        );
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
    async fn explicit_local_environment_matches_default_when_unset() {
        let manager = EnvironmentManager::new(/*exec_server_url*/ None);

        let first = manager
            .environment(None)
            .await
            .expect("get default environment")
            .expect("local environment");
        let second = manager
            .environment(Some("local"))
            .await
            .expect("get local environment")
            .expect("local environment");

        assert!(Arc::ptr_eq(&first, &second));
    }

    #[tokio::test]
    async fn configured_remote_environment_matches_default() {
        let manager = EnvironmentManager::new(Some("ws://127.0.0.1:8765".to_string()));

        let first = manager
            .environment(None)
            .await
            .expect("get default environment")
            .expect("remote environment");
        let second = manager
            .environment(Some("remote"))
            .await
            .expect("get remote environment")
            .expect("remote environment");

        assert!(first.is_remote());
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[tokio::test]
    async fn environment_manager_from_none_environment_preserves_disabled_default() {
        let manager = EnvironmentManager::from_environment(None);

        assert!(
            manager
                .environment(None)
                .await
                .expect("get default environment")
                .is_none()
        );
    }

    #[tokio::test]
    async fn from_remote_environment_preserves_remote_default() {
        let source_manager = EnvironmentManager::new(Some("ws://127.0.0.1:8765".to_string()));
        let environment = source_manager
            .environment(Some("remote"))
            .await
            .expect("get remote environment")
            .expect("remote environment");
        let manager = EnvironmentManager::from_environment(Some(&environment));

        let default_environment = manager
            .environment(None)
            .await
            .expect("get default environment")
            .expect("default environment");

        assert!(default_environment.is_remote());
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
