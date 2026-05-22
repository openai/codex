use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;

use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RuntimeInstallParams;
use codex_app_server_protocol::RuntimeInstallResponse;
use codex_utils_absolute_path::AbsolutePathBuf;
use futures::FutureExt;
use futures::future::BoxFuture;

use crate::ExecServerError;
use crate::ExecServerRuntimePaths;
use crate::ExecutorFileSystem;
use crate::HttpClient;
use crate::client::LazyRemoteExecServerClient;
use crate::client::http_client::ReqwestHttpClient;
use crate::client_api::ExecServerTransportParams;
use crate::codex_home::default_codex_home_path;
use crate::environment_provider::DefaultEnvironmentProvider;
use crate::environment_provider::EnvironmentDefault;
use crate::environment_provider::EnvironmentProvider;
use crate::environment_provider::EnvironmentProviderSnapshot;
use crate::environment_provider::normalize_exec_server_url;
use crate::environment_toml::environment_provider_from_codex_home;
use crate::local_file_system::LocalFileSystem;
use crate::local_process::LocalProcess;
use crate::process::ExecBackend;
use crate::protocol::EnvironmentInfo;
use crate::protocol::ShellInfo;
use crate::remote_file_system::RemoteFileSystem;
use crate::remote_process::RemoteProcess;
use crate::rpc::internal_error;
use codex_shell_command::shell_detect::DetectedShell;

pub const CODEX_EXEC_SERVER_URL_ENV_VAR: &str = "CODEX_EXEC_SERVER_URL";

/// Owns the execution/filesystem environments available to the Codex runtime.
///
/// `EnvironmentManager` is a shared registry for concrete environments. Its
/// default constructor preserves the legacy `CODEX_EXEC_SERVER_URL` behavior
/// while configured construction accepts a provider-supplied snapshot.
///
/// Setting `CODEX_EXEC_SERVER_URL=none` disables environment access by leaving
/// the default environment unset and omitting the local environment. Callers
/// use `default_environment().is_some()` as the signal for model-facing
/// shell/filesystem tool availability.
///
/// Remote environments create remote filesystem and execution backends that
/// lazy-connect to the configured exec-server on first use. The remote
/// transport is not opened when the manager or environment is constructed.
#[derive(Debug)]
pub struct EnvironmentManager {
    default_environment: Option<String>,
    environments: RwLock<HashMap<String, Arc<Environment>>>,
    local_environment: Option<Arc<Environment>>,
    local_runtime_paths: Option<ExecServerRuntimePaths>,
}

pub const LOCAL_ENVIRONMENT_ID: &str = "local";
pub const REMOTE_ENVIRONMENT_ID: &str = "remote";

impl EnvironmentManager {
    /// Builds a test-only manager without configured sandbox helper paths.
    pub fn default_for_tests() -> Self {
        Self {
            default_environment: Some(LOCAL_ENVIRONMENT_ID.to_string()),
            environments: RwLock::new(HashMap::from([(
                LOCAL_ENVIRONMENT_ID.to_string(),
                Arc::new(Environment::default_for_tests()),
            )])),
            local_environment: Some(Arc::new(Environment::default_for_tests())),
            local_runtime_paths: None,
        }
    }

    /// Builds a manager with no configured execution environments.
    pub fn without_environments() -> Self {
        Self {
            default_environment: None,
            environments: RwLock::new(HashMap::new()),
            local_environment: None,
            local_runtime_paths: None,
        }
    }

    /// Builds a test-only manager from a raw exec-server URL value.
    pub async fn create_for_tests(
        exec_server_url: Option<String>,
        local_runtime_paths: Option<ExecServerRuntimePaths>,
    ) -> Self {
        Self::from_default_provider_url(exec_server_url, local_runtime_paths).await
    }

    /// Builds a manager from `CODEX_HOME` and local runtime paths used when
    /// creating local filesystem helpers.
    ///
    /// If `CODEX_HOME/environments.toml` is present, it defines the configured
    /// environments. Otherwise this preserves the legacy
    /// `CODEX_EXEC_SERVER_URL` behavior.
    pub async fn from_codex_home(
        codex_home: impl AsRef<std::path::Path>,
        local_runtime_paths: Option<ExecServerRuntimePaths>,
    ) -> Result<Self, ExecServerError> {
        let provider = environment_provider_from_codex_home(codex_home.as_ref())?;
        let local_codex_home = AbsolutePathBuf::from_absolute_path_checked(codex_home.as_ref())
            .map_err(|err| ExecServerError::Protocol(err.to_string()))?;
        Self::from_snapshot_with_codex_home(
            provider.snapshot().await?,
            local_runtime_paths,
            Some(local_codex_home),
        )
    }

    /// Builds a manager from the legacy environment-variable provider without
    /// reading user config files from `CODEX_HOME`.
    pub async fn from_env(
        local_runtime_paths: Option<ExecServerRuntimePaths>,
    ) -> Result<Self, ExecServerError> {
        let provider = DefaultEnvironmentProvider::from_env();
        Self::from_snapshot(provider.snapshot().await?, local_runtime_paths)
    }

    async fn from_default_provider_url(
        exec_server_url: Option<String>,
        local_runtime_paths: Option<ExecServerRuntimePaths>,
    ) -> Self {
        let provider = DefaultEnvironmentProvider::new(exec_server_url);
        match Self::from_snapshot(provider.snapshot_inner(), local_runtime_paths) {
            Ok(manager) => manager,
            Err(err) => panic!("default provider should create valid environments: {err}"),
        }
    }

    /// Builds a test-only manager that keeps the provider default while also
    /// allowing tests to select the local environment explicitly.
    pub async fn create_for_tests_with_local(
        exec_server_url: Option<String>,
        local_runtime_paths: ExecServerRuntimePaths,
    ) -> Self {
        let mut snapshot = DefaultEnvironmentProvider::new(exec_server_url).snapshot_inner();
        snapshot.include_local = true;
        match Self::from_snapshot(snapshot, Some(local_runtime_paths)) {
            Ok(manager) => manager,
            Err(err) => panic!("test provider with local should create valid environments: {err}"),
        }
    }

    fn from_snapshot(
        snapshot: EnvironmentProviderSnapshot,
        local_runtime_paths: Option<ExecServerRuntimePaths>,
    ) -> Result<Self, ExecServerError> {
        Self::from_snapshot_with_codex_home(
            snapshot,
            local_runtime_paths,
            default_local_codex_home(),
        )
    }

    fn from_snapshot_with_codex_home(
        snapshot: EnvironmentProviderSnapshot,
        local_runtime_paths: Option<ExecServerRuntimePaths>,
        local_codex_home: Option<AbsolutePathBuf>,
    ) -> Result<Self, ExecServerError> {
        let EnvironmentProviderSnapshot {
            environments,
            default,
            include_local,
        } = snapshot;
        let mut environment_map =
            HashMap::with_capacity(environments.len() + usize::from(include_local));
        let local_environment = if include_local {
            let local_runtime_paths = local_runtime_paths.clone().ok_or_else(|| {
                ExecServerError::Protocol(
                    "local environment requires configured runtime paths".to_string(),
                )
            })?;
            let local_environment = Arc::new(Environment::local_with_codex_home(
                local_runtime_paths,
                local_codex_home,
            ));
            environment_map.insert(
                LOCAL_ENVIRONMENT_ID.to_string(),
                Arc::clone(&local_environment),
            );
            Some(local_environment)
        } else {
            None
        };
        for (id, environment) in environments {
            if id.is_empty() {
                return Err(ExecServerError::Protocol(
                    "environment id cannot be empty".to_string(),
                ));
            }
            if id == LOCAL_ENVIRONMENT_ID {
                return Err(ExecServerError::Protocol(format!(
                    "environment id `{LOCAL_ENVIRONMENT_ID}` is reserved for EnvironmentManager"
                )));
            }
            if environment_map
                .insert(id.clone(), Arc::new(environment))
                .is_some()
            {
                return Err(ExecServerError::Protocol(format!(
                    "environment id `{id}` is duplicated"
                )));
            }
        }
        let default_environment = match default {
            EnvironmentDefault::Disabled => None,
            EnvironmentDefault::EnvironmentId(environment_id) => {
                if !environment_map.contains_key(&environment_id) {
                    return Err(ExecServerError::Protocol(format!(
                        "default environment `{environment_id}` is not configured"
                    )));
                }
                Some(environment_id)
            }
        };
        Ok(Self {
            default_environment,
            environments: RwLock::new(environment_map),
            local_environment,
            local_runtime_paths,
        })
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

    /// Returns the ordered environment ids used for new thread startup.
    pub fn default_environment_ids(&self) -> Vec<String> {
        let Some(default_environment_id) = self.default_environment.as_ref() else {
            return Vec::new();
        };
        let environments = self
            .environments
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut environment_ids = Vec::with_capacity(environments.len());
        environment_ids.push(default_environment_id.clone());
        environment_ids.extend(
            environments
                .keys()
                .filter(|environment_id| *environment_id != default_environment_id)
                .cloned(),
        );
        environment_ids
    }

    /// Returns the local environment instance when one is configured.
    pub fn try_local_environment(&self) -> Option<Arc<Environment>> {
        self.local_environment.as_ref().map(Arc::clone)
    }

    /// Returns the default environment or local environment when either exists.
    pub fn default_or_local_environment(&self) -> Option<Arc<Environment>> {
        self.default_environment()
            .or_else(|| self.try_local_environment())
    }

    /// Returns a named environment instance.
    pub fn get_environment(&self, environment_id: &str) -> Option<Arc<Environment>> {
        self.environments
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(environment_id)
            .cloned()
    }

    /// Adds or replaces a named remote environment without changing the
    /// manager's default environment selection.
    pub fn upsert_environment(
        &self,
        environment_id: String,
        exec_server_url: String,
    ) -> Result<(), ExecServerError> {
        if environment_id.is_empty() {
            return Err(ExecServerError::Protocol(
                "environment id cannot be empty".to_string(),
            ));
        }
        let (exec_server_url, disabled) = normalize_exec_server_url(Some(exec_server_url));
        if disabled {
            return Err(ExecServerError::Protocol(
                "remote environment cannot use disabled exec-server url".to_string(),
            ));
        }
        let Some(exec_server_url) = exec_server_url else {
            return Err(ExecServerError::Protocol(
                "remote environment requires an exec-server url".to_string(),
            ));
        };
        let environment =
            Environment::remote_inner(exec_server_url, self.local_runtime_paths.clone());
        self.environments
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(environment_id, Arc::new(environment));
        Ok(())
    }
}

/// Concrete execution/filesystem environment selected for a session.
///
/// This bundles the selected backend metadata together with the local runtime
/// paths used by filesystem helpers.
#[derive(Clone)]
pub struct Environment {
    exec_server_url: Option<String>,
    remote_transport: Option<ExecServerTransportParams>,
    info_provider: Arc<dyn EnvironmentInfoProvider>,
    exec_backend: Arc<dyn ExecBackend>,
    filesystem: Arc<dyn ExecutorFileSystem>,
    http_client: Arc<dyn HttpClient>,
    runtime_installer: RuntimeInstaller,
    local_runtime_paths: Option<ExecServerRuntimePaths>,
    codex_home: Option<AbsolutePathBuf>,
}

/// Provides environment metadata from either a local environment or a remote exec-server.
trait EnvironmentInfoProvider: Send + Sync {
    fn info(&self) -> BoxFuture<'_, Result<EnvironmentInfo, ExecServerError>>;
}

struct LocalEnvironmentInfoProvider;

impl EnvironmentInfoProvider for LocalEnvironmentInfoProvider {
    fn info(&self) -> BoxFuture<'_, Result<EnvironmentInfo, ExecServerError>> {
        std::future::ready(Ok(EnvironmentInfo::local())).boxed()
    }
}

struct RemoteEnvironmentInfoProvider {
    client: LazyRemoteExecServerClient,
}

impl RemoteEnvironmentInfoProvider {
    fn new(client: LazyRemoteExecServerClient) -> Self {
        Self { client }
    }
}

impl EnvironmentInfoProvider for RemoteEnvironmentInfoProvider {
    fn info(&self) -> BoxFuture<'_, Result<EnvironmentInfo, ExecServerError>> {
        async move { self.client.environment_info().await }.boxed()
    }
}

#[derive(Clone)]
enum RuntimeInstaller {
    Local,
    Remote(LazyRemoteExecServerClient),
}

impl RuntimeInstaller {
    async fn install_runtime(
        &self,
        params: RuntimeInstallParams,
    ) -> Result<RuntimeInstallResponse, JSONRPCErrorError> {
        match self {
            RuntimeInstaller::Local => crate::runtime_install::install_runtime(params).await,
            RuntimeInstaller::Remote(client) => {
                let client = client.get().await.map_err(exec_server_error_to_jsonrpc)?;
                client
                    .runtime_install(params)
                    .await
                    .map_err(exec_server_error_to_jsonrpc)
            }
        }
    }
}

fn exec_server_error_to_jsonrpc(err: ExecServerError) -> JSONRPCErrorError {
    match err {
        ExecServerError::Server { code, message } => JSONRPCErrorError {
            code,
            data: None,
            message,
        },
        _ => internal_error(err.to_string()),
    }
}

impl Environment {
    /// Builds a test-only local environment without configured sandbox helper paths.
    pub fn default_for_tests() -> Self {
        Self {
            exec_server_url: None,
            remote_transport: None,
            info_provider: Arc::new(LocalEnvironmentInfoProvider),
            exec_backend: Arc::new(LocalProcess::default()),
            filesystem: Arc::new(LocalFileSystem::unsandboxed()),
            http_client: Arc::new(ReqwestHttpClient),
            runtime_installer: RuntimeInstaller::Local,
            local_runtime_paths: None,
            codex_home: default_local_codex_home(),
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

    pub(crate) fn local(local_runtime_paths: ExecServerRuntimePaths) -> Self {
        Self::local_with_codex_home(local_runtime_paths, default_local_codex_home())
    }

    fn local_with_codex_home(
        local_runtime_paths: ExecServerRuntimePaths,
        codex_home: Option<AbsolutePathBuf>,
    ) -> Self {
        Self {
            exec_server_url: None,
            remote_transport: None,
            info_provider: Arc::new(LocalEnvironmentInfoProvider),
            exec_backend: Arc::new(LocalProcess::default()),
            filesystem: Arc::new(LocalFileSystem::with_runtime_paths(
                local_runtime_paths.clone(),
            )),
            http_client: Arc::new(ReqwestHttpClient),
            runtime_installer: RuntimeInstaller::Local,
            local_runtime_paths: Some(local_runtime_paths),
            codex_home,
        }
    }

    pub(crate) fn remote_inner(
        exec_server_url: String,
        local_runtime_paths: Option<ExecServerRuntimePaths>,
    ) -> Self {
        Self::remote_with_transport(
            ExecServerTransportParams::websocket_url(exec_server_url),
            local_runtime_paths,
        )
    }

    pub(crate) fn remote_with_transport(
        remote_transport: ExecServerTransportParams,
        local_runtime_paths: Option<ExecServerRuntimePaths>,
    ) -> Self {
        let exec_server_url = match &remote_transport {
            ExecServerTransportParams::WebSocketUrl {
                websocket_url: exec_server_url,
                ..
            } => Some(exec_server_url.clone()),
            ExecServerTransportParams::StdioCommand { .. } => None,
        };
        let client = LazyRemoteExecServerClient::new(remote_transport.clone());
        let exec_backend: Arc<dyn ExecBackend> = Arc::new(RemoteProcess::new(client.clone()));
        let filesystem: Arc<dyn ExecutorFileSystem> =
            Arc::new(RemoteFileSystem::new(client.clone()));
        let http_client = client.clone();

        Self {
            exec_server_url,
            remote_transport: Some(remote_transport),
            info_provider: Arc::new(RemoteEnvironmentInfoProvider::new(client.clone())),
            exec_backend,
            filesystem,
            http_client: Arc::new(http_client),
            runtime_installer: RuntimeInstaller::Remote(client),
            local_runtime_paths,
            codex_home: None,
        }
    }

    pub fn is_remote(&self) -> bool {
        self.remote_transport.is_some()
    }

    /// Returns the remote exec-server URL when this environment is remote.
    pub fn exec_server_url(&self) -> Option<&str> {
        self.exec_server_url.as_deref()
    }

    pub fn local_runtime_paths(&self) -> Option<&ExecServerRuntimePaths> {
        self.local_runtime_paths.as_ref()
    }

    /// Returns environment information from the selected execution/filesystem environment.
    pub async fn info(&self) -> Result<EnvironmentInfo, ExecServerError> {
        self.info_provider.info().await
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

    pub async fn install_runtime(
        &self,
        params: RuntimeInstallParams,
    ) -> Result<RuntimeInstallResponse, JSONRPCErrorError> {
        self.runtime_installer.install_runtime(params).await
    }

    pub async fn codex_home(&self) -> Result<AbsolutePathBuf, JSONRPCErrorError> {
        if let Some(codex_home) = self.codex_home.clone() {
            return Ok(codex_home);
        }
        match &self.runtime_installer {
            RuntimeInstaller::Local => default_local_codex_home().ok_or_else(|| {
                internal_error("failed to locate local codex home for runtime install")
            }),
            RuntimeInstaller::Remote(client) => {
                let client = client.get().await.map_err(exec_server_error_to_jsonrpc)?;
                client
                    .codex_home()
                    .ok_or_else(|| internal_error("remote exec-server did not report a codex home"))
            }
        }
    }
}

fn default_local_codex_home() -> Option<AbsolutePathBuf> {
    default_codex_home_path()
        .ok()
        .and_then(|path| AbsolutePathBuf::from_absolute_path_checked(path).ok())
}

impl EnvironmentInfo {
    pub(crate) fn local() -> Self {
        Self {
            shell: codex_shell_command::shell_detect::default_user_shell().into(),
        }
    }
}

impl From<DetectedShell> for ShellInfo {
    fn from(shell: DetectedShell) -> Self {
        Self {
            name: shell.name().to_string(),
            path: shell.shell_path.to_string_lossy().into_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::Environment;
    use super::EnvironmentManager;
    use super::LOCAL_ENVIRONMENT_ID;
    use super::REMOTE_ENVIRONMENT_ID;
    use crate::ExecServerRuntimePaths;
    use crate::ProcessId;
    use crate::environment_provider::EnvironmentDefault;
    use crate::environment_provider::EnvironmentProviderSnapshot;
    use codex_utils_path_uri::PathUri;
    use pretty_assertions::assert_eq;

    fn test_runtime_paths() -> ExecServerRuntimePaths {
        ExecServerRuntimePaths::new(
            std::env::current_exe().expect("current exe"),
            /*codex_linux_sandbox_exe*/ None,
        )
        .expect("runtime paths")
    }

    fn assert_local_environment_unavailable(manager: &EnvironmentManager) {
        assert!(manager.try_local_environment().is_none());
    }

    #[tokio::test]
    async fn create_local_environment_does_not_connect() {
        let environment = Environment::create(/*exec_server_url*/ None, test_runtime_paths())
            .expect("create environment");

        assert_eq!(environment.exec_server_url(), None);
        assert!(!environment.is_remote());
        assert!(environment.info().await.is_ok());
    }

    #[tokio::test]
    async fn environment_manager_normalizes_empty_url() {
        let manager =
            EnvironmentManager::create_for_tests(Some(String::new()), Some(test_runtime_paths()))
                .await;

        let environment = manager.default_environment().expect("default environment");
        assert_eq!(manager.default_environment_id(), Some(LOCAL_ENVIRONMENT_ID));
        assert!(Arc::ptr_eq(
            &environment,
            &manager
                .get_environment(LOCAL_ENVIRONMENT_ID)
                .expect("local environment")
        ));
        assert!(Arc::ptr_eq(
            &environment,
            &manager.try_local_environment().expect("local environment")
        ));
        assert!(manager.try_local_environment().is_some());
        assert!(manager.get_environment(REMOTE_ENVIRONMENT_ID).is_none());
        assert!(!environment.is_remote());
    }

    #[tokio::test]
    async fn disabled_environment_manager_has_no_default_or_local_environment() {
        let manager = EnvironmentManager::without_environments();

        assert!(manager.default_environment().is_none());
        assert_eq!(manager.default_environment_id(), None);
        assert_local_environment_unavailable(&manager);
        assert!(manager.get_environment(LOCAL_ENVIRONMENT_ID).is_none());
        assert!(manager.get_environment(REMOTE_ENVIRONMENT_ID).is_none());
    }

    #[tokio::test]
    async fn environment_manager_reports_remote_url() {
        let manager = EnvironmentManager::create_for_tests(
            Some("ws://127.0.0.1:8765".to_string()),
            Some(test_runtime_paths()),
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
        assert!(manager.get_environment(LOCAL_ENVIRONMENT_ID).is_none());
        assert_local_environment_unavailable(&manager);
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
    async fn environment_manager_builds_from_snapshot() {
        let snapshot = EnvironmentProviderSnapshot {
            environments: vec![(
                REMOTE_ENVIRONMENT_ID.to_string(),
                Environment::create_for_tests(Some("ws://127.0.0.1:8765".to_string()))
                    .expect("remote environment"),
            )],
            default: EnvironmentDefault::EnvironmentId(REMOTE_ENVIRONMENT_ID.to_string()),
            include_local: false,
        };
        let manager = EnvironmentManager::from_snapshot(snapshot, Some(test_runtime_paths()))
            .expect("environment manager");

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
        assert!(manager.get_environment(LOCAL_ENVIRONMENT_ID).is_none());
        assert_local_environment_unavailable(&manager);
    }

    #[tokio::test]
    async fn environment_manager_rejects_empty_environment_id() {
        let snapshot = EnvironmentProviderSnapshot {
            environments: vec![("".to_string(), Environment::default_for_tests())],
            default: EnvironmentDefault::Disabled,
            include_local: false,
        };
        let err = EnvironmentManager::from_snapshot(snapshot, Some(test_runtime_paths()))
            .expect_err("empty id should fail");

        assert_eq!(
            err.to_string(),
            "exec-server protocol error: environment id cannot be empty"
        );
    }

    #[tokio::test]
    async fn environment_manager_rejects_provider_supplied_local_environment() {
        let snapshot = EnvironmentProviderSnapshot {
            environments: vec![(
                LOCAL_ENVIRONMENT_ID.to_string(),
                Environment::default_for_tests(),
            )],
            default: EnvironmentDefault::Disabled,
            include_local: false,
        };
        let err = EnvironmentManager::from_snapshot(snapshot, Some(test_runtime_paths()))
            .expect_err("local id should fail");

        assert_eq!(
            err.to_string(),
            "exec-server protocol error: environment id `local` is reserved for EnvironmentManager"
        );
    }

    #[tokio::test]
    async fn environment_manager_uses_explicit_provider_default() {
        let snapshot = EnvironmentProviderSnapshot {
            environments: vec![(
                "devbox".to_string(),
                Environment::create_for_tests(Some("ws://127.0.0.1:8765".to_string()))
                    .expect("remote environment"),
            )],
            default: EnvironmentDefault::EnvironmentId("devbox".to_string()),
            include_local: true,
        };
        let manager = EnvironmentManager::from_snapshot(snapshot, Some(test_runtime_paths()))
            .expect("manager");

        assert_eq!(manager.default_environment_id(), Some("devbox"));
        assert_eq!(
            manager.default_environment_ids(),
            vec!["devbox".to_string(), LOCAL_ENVIRONMENT_ID.to_string()]
        );
        assert!(manager.default_environment().expect("default").is_remote());
    }

    #[tokio::test]
    async fn environment_manager_disables_provider_default() {
        let snapshot = EnvironmentProviderSnapshot {
            environments: vec![(
                "devbox".to_string(),
                Environment::create_for_tests(Some("ws://127.0.0.1:8765".to_string()))
                    .expect("remote environment"),
            )],
            default: EnvironmentDefault::Disabled,
            include_local: true,
        };
        let manager = EnvironmentManager::from_snapshot(snapshot, Some(test_runtime_paths()))
            .expect("manager");

        assert_eq!(manager.default_environment_id(), None);
        assert!(manager.default_environment().is_none());
        assert!(Arc::ptr_eq(
            &manager
                .get_environment(LOCAL_ENVIRONMENT_ID)
                .expect("local environment"),
            &manager.try_local_environment().expect("local environment")
        ));
    }

    #[tokio::test]
    async fn environment_manager_rejects_unknown_provider_default() {
        let snapshot = EnvironmentProviderSnapshot {
            environments: vec![(
                "devbox".to_string(),
                Environment::create_for_tests(Some("ws://127.0.0.1:8765".to_string()))
                    .expect("remote environment"),
            )],
            default: EnvironmentDefault::EnvironmentId("missing".to_string()),
            include_local: true,
        };
        let err = EnvironmentManager::from_snapshot(snapshot, Some(test_runtime_paths()))
            .expect_err("unknown default should fail");

        assert_eq!(
            err.to_string(),
            "exec-server protocol error: default environment `missing` is not configured"
        );
    }

    #[tokio::test]
    async fn environment_manager_includes_local_for_default_provider_without_url() {
        let manager = EnvironmentManager::create_for_tests(
            /*exec_server_url*/ None,
            Some(test_runtime_paths()),
        )
        .await;

        let environment = manager.default_environment().expect("default environment");
        assert_eq!(manager.default_environment_id(), Some(LOCAL_ENVIRONMENT_ID));
        assert!(Arc::ptr_eq(
            &environment,
            &manager
                .get_environment(LOCAL_ENVIRONMENT_ID)
                .expect("local environment")
        ));
        assert!(Arc::ptr_eq(
            &environment,
            &manager.try_local_environment().expect("local environment")
        ));
        assert!(!environment.is_remote());
    }

    #[tokio::test]
    async fn environment_manager_carries_local_runtime_paths() {
        let runtime_paths = test_runtime_paths();
        let manager = EnvironmentManager::create_for_tests(
            /*exec_server_url*/ None,
            Some(runtime_paths.clone()),
        )
        .await;

        let environment = manager.try_local_environment().expect("local environment");

        assert_eq!(environment.local_runtime_paths(), Some(&runtime_paths));
        let manager = EnvironmentManager::create_for_tests(
            environment.exec_server_url().map(str::to_owned),
            Some(
                environment
                    .local_runtime_paths()
                    .expect("local runtime paths")
                    .clone(),
            ),
        )
        .await;
        let environment = manager.try_local_environment().expect("local environment");
        assert_eq!(environment.local_runtime_paths(), Some(&runtime_paths));
    }

    #[tokio::test]
    async fn environment_manager_omits_default_provider_local_lookup_when_default_disabled() {
        let manager = EnvironmentManager::create_for_tests(
            Some("none".to_string()),
            Some(test_runtime_paths()),
        )
        .await;

        assert!(manager.default_environment().is_none());
        assert_eq!(manager.default_environment_id(), None);
        assert!(manager.get_environment(LOCAL_ENVIRONMENT_ID).is_none());
        assert!(manager.get_environment(REMOTE_ENVIRONMENT_ID).is_none());
        assert_local_environment_unavailable(&manager);
    }

    #[tokio::test]
    async fn environment_manager_snapshot_without_local_environment_disables_local_default() {
        let mut snapshot = EnvironmentProviderSnapshot {
            environments: Vec::new(),
            default: EnvironmentDefault::EnvironmentId(LOCAL_ENVIRONMENT_ID.to_string()),
            include_local: true,
        };
        snapshot.include_local = false;
        snapshot.default = EnvironmentDefault::Disabled;
        let manager =
            EnvironmentManager::from_snapshot(snapshot, /*local_runtime_paths*/ None)
                .expect("environment manager");

        assert!(manager.default_environment().is_none());
        assert_eq!(manager.default_environment_id(), None);
        assert!(manager.get_environment(LOCAL_ENVIRONMENT_ID).is_none());
        assert_local_environment_unavailable(&manager);
    }

    #[tokio::test]
    async fn get_environment_returns_none_for_unknown_id() {
        let manager = EnvironmentManager::default_for_tests();

        assert!(manager.get_environment("does-not-exist").is_none());
    }

    #[tokio::test]
    async fn environment_manager_upserts_named_remote_environment() {
        let manager = EnvironmentManager::without_environments();

        manager
            .upsert_environment("executor-a".to_string(), "ws://127.0.0.1:8765".to_string())
            .expect("remote environment");
        let first = manager
            .get_environment("executor-a")
            .expect("first remote environment");
        assert!(first.is_remote());
        assert_eq!(first.exec_server_url(), Some("ws://127.0.0.1:8765"));
        assert_eq!(manager.default_environment_id(), None);

        manager
            .upsert_environment("executor-a".to_string(), "ws://127.0.0.1:9876".to_string())
            .expect("updated remote environment");
        let second = manager
            .get_environment("executor-a")
            .expect("second remote environment");
        assert!(second.is_remote());
        assert_eq!(second.exec_server_url(), Some("ws://127.0.0.1:9876"));
        assert!(!Arc::ptr_eq(&first, &second));
    }

    #[tokio::test]
    async fn environment_manager_rejects_empty_remote_environment_url() {
        let manager = EnvironmentManager::without_environments();

        let err = manager
            .upsert_environment("executor-a".to_string(), String::new())
            .expect_err("empty URL should fail");

        assert_eq!(
            err.to_string(),
            "exec-server protocol error: remote environment requires an exec-server url"
        );
    }

    #[tokio::test]
    async fn default_environment_has_ready_local_executor() {
        let environment = Environment::default_for_tests();

        let response = environment
            .get_exec_backend()
            .start(crate::ExecParams {
                process_id: ProcessId::from("default-env-proc"),
                argv: vec!["true".to_string()],
                cwd: PathUri::from_path(std::env::current_dir().expect("read current dir"))
                    .expect("cwd URI"),
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
        let path = codex_utils_path_uri::PathUri::from_abs_path(&path);
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
