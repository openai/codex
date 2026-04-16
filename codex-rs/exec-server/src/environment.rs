use std::sync::Arc;

use async_trait::async_trait;
use codex_login::AuthManager;
use codex_login::AuthManagerConfig;
use tokio::sync::OnceCell;

use crate::CODEX_CLOUD_ENVIRONMENT_ID_ENV_VAR;
use crate::CODEX_CLOUD_ENVIRONMENTS_BASE_URL_ENV_VAR;
use crate::CloudEnvironmentClient;
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

/// Resolves the execution/filesystem environment for a session.
///
/// Implementations own the selection-specific details, such as whether a remote
/// websocket URL is used directly or resolved from cloud environment metadata.
#[async_trait]
pub trait EnvironmentResolver: std::fmt::Debug + Send + Sync {
    fn exec_server_url(&self) -> Option<&str>;
    fn is_remote(&self) -> bool;
    async fn create_environment(&self) -> Result<Option<Environment>, ExecServerError>;
}

/// Resolver for the existing direct/local/disabled behavior.
#[derive(Debug)]
pub struct DefaultEnvironmentResolver {
    exec_server_url: Option<String>,
    local_runtime_paths: Option<ExecServerRuntimePaths>,
    disabled: bool,
}

impl DefaultEnvironmentResolver {
    pub fn new(
        exec_server_url: Option<String>,
        local_runtime_paths: Option<ExecServerRuntimePaths>,
    ) -> Self {
        let (exec_server_url, disabled) = normalize_exec_server_url(exec_server_url);
        Self {
            exec_server_url,
            local_runtime_paths,
            disabled,
        }
    }

    pub fn disabled() -> Self {
        Self {
            exec_server_url: None,
            local_runtime_paths: None,
            disabled: true,
        }
    }
}

#[async_trait]
impl EnvironmentResolver for DefaultEnvironmentResolver {
    fn exec_server_url(&self) -> Option<&str> {
        self.exec_server_url.as_deref()
    }

    fn is_remote(&self) -> bool {
        self.exec_server_url.is_some()
    }

    async fn create_environment(&self) -> Result<Option<Environment>, ExecServerError> {
        if self.disabled {
            return Ok(None);
        }

        Ok(Some(
            Environment::create_with_runtime_paths(
                self.exec_server_url.clone(),
                /*cloud_environment_id*/ None,
                /*cloud_environments_base_url*/ None,
                /*auth_manager*/ None,
                self.local_runtime_paths.clone(),
            )
            .await?,
        ))
    }
}

/// Resolver for cloud environment id selection.
pub struct CloudEnvironmentResolver {
    cloud_environment_id: String,
    cloud_environments_base_url: Option<String>,
    auth_manager: Option<Arc<AuthManager>>,
    local_runtime_paths: Option<ExecServerRuntimePaths>,
}

impl CloudEnvironmentResolver {
    pub fn new(
        cloud_environment_id: String,
        cloud_environments_base_url: Option<String>,
        auth_manager: Option<Arc<AuthManager>>,
        local_runtime_paths: Option<ExecServerRuntimePaths>,
    ) -> Self {
        Self {
            cloud_environment_id,
            cloud_environments_base_url: normalize_optional_env_value(cloud_environments_base_url),
            auth_manager,
            local_runtime_paths,
        }
    }

    pub fn cloud_environment_id(&self) -> &str {
        &self.cloud_environment_id
    }

    pub fn cloud_environments_base_url(&self) -> Option<&str> {
        self.cloud_environments_base_url.as_deref()
    }
}

impl std::fmt::Debug for CloudEnvironmentResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudEnvironmentResolver")
            .field("cloud_environment_id", &self.cloud_environment_id)
            .field(
                "cloud_environments_base_url",
                &self.cloud_environments_base_url,
            )
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl EnvironmentResolver for CloudEnvironmentResolver {
    fn exec_server_url(&self) -> Option<&str> {
        None
    }

    fn is_remote(&self) -> bool {
        true
    }

    async fn create_environment(&self) -> Result<Option<Environment>, ExecServerError> {
        Ok(Some(
            Environment::create_with_runtime_paths(
                /*exec_server_url*/ None,
                Some(self.cloud_environment_id.clone()),
                self.cloud_environments_base_url.clone(),
                self.auth_manager.clone(),
                self.local_runtime_paths.clone(),
            )
            .await?,
        ))
    }
}

/// Lazily creates and caches the active environment for a session.
///
/// The manager keeps the session's environment selection stable so subagents
/// and follow-up turns preserve an explicit disabled state.
pub struct EnvironmentManager {
    resolver: Box<dyn EnvironmentResolver>,
    current_environment: OnceCell<Option<Arc<Environment>>>,
}

impl std::fmt::Debug for EnvironmentManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnvironmentManager")
            .field("resolver", &self.resolver)
            .finish_non_exhaustive()
    }
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
        Self::from_resolver(DefaultEnvironmentResolver::new(
            exec_server_url,
            local_runtime_paths,
        ))
    }

    fn new_with_options(options: EnvironmentManagerOptions) -> Self {
        let EnvironmentManagerOptions {
            exec_server_url,
            cloud_environment_id,
            cloud_environments_base_url,
            auth_manager,
            local_runtime_paths,
        } = options;
        let (exec_server_url, disabled) = normalize_exec_server_url(exec_server_url);
        if exec_server_url.is_some() || disabled {
            return Self::from_resolver(DefaultEnvironmentResolver {
                exec_server_url,
                local_runtime_paths,
                disabled,
            });
        }

        if let Some(cloud_environment_id) = normalize_optional_env_value(cloud_environment_id) {
            Self::from_resolver(CloudEnvironmentResolver::new(
                cloud_environment_id,
                cloud_environments_base_url,
                auth_manager,
                local_runtime_paths,
            ))
        } else {
            Self::from_resolver(DefaultEnvironmentResolver {
                exec_server_url: None,
                local_runtime_paths,
                disabled: false,
            })
        }
    }

    pub fn from_resolver(resolver: impl EnvironmentResolver + 'static) -> Self {
        Self {
            resolver: Box::new(resolver),
            current_environment: OnceCell::new(),
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
        Self::new_with_options(EnvironmentManagerOptions {
            exec_server_url: std::env::var(CODEX_EXEC_SERVER_URL_ENV_VAR).ok(),
            cloud_environment_id: std::env::var(CODEX_CLOUD_ENVIRONMENT_ID_ENV_VAR).ok(),
            cloud_environments_base_url: std::env::var(CODEX_CLOUD_ENVIRONMENTS_BASE_URL_ENV_VAR)
                .ok(),
            auth_manager: None,
            local_runtime_paths,
        })
    }

    /// Builds a manager from process environment variables and an auth manager.
    /// This is the production constructor used after config/auth are
    /// available.
    pub fn from_env_with_runtime_paths_and_auth_manager(
        local_runtime_paths: Option<ExecServerRuntimePaths>,
        auth_manager: Option<Arc<AuthManager>>,
    ) -> Self {
        Self::new_with_options(EnvironmentManagerOptions {
            exec_server_url: std::env::var(CODEX_EXEC_SERVER_URL_ENV_VAR).ok(),
            cloud_environment_id: std::env::var(CODEX_CLOUD_ENVIRONMENT_ID_ENV_VAR).ok(),
            cloud_environments_base_url: std::env::var(CODEX_CLOUD_ENVIRONMENTS_BASE_URL_ENV_VAR)
                .ok(),
            auth_manager,
            local_runtime_paths,
        })
    }

    /// Builds a manager from process environment variables and the resolved
    /// login config. Cloud environments use ChatGPT credentials, so API-key
    /// auth from environment variables is intentionally disabled here.
    pub fn from_env_with_runtime_paths_and_chatgpt_login_config(
        local_runtime_paths: Option<ExecServerRuntimePaths>,
        config: &impl AuthManagerConfig,
    ) -> Self {
        let auth_manager =
            AuthManager::shared_from_config(config, /*enable_codex_api_key_env*/ false);
        Self::from_env_with_runtime_paths_and_auth_manager(local_runtime_paths, Some(auth_manager))
    }

    /// Builds a manager from the currently selected environment, or from the
    /// disabled mode when no environment is available.
    pub fn from_environment(environment: Option<&Environment>) -> Self {
        match environment {
            Some(environment) => {
                if let Some(cloud_environment_id) = &environment.cloud_environment_id {
                    return Self::from_resolver(CloudEnvironmentResolver::new(
                        cloud_environment_id.clone(),
                        environment.cloud_environments_base_url.clone(),
                        environment.auth_manager.clone(),
                        environment.local_runtime_paths().cloned(),
                    ));
                }

                Self::from_resolver(DefaultEnvironmentResolver::new(
                    environment.exec_server_url().map(str::to_owned),
                    environment.local_runtime_paths().cloned(),
                ))
            }
            None => Self::from_resolver(DefaultEnvironmentResolver::disabled()),
        }
    }

    /// Returns the remote exec-server URL when one is configured.
    pub fn exec_server_url(&self) -> Option<&str> {
        self.resolver.exec_server_url()
    }

    /// Returns true when this manager is configured to use a remote exec server.
    pub fn is_remote(&self) -> bool {
        self.resolver.is_remote()
    }

    /// Returns the cached environment, creating it on first access.
    pub async fn current(&self) -> Result<Option<Arc<Environment>>, ExecServerError> {
        self.current_environment
            .get_or_try_init(|| async {
                self.resolver
                    .create_environment()
                    .await
                    .map(|environment| environment.map(Arc::new))
            })
            .await
            .map(Option::as_ref)
            .map(std::option::Option::<&Arc<Environment>>::cloned)
    }
}

/// Concrete execution/filesystem environment selected for a session.
///
/// This bundles the selected backend together with the corresponding remote
/// client, if any.
#[derive(Clone)]
pub struct Environment {
    exec_server_url: Option<String>,
    cloud_environment_id: Option<String>,
    cloud_environments_base_url: Option<String>,
    auth_manager: Option<Arc<AuthManager>>,
    remote_exec_server_client: Option<ExecServerClient>,
    exec_backend: Arc<dyn ExecBackend>,
    local_runtime_paths: Option<ExecServerRuntimePaths>,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            exec_server_url: None,
            cloud_environment_id: None,
            cloud_environments_base_url: None,
            auth_manager: None,
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
            .field("cloud_environment_id", &self.cloud_environment_id)
            .finish_non_exhaustive()
    }
}

impl Environment {
    /// Builds an environment from the raw `CODEX_EXEC_SERVER_URL` value.
    pub async fn create(exec_server_url: Option<String>) -> Result<Self, ExecServerError> {
        Self::create_with_runtime_paths(
            exec_server_url,
            /*cloud_environment_id*/ None,
            /*cloud_environments_base_url*/ None,
            /*auth_manager*/ None,
            /*local_runtime_paths*/ None,
        )
        .await
    }

    /// Builds an environment from the raw `CODEX_EXEC_SERVER_URL` value and
    /// local runtime paths used when creating local filesystem helpers.
    pub async fn create_with_runtime_paths(
        exec_server_url: Option<String>,
        cloud_environment_id: Option<String>,
        cloud_environments_base_url: Option<String>,
        auth_manager: Option<Arc<AuthManager>>,
        local_runtime_paths: Option<ExecServerRuntimePaths>,
    ) -> Result<Self, ExecServerError> {
        let (exec_server_url, disabled) = normalize_exec_server_url(exec_server_url);
        if disabled {
            return Err(ExecServerError::Protocol(
                "disabled mode does not create an Environment".to_string(),
            ));
        }

        let cloud_environment_id = normalize_optional_env_value(cloud_environment_id);
        let cloud_environments_base_url = normalize_optional_env_value(cloud_environments_base_url);
        let remote_exec_server_client = if let Some(exec_server_url) = &exec_server_url {
            Some(connect_remote_exec_server(exec_server_url.clone()).await?)
        } else if let Some(environment_id) = &cloud_environment_id {
            let base_url = cloud_environments_base_url.clone().ok_or_else(|| {
                ExecServerError::CloudEnvironmentConfig(format!(
                    "{CODEX_CLOUD_ENVIRONMENTS_BASE_URL_ENV_VAR} is required when {CODEX_CLOUD_ENVIRONMENT_ID_ENV_VAR} is set"
                ))
            })?;
            let auth_manager = auth_manager.clone().ok_or_else(|| {
                ExecServerError::CloudEnvironmentAuth(
                    "cloud environment selection requires ChatGPT authentication".to_string(),
                )
            })?;
            let client = CloudEnvironmentClient::new(base_url, auth_manager)?;
            let response = client.connect_environment(environment_id).await?;
            Some(connect_remote_exec_server(response.url).await?)
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
            cloud_environment_id,
            cloud_environments_base_url,
            auth_manager,
            remote_exec_server_client,
            exec_backend,
            local_runtime_paths,
        })
    }

    pub fn is_remote(&self) -> bool {
        self.exec_server_url.is_some() || self.cloud_environment_id.is_some()
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

fn normalize_optional_env_value(value: Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

async fn connect_remote_exec_server(
    websocket_url: String,
) -> Result<ExecServerClient, ExecServerError> {
    ExecServerClient::connect_websocket(RemoteExecServerConnectArgs {
        websocket_url,
        client_name: "codex-environment".to_string(),
        connect_timeout: std::time::Duration::from_secs(5),
        initialize_timeout: std::time::Duration::from_secs(5),
        resume_session_id: None,
    })
    .await
}

struct EnvironmentManagerOptions {
    exec_server_url: Option<String>,
    cloud_environment_id: Option<String>,
    cloud_environments_base_url: Option<String>,
    auth_manager: Option<Arc<AuthManager>>,
    local_runtime_paths: Option<ExecServerRuntimePaths>,
}
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::CloudEnvironmentResolver;
    use super::Environment;
    use super::EnvironmentManager;
    use super::EnvironmentManagerOptions;
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

    #[test]
    fn direct_remote_url_takes_precedence_over_cloud_environment() {
        let manager = EnvironmentManager::new_with_options(EnvironmentManagerOptions {
            exec_server_url: Some("ws://127.0.0.1:8765".to_string()),
            cloud_environment_id: Some("env-1".to_string()),
            cloud_environments_base_url: Some("https://cloud.example.test".to_string()),
            auth_manager: None,
            local_runtime_paths: None,
        });

        assert!(manager.is_remote());
        assert_eq!(manager.exec_server_url(), Some("ws://127.0.0.1:8765"));
    }

    #[test]
    fn none_disables_cloud_environment_selection() {
        let manager = EnvironmentManager::new_with_options(EnvironmentManagerOptions {
            exec_server_url: Some("none".to_string()),
            cloud_environment_id: Some("env-1".to_string()),
            cloud_environments_base_url: Some("https://cloud.example.test".to_string()),
            auth_manager: None,
            local_runtime_paths: None,
        });

        assert!(!manager.is_remote());
    }

    #[test]
    fn cloud_environment_id_is_remote_without_direct_url() {
        let cloud_resolver = CloudEnvironmentResolver::new(
            "env-1".to_string(),
            Some(" https://cloud.example.test ".to_string()),
            /*auth_manager*/ None,
            /*local_runtime_paths*/ None,
        );

        assert_eq!(cloud_resolver.cloud_environment_id(), "env-1");
        assert_eq!(
            cloud_resolver.cloud_environments_base_url(),
            Some("https://cloud.example.test")
        );
        let manager = EnvironmentManager::from_resolver(cloud_resolver);
        assert!(manager.is_remote());
        assert_eq!(manager.exec_server_url(), None);
    }

    #[tokio::test]
    async fn cloud_environment_requires_base_url() {
        let err = Environment::create_with_runtime_paths(
            /*exec_server_url*/ None,
            Some("env-1".to_string()),
            /*cloud_environments_base_url*/ None,
            /*auth_manager*/ None,
            /*local_runtime_paths*/ None,
        )
        .await
        .expect_err("missing base URL should fail");

        assert_eq!(
            err.to_string(),
            "cloud environment configuration error: CODEX_CLOUD_ENVIRONMENTS_BASE_URL is required when CODEX_CLOUD_ENVIRONMENT_ID is set"
        );
    }

    #[tokio::test]
    async fn cloud_environment_requires_auth_manager() {
        let err = Environment::create_with_runtime_paths(
            /*exec_server_url*/ None,
            Some("env-1".to_string()),
            Some("https://cloud.example.test".to_string()),
            /*auth_manager*/ None,
            /*local_runtime_paths*/ None,
        )
        .await
        .expect_err("missing auth should fail");

        assert_eq!(
            err.to_string(),
            "cloud environment authentication error: cloud environment selection requires ChatGPT authentication"
        );
    }

    #[tokio::test]
    async fn environment_manager_current_caches_environment() {
        let manager = EnvironmentManager::new(/*exec_server_url*/ None);

        let first = manager.current().await.expect("get current environment");
        let second = manager.current().await.expect("get current environment");

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
            .current()
            .await
            .expect("get current environment")
            .expect("local environment");

        assert_eq!(environment.local_runtime_paths(), Some(&runtime_paths));
        let inherited_environment = EnvironmentManager::from_environment(Some(&environment))
            .current()
            .await
            .expect("get inherited current environment")
            .expect("inherited local environment");
        assert_eq!(
            inherited_environment.local_runtime_paths(),
            Some(&runtime_paths)
        );
    }

    #[tokio::test]
    async fn disabled_environment_manager_has_no_current_environment() {
        let manager = EnvironmentManager::new(Some("none".to_string()));

        assert!(
            manager
                .current()
                .await
                .expect("get current environment")
                .is_none()
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
                arg0: None,
            })
            .await
            .expect("start process");

        assert_eq!(response.process.process_id().as_str(), "default-env-proc");
    }
}
