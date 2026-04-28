use std::collections::HashMap;
use std::sync::Arc;

use crate::ExecServerError;
use crate::ExecServerRuntimePaths;
use crate::ExecutorFileSystem;
use crate::HttpClient;
use crate::client::LazyRemoteExecServerClient;
use crate::client::http_client::ReqwestHttpClient;
use crate::environment_provider::DefaultEnvironmentProvider;
use crate::environment_provider::EnvironmentConfigurations;
use crate::environment_provider::EnvironmentProvider;
use crate::environment_provider::normalize_exec_server_url;
use crate::local_file_system::LocalFileSystem;
use crate::local_process::LocalProcess;
use crate::process::ExecBackend;
use crate::remote_file_system::RemoteFileSystem;
use crate::remote_process::RemoteProcess;

pub const CODEX_EXEC_SERVER_URL_ENV_VAR: &str = "CODEX_EXEC_SERVER_URL";

/// Owns the execution/filesystem environments available to the Codex runtime.
///
/// `EnvironmentManager` is a shared registry for concrete environments. Its
/// default constructor preserves the legacy `CODEX_EXEC_SERVER_URL` behavior
/// while provider-based construction accepts a provider-supplied snapshot.
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
    pub local_runtime_paths: ExecServerRuntimePaths,
}

impl EnvironmentManagerArgs {
    pub fn new(local_runtime_paths: ExecServerRuntimePaths) -> Self {
        Self {
            local_runtime_paths,
        }
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

    /// Builds a test-only manager with environment access disabled.
    pub fn disabled_for_tests(local_runtime_paths: ExecServerRuntimePaths) -> Self {
        let mut manager =
            Self::from_configurations(EnvironmentConfigurations::empty(), local_runtime_paths);
        manager.default_environment = None;
        manager
    }

    /// Builds a test-only manager from a raw exec-server URL value.
    pub async fn create_for_tests(
        exec_server_url: Option<String>,
        local_runtime_paths: ExecServerRuntimePaths,
    ) -> Self {
        Self::from_default_provider_url(exec_server_url, local_runtime_paths).await
    }

    /// Builds a manager from `CODEX_EXEC_SERVER_URL` and local runtime paths
    /// used when creating local filesystem helpers.
    pub async fn new(args: EnvironmentManagerArgs) -> Self {
        let EnvironmentManagerArgs {
            local_runtime_paths,
        } = args;
        let exec_server_url = std::env::var(CODEX_EXEC_SERVER_URL_ENV_VAR).ok();
        Self::from_default_provider_url(exec_server_url, local_runtime_paths).await
    }

    async fn from_default_provider_url(
        exec_server_url: Option<String>,
        local_runtime_paths: ExecServerRuntimePaths,
    ) -> Self {
        let environment_disabled = normalize_exec_server_url(exec_server_url.clone()).1;
        let provider = DefaultEnvironmentProvider::new(exec_server_url);
        let mut manager = Self::from_configurations(
            provider.environment_configurations().await,
            local_runtime_paths,
        );
        if environment_disabled {
            // TODO: Remove this legacy `CODEX_EXEC_SERVER_URL=none` crutch once
            // environment attachment defaulting moves out of EnvironmentManager.
            manager.default_environment = None;
        }
        manager
    }

    /// Builds a manager from a provider-supplied startup snapshot.
    pub async fn from_provider<P>(
        provider: &P,
        local_runtime_paths: ExecServerRuntimePaths,
    ) -> Result<Self, ExecServerError>
    where
        P: EnvironmentProvider + ?Sized,
    {
        Ok(Self::from_configurations(
            provider.get_environments().await?,
            local_runtime_paths,
        ))
    }

    fn from_configurations(
        environment_configurations: EnvironmentConfigurations,
        local_runtime_paths: ExecServerRuntimePaths,
    ) -> Self {
        let environment_configurations = environment_configurations.into_environments();
        // TODO: Stop deriving a default environment here once omitted
        // environment attachment is owned by thread/session setup.
        let default_environment = if environment_configurations.contains_key(REMOTE_ENVIRONMENT_ID)
        {
            REMOTE_ENVIRONMENT_ID
        } else {
            LOCAL_ENVIRONMENT_ID
        }
        .to_string();
        let mut environments = HashMap::new();
        for (id, configuration) in environment_configurations {
            let environment = Environment::remote_inner(
                configuration.exec_server_url,
                Some(local_runtime_paths.clone()),
            );
            environments.insert(id, Arc::new(environment));
        }
        environments.insert(
            LOCAL_ENVIRONMENT_ID.to_string(),
            Arc::new(Environment::local(local_runtime_paths)),
        );

        Self {
            default_environment: Some(default_environment),
            environments,
        }
    }

    /// Returns the default environment instance.
    pub fn default_environment(&self) -> Option<Arc<Environment>> {
        self.default_environment
            .as_deref()
            .and_then(|environment_id| self.get_environment(environment_id))
    }

    /// Returns the id of the default environment.
    pub fn default_environment_id(&self) -> Option<&str> {
        self.default_environment.as_deref()
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
    exec_server_url: Option<String>,
    exec_backend: Arc<dyn ExecBackend>,
    filesystem: Arc<dyn ExecutorFileSystem>,
    http_client: Arc<dyn HttpClient>,
    local_runtime_paths: Option<ExecServerRuntimePaths>,
}

impl Environment {
    /// Builds a test-only local environment without configured sandbox helper paths.
    pub fn default_for_tests() -> Self {
        Self {
            exec_server_url: None,
            exec_backend: Arc::new(LocalProcess::default()),
            filesystem: Arc::new(LocalFileSystem::unsandboxed()),
            http_client: Arc::new(ReqwestHttpClient),
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
            Some(exec_server_url) => Self::remote_inner(exec_server_url, local_runtime_paths),
            None => match local_runtime_paths {
                Some(local_runtime_paths) => Self::local(local_runtime_paths),
                None => Self::default_for_tests(),
            },
        })
    }

    fn local(local_runtime_paths: ExecServerRuntimePaths) -> Self {
        Self {
            exec_server_url: None,
            exec_backend: Arc::new(LocalProcess::default()),
            filesystem: Arc::new(LocalFileSystem::with_runtime_paths(
                local_runtime_paths.clone(),
            )),
            http_client: Arc::new(ReqwestHttpClient),
            local_runtime_paths: Some(local_runtime_paths),
        }
    }

    fn remote_inner(
        exec_server_url: String,
        local_runtime_paths: Option<ExecServerRuntimePaths>,
    ) -> Self {
        let client = LazyRemoteExecServerClient::new(exec_server_url.clone());
        let exec_backend: Arc<dyn ExecBackend> = Arc::new(RemoteProcess::new(client.clone()));
        let filesystem: Arc<dyn ExecutorFileSystem> =
            Arc::new(RemoteFileSystem::new(client.clone()));

        Self {
            exec_server_url: Some(exec_server_url),
            exec_backend,
            filesystem,
            http_client: Arc::new(client),
            local_runtime_paths,
        }
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

    pub fn get_http_client(&self) -> Arc<dyn HttpClient> {
        Arc::clone(&self.http_client)
    }

    pub fn get_filesystem(&self) -> Arc<dyn ExecutorFileSystem> {
        Arc::clone(&self.filesystem)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use super::Environment;
    use super::EnvironmentManager;
    use super::LOCAL_ENVIRONMENT_ID;
    use super::REMOTE_ENVIRONMENT_ID;
    use crate::EnvironmentConfiguration;
    use crate::EnvironmentConfigurations;
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
        let manager =
            EnvironmentManager::create_for_tests(Some(String::new()), test_runtime_paths()).await;

        let environment = manager.default_environment().expect("default environment");
        assert_eq!(manager.default_environment_id(), Some(LOCAL_ENVIRONMENT_ID));
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
    async fn disabled_environment_manager_has_no_default_but_keeps_local_lookup() {
        let manager = EnvironmentManager::disabled_for_tests(test_runtime_paths());

        assert!(manager.default_environment().is_none());
        assert_eq!(manager.default_environment_id(), None);
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
        let manager = EnvironmentManager::create_for_tests(
            Some("ws://127.0.0.1:8765".to_string()),
            test_runtime_paths(),
        )
        .await;

        let environment = manager.default_environment().expect("default environment");
        assert_eq!(
            manager.default_environment_id(),
            Some(REMOTE_ENVIRONMENT_ID)
        );
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
    async fn environment_manager_builds_from_provider_configurations() {
        let manager = EnvironmentManager::from_configurations(
            EnvironmentConfigurations::new(HashMap::from([(
                REMOTE_ENVIRONMENT_ID.to_string(),
                EnvironmentConfiguration {
                    exec_server_url: "ws://127.0.0.1:8765".to_string(),
                },
            )]))
            .expect("environment configurations"),
            test_runtime_paths(),
        );

        assert_eq!(
            manager.default_environment_id(),
            Some(REMOTE_ENVIRONMENT_ID)
        );
        assert!(
            manager
                .get_environment(REMOTE_ENVIRONMENT_ID)
                .expect("remote environment")
                .is_remote()
        );
        assert!(
            !manager
                .get_environment(LOCAL_ENVIRONMENT_ID)
                .expect("local environment")
                .is_remote()
        );
    }

    #[tokio::test]
    async fn environment_manager_inserts_local_environment() {
        let manager = EnvironmentManager::from_configurations(
            EnvironmentConfigurations::empty(),
            test_runtime_paths(),
        );

        assert_eq!(manager.default_environment_id(), Some(LOCAL_ENVIRONMENT_ID));
        assert!(
            !manager
                .get_environment(LOCAL_ENVIRONMENT_ID)
                .expect("local environment")
                .is_remote()
        );
    }

    #[tokio::test]
    async fn environment_manager_carries_local_runtime_paths() {
        let runtime_paths = test_runtime_paths();
        let manager = EnvironmentManager::create_for_tests(
            /*exec_server_url*/ None,
            runtime_paths.clone(),
        )
        .await;

        let environment = manager.default_environment().expect("default environment");

        assert_eq!(environment.local_runtime_paths(), Some(&runtime_paths));
        let manager = EnvironmentManager::create_for_tests(
            environment.exec_server_url().map(str::to_owned),
            environment
                .local_runtime_paths()
                .expect("local runtime paths")
                .clone(),
        )
        .await;
        let environment = manager.default_environment().expect("default environment");
        assert_eq!(environment.local_runtime_paths(), Some(&runtime_paths));
    }

    #[tokio::test]
    async fn disabled_environment_manager_has_no_default_environment() {
        let manager = EnvironmentManager::disabled_for_tests(test_runtime_paths());

        assert!(manager.default_environment().is_none());
        assert_eq!(manager.default_environment_id(), None);
    }

    #[tokio::test]
    async fn environment_manager_keeps_local_lookup_when_default_disabled() {
        let manager = EnvironmentManager::disabled_for_tests(test_runtime_paths());

        assert!(manager.default_environment().is_none());
        assert_eq!(manager.default_environment_id(), None);
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
        let sandbox = crate::FileSystemSandboxContext::from_permission_profile(
            codex_protocol::models::PermissionProfile::from_runtime_permissions(
                &codex_protocol::permissions::FileSystemSandboxPolicy::restricted(Vec::new()),
                codex_protocol::permissions::NetworkSandboxPolicy::Restricted,
            ),
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
