use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::RwLock;

use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use futures::FutureExt;

use crate::CapabilityRootsDiscoverParams;
use crate::CapabilityRootsDiscoverResponse;
use crate::ExecServerError;
use crate::ExecServerRuntimePaths;
use crate::ExecutorFileSystem;
use crate::HttpClient;
use crate::NoiseChannelIdentity;
use crate::NoiseRendezvousConnectProvider;
use crate::client::LazyRemoteExecServerClient;
use crate::client::http_client::ReqwestHttpClient;
use crate::client_api::DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT;
use crate::client_api::ExecServerTransportParams;
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
use crate::remote::NoiseRendezvousEnvironmentConfig;
use crate::remote_file_system::RemoteFileSystem;
use crate::remote_process::RemoteProcess;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio_util::task::AbortOnDropHandle;

pub const CODEX_EXEC_SERVER_URL_ENV_VAR: &str = "CODEX_EXEC_SERVER_URL";
pub const CODEX_EXEC_SERVER_NOISE_REGISTRY_URL_ENV_VAR: &str =
    "CODEX_EXEC_SERVER_NOISE_REGISTRY_URL";
pub const CODEX_EXEC_SERVER_NOISE_ENVIRONMENT_ID_ENV_VAR: &str =
    "CODEX_EXEC_SERVER_NOISE_ENVIRONMENT_ID";
pub const CODEX_EXEC_SERVER_NOISE_AUTH_TOKEN_ENV_VAR: &str = "CODEX_EXEC_SERVER_NOISE_AUTH_TOKEN";
pub const CODEX_EXEC_SERVER_NOISE_CHATGPT_ACCOUNT_ID_ENV_VAR: &str =
    "CODEX_EXEC_SERVER_NOISE_CHATGPT_ACCOUNT_ID";

/// The current connection state for one concrete environment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvironmentConnectionState {
    /// An initialized exec-server connection is available.
    Connected,
    /// No initialized exec-server connection is currently available.
    Disconnected,
}

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
/// Remote environments begin connecting when added to the manager. Their
/// filesystem and execution backends share that startup result and reconnect
/// after later disconnects as needed.
#[derive(Debug)]
pub struct EnvironmentManager {
    default_environment: Option<String>,
    pub(super) environments: RwLock<HashMap<String, Arc<Environment>>>,
    local_environment: Option<Arc<Environment>>,
    local_runtime_paths: Option<ExecServerRuntimePaths>,
}

/// Information supplied by the environment owner when a deferred environment is ready.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EnvironmentReadyInfo {
    /// Ordered capability roots selected for this environment.
    pub selected_capability_roots: Vec<SelectedCapabilityRoot>,
}

/// The one-shot capability to complete a deferred environment registration.
#[must_use = "the deferred environment cannot connect until registration is completed"]
pub struct DeferredEnvironmentRegistration {
    completion: oneshot::Sender<Result<(), String>>,
    environment_id: String,
    ready_info: Arc<OnceLock<EnvironmentReadyInfo>>,
}

/// Maximum capability roots accepted from deferred environment ready information.
pub const MAX_SELECTED_CAPABILITY_ROOTS: usize = 256;

pub const LOCAL_ENVIRONMENT_ID: &str = "local";
pub const REMOTE_ENVIRONMENT_ID: &str = "remote";

/// Non-mutating connection status observed by an environment owner.
///
/// Computing this status never starts, waits for, or reconnects an exec-server
/// transport. Already-ready remote environments may receive a fail-fast probe
/// over their existing connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvironmentObservedStatus {
    /// A local environment, or a remote environment whose existing connection answered a probe.
    Ready,
    /// The configured environment has no ready connection and no observed connection failure.
    ///
    /// This includes lazy transports that have never been started and initial startup that has
    /// not finished. Computing status does not start the environment or wait for startup.
    Pending,
    /// A connection attempt, prior connection, or fail-fast status probe observed a failure.
    ///
    /// This does not promise that the failure is terminal: later normal environment use may
    /// recover the connection. Computing status itself does not trigger recovery.
    Disconnected {
        /// Human-readable reason recorded by the failed connection attempt or probe.
        error: String,
    },
}

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
        if let Some(config) = noise_environment_config_from_env()? {
            return Self::from_noise_environment_config(config, local_runtime_paths);
        }
        let provider = environment_provider_from_codex_home(codex_home.as_ref())?;
        Self::from_snapshot(provider.snapshot().await?, local_runtime_paths)
    }

    /// Builds a manager from the legacy environment-variable provider without
    /// reading user config files from `CODEX_HOME`.
    pub async fn from_env(
        local_runtime_paths: Option<ExecServerRuntimePaths>,
    ) -> Result<Self, ExecServerError> {
        if let Some(config) = noise_environment_config_from_env()? {
            return Self::from_noise_environment_config(config, local_runtime_paths);
        }
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

    fn from_noise_environment_config(
        config: NoiseRendezvousEnvironmentConfig,
        local_runtime_paths: Option<ExecServerRuntimePaths>,
    ) -> Result<Self, ExecServerError> {
        let manager = Self {
            default_environment: Some(REMOTE_ENVIRONMENT_ID.to_string()),
            environments: RwLock::new(HashMap::new()),
            local_environment: None,
            local_runtime_paths,
        };
        manager.upsert_noise_environment(
            REMOTE_ENVIRONMENT_ID.to_string(),
            config.connect_provider(),
        )?;
        Ok(manager)
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
            let local_environment = Arc::new(Environment::local(local_runtime_paths));
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
        // The snapshot is valid; start connecting its remote environments in the background.
        for environment in environment_map.values() {
            environment.start_connecting();
        }
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

    /// Returns the current status of one named environment when it is configured.
    pub async fn get_environment_status(
        &self,
        environment_id: &str,
    ) -> Option<EnvironmentObservedStatus> {
        let environment = self.get_environment(environment_id)?;
        Some(environment.status().await)
    }

    /// Adds or replaces a named remote environment without changing the
    /// manager's default environment selection. Uses the default WebSocket
    /// connection timeout when none is provided.
    pub fn upsert_environment(
        &self,
        environment_id: String,
        exec_server_url: String,
        connect_timeout: Option<std::time::Duration>,
    ) -> Result<(), ExecServerError> {
        validate_environment_id(&environment_id)?;
        let exec_server_url = validate_remote_exec_server_url(exec_server_url)?;
        let environment = Arc::new(Environment::remote_with_transport(
            ExecServerTransportParams::websocket_url(
                exec_server_url,
                connect_timeout.unwrap_or(DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT),
            ),
            self.local_runtime_paths.clone(),
        ));
        self.insert_environment(environment_id, environment);
        Ok(())
    }

    /// Adds or replaces a Noise rendezvous environment that will become ready later.
    pub fn register_deferred_noise_environment(
        &self,
        environment_id: String,
        provider: Arc<dyn NoiseRendezvousConnectProvider>,
    ) -> Result<DeferredEnvironmentRegistration, ExecServerError> {
        validate_environment_id(&environment_id)?;
        let identity = noise_channel_identity()?;
        let (completion, readiness) = oneshot::channel();
        let ready_info = Arc::new(OnceLock::new());
        let mut environment = Environment::remote_with_transport(
            ExecServerTransportParams::Deferred(Box::new(crate::client_api::Deferred {
                readiness: readiness.shared(),
                transport: ExecServerTransportParams::NoiseRendezvous { provider, identity },
            })),
            self.local_runtime_paths.clone(),
        );
        environment.ready_info = Some(Arc::clone(&ready_info));
        let environment = Arc::new(environment);
        self.insert_environment(environment_id.clone(), environment);
        Ok(DeferredEnvironmentRegistration {
            completion,
            environment_id,
            ready_info,
        })
    }

    /// Adds or replaces a named remote environment that connects through an
    /// authenticated, end-to-end encrypted rendezvous stream.
    ///
    /// The provider is retained so every reconnect obtains fresh authorization.
    /// This transport never falls back to the URL-only remote environment path.
    pub fn upsert_noise_environment(
        &self,
        environment_id: String,
        provider: Arc<dyn NoiseRendezvousConnectProvider>,
    ) -> Result<(), ExecServerError> {
        validate_environment_id(&environment_id)?;
        let identity = noise_channel_identity()?;
        let environment = Arc::new(Environment::remote_with_transport(
            ExecServerTransportParams::NoiseRendezvous { provider, identity },
            self.local_runtime_paths.clone(),
        ));
        self.insert_environment(environment_id, environment);
        Ok(())
    }

    fn insert_environment(&self, environment_id: String, environment: Arc<Environment>) {
        self.environments
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(environment_id, Arc::clone(&environment));
        environment.start_connecting();
    }
}

impl DeferredEnvironmentRegistration {
    /// Completes provisioning with ready information or a terminal error message.
    pub fn complete(
        self,
        result: Result<EnvironmentReadyInfo, String>,
    ) -> Result<(), ExecServerError> {
        let result = match result {
            Ok(ready_info) => {
                if ready_info.selected_capability_roots.len() > MAX_SELECTED_CAPABILITY_ROOTS {
                    let error = ExecServerError::Protocol(format!(
                        "environment ready info contains more than {MAX_SELECTED_CAPABILITY_ROOTS} selected capability roots"
                    ));
                    let _ = self.completion.send(Err(error.to_string()));
                    return Err(error);
                }
                let mut root_ids =
                    HashSet::with_capacity(ready_info.selected_capability_roots.len());
                for root in &ready_info.selected_capability_roots {
                    let CapabilityRootLocation::Environment { environment_id, .. } = &root.location;
                    if root.id.trim().is_empty()
                        || environment_id != &self.environment_id
                        || !root_ids.insert(root.id.as_str())
                    {
                        let error = ExecServerError::Protocol(format!(
                            "selected capability roots must have unique non-empty IDs and belong to environment `{}`",
                            self.environment_id
                        ));
                        let _ = self.completion.send(Err(error.to_string()));
                        return Err(error);
                    }
                }
                if self.ready_info.set(ready_info).is_err() {
                    let error = ExecServerError::Protocol(
                        "deferred environment ready info was already set".to_string(),
                    );
                    let _ = self.completion.send(Err(error.to_string()));
                    return Err(error);
                }
                Ok(())
            }
            Err(message) => Err(message),
        };
        self.completion.send(result).map_err(|_| {
            ExecServerError::Disconnected("deferred environment registration is inactive".into())
        })
    }
}

fn noise_channel_identity() -> Result<NoiseChannelIdentity, ExecServerError> {
    NoiseChannelIdentity::generate().map_err(|error| {
        ExecServerError::Protocol(format!(
            "failed to generate Noise harness identity: {error}"
        ))
    })
}

fn validate_environment_id(environment_id: &str) -> Result<(), ExecServerError> {
    if environment_id.is_empty() {
        return Err(ExecServerError::Protocol(
            "environment id cannot be empty".to_string(),
        ));
    }
    Ok(())
}

fn validate_remote_exec_server_url(exec_server_url: String) -> Result<String, ExecServerError> {
    let (exec_server_url, disabled) = normalize_exec_server_url(Some(exec_server_url));
    if disabled {
        return Err(ExecServerError::Protocol(
            "remote environment cannot use disabled exec-server url".to_string(),
        ));
    }
    exec_server_url.ok_or_else(|| {
        ExecServerError::Protocol("remote environment requires an exec-server url".to_string())
    })
}

fn noise_environment_config_from_env()
-> Result<Option<NoiseRendezvousEnvironmentConfig>, ExecServerError> {
    noise_environment_config_from_values(
        optional_environment_value(CODEX_EXEC_SERVER_NOISE_REGISTRY_URL_ENV_VAR),
        optional_environment_value(CODEX_EXEC_SERVER_NOISE_ENVIRONMENT_ID_ENV_VAR),
        optional_environment_value(CODEX_EXEC_SERVER_NOISE_AUTH_TOKEN_ENV_VAR),
        optional_environment_value(CODEX_EXEC_SERVER_NOISE_CHATGPT_ACCOUNT_ID_ENV_VAR),
    )
}

fn noise_environment_config_from_values(
    registry_url: Option<String>,
    environment_id: Option<String>,
    auth_token: Option<String>,
    chatgpt_account_id: Option<String>,
) -> Result<Option<NoiseRendezvousEnvironmentConfig>, ExecServerError> {
    let (registry_url, environment_id, auth_token) =
        match (registry_url, environment_id, auth_token) {
            (None, None, None) => return Ok(None),
            (Some(registry_url), Some(environment_id), Some(auth_token)) => {
                (registry_url, environment_id, auth_token)
            }
            _ => {
                return Err(ExecServerError::EnvironmentRegistryConfig(format!(
                    "Noise environment requires {CODEX_EXEC_SERVER_NOISE_REGISTRY_URL_ENV_VAR}, \
{CODEX_EXEC_SERVER_NOISE_ENVIRONMENT_ID_ENV_VAR}, and \
{CODEX_EXEC_SERVER_NOISE_AUTH_TOKEN_ENV_VAR}"
                )));
            }
        };

    let config = NoiseRendezvousEnvironmentConfig::new(
        registry_url,
        environment_id,
        auth_token,
        chatgpt_account_id,
    )?;
    Ok(Some(config))
}

fn optional_environment_value(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Concrete execution/filesystem environment selected for a session.
///
/// This bundles the selected backend metadata together with the local runtime
/// paths used by filesystem helpers.
#[derive(Clone)]
pub struct Environment {
    remote_client: Option<LazyRemoteExecServerClient>,
    ready_info: Option<Arc<OnceLock<EnvironmentReadyInfo>>>,
    // Dropping the environment stops unfinished background startup work.
    startup_task: Arc<Mutex<Option<AbortOnDropHandle<()>>>>,
    exec_backend: Arc<dyn ExecBackend>,
    filesystem: Arc<dyn ExecutorFileSystem>,
    http_client: Arc<dyn HttpClient>,
    local_runtime_paths: Option<ExecServerRuntimePaths>,
}

impl Environment {
    /// Builds a test-only local environment without configured sandbox helper paths.
    pub fn default_for_tests() -> Self {
        Self {
            remote_client: None,
            ready_info: None,
            startup_task: Arc::new(Mutex::new(None)),
            exec_backend: Arc::new(LocalProcess::default()),
            filesystem: Arc::new(LocalFileSystem::unsandboxed()),
            http_client: Arc::new(ReqwestHttpClient),
            local_runtime_paths: None,
        }
    }
}

impl std::fmt::Debug for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Environment").finish_non_exhaustive()
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
        Self {
            remote_client: None,
            ready_info: None,
            startup_task: Arc::new(Mutex::new(None)),
            exec_backend: Arc::new(LocalProcess::with_local_runtime_paths(
                local_runtime_paths.clone(),
            )),
            filesystem: Arc::new(LocalFileSystem::with_runtime_paths(
                local_runtime_paths.clone(),
            )),
            http_client: Arc::new(ReqwestHttpClient),
            local_runtime_paths: Some(local_runtime_paths),
        }
    }

    pub(crate) fn remote_inner(
        exec_server_url: String,
        local_runtime_paths: Option<ExecServerRuntimePaths>,
    ) -> Self {
        Self::remote_with_transport(
            ExecServerTransportParams::websocket_url(
                exec_server_url,
                DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT,
            ),
            local_runtime_paths,
        )
    }

    pub(crate) fn remote_with_transport(
        remote_transport: ExecServerTransportParams,
        local_runtime_paths: Option<ExecServerRuntimePaths>,
    ) -> Self {
        let client = LazyRemoteExecServerClient::new(remote_transport);
        let exec_backend: Arc<dyn ExecBackend> = Arc::new(RemoteProcess::new(client.clone()));
        let filesystem: Arc<dyn ExecutorFileSystem> =
            Arc::new(RemoteFileSystem::new(client.clone()));

        Self {
            remote_client: Some(client.clone()),
            ready_info: None,
            startup_task: Arc::new(Mutex::new(None)),
            exec_backend,
            filesystem,
            http_client: Arc::new(client),
            local_runtime_paths,
        }
    }

    pub fn is_remote(&self) -> bool {
        self.remote_client.is_some()
    }

    /// Returns capability roots supplied with the deferred environment's ready signal.
    pub fn selected_capability_roots(&self) -> &[SelectedCapabilityRoot] {
        self.ready_info
            .as_ref()
            .and_then(|ready_info| ready_info.get())
            .map_or(&[], |ready_info| {
                ready_info.selected_capability_roots.as_slice()
            })
    }

    /// Subscribes to the current connection state for this remote environment.
    pub fn subscribe_connection_state(
        &self,
    ) -> Option<watch::Receiver<EnvironmentConnectionState>> {
        self.remote_client
            .as_ref()
            .map(LazyRemoteExecServerClient::subscribe_connection_state)
    }

    pub fn local_runtime_paths(&self) -> Option<&ExecServerRuntimePaths> {
        self.local_runtime_paths.as_ref()
    }

    /// Returns environment information from the selected execution/filesystem environment.
    pub async fn info(&self) -> Result<EnvironmentInfo, ExecServerError> {
        match &self.remote_client {
            Some(client) => client.environment_info().await,
            None => Ok(EnvironmentInfo::local()),
        }
    }

    /// Discovers plugin and skill manifests through the environment's high-level discovery API.
    pub async fn discover_capability_roots(
        &self,
        params: CapabilityRootsDiscoverParams,
    ) -> Result<CapabilityRootsDiscoverResponse, ExecServerError> {
        match &self.remote_client {
            Some(client) => client.get().await?.discover_capability_roots(params).await,
            None => crate::discover_capability_roots(self.filesystem.as_ref(), params)
                .await
                .map_err(|error| ExecServerError::Protocol(error.to_string())),
        }
    }

    /// Starts connecting a remote environment without waiting for it.
    /// Requires an active Tokio runtime when background startup is supported.
    pub fn start_connecting(&self) {
        let Some(client) = &self.remote_client else {
            return;
        };
        let mut startup_task = self
            .startup_task
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if startup_task.is_none() {
            *startup_task = client.start_connecting();
        }
    }

    /// Starts the initial connection after an environment is actually selected for use.
    pub(crate) fn start_connecting_for_use(environment: &Arc<Self>) {
        if environment.remote_client.is_none() {
            return;
        }
        let mut startup_task = environment
            .startup_task
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if startup_task.is_none() {
            let environment = Arc::clone(environment);
            *startup_task = Some(AbortOnDropHandle::new(tokio::spawn(async move {
                if let Err(error) = environment.wait_until_ready().await {
                    tracing::debug!(%error, "exec-server environment startup failed");
                }
            })));
        }
    }

    /// Returns whether initial startup has either succeeded or permanently failed.
    pub fn startup_finished(&self) -> bool {
        self.remote_client
            .as_ref()
            .is_none_or(LazyRemoteExecServerClient::startup_finished)
    }

    /// Waits for initial startup. A failed startup is never attempted again.
    pub async fn wait_until_ready(&self) -> Result<(), ExecServerError> {
        match &self.remote_client {
            Some(client) => client.wait_until_ready().await,
            None => Ok(()),
        }
    }

    /// Returns whether the environment can serve a request without waiting or reconnecting.
    pub(crate) fn readiness_result(&self) -> Option<Result<(), ExecServerError>> {
        match &self.remote_client {
            Some(client) => client.readiness_result(),
            None => Some(Ok(())),
        }
    }

    /// Returns the environment's status without starting or recovering it.
    ///
    /// Local environments are always ready. Remote environments with an
    /// already-ready cached connection receive a fail-fast `environment/status`
    /// probe; other remote states are returned from cached connection state
    /// without waiting for startup or recovery.
    pub async fn status(&self) -> EnvironmentObservedStatus {
        match &self.remote_client {
            Some(client) => client.status().await,
            None => EnvironmentObservedStatus::Ready,
        }
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

    /// Returns a filesystem view that fails instead of starting or waiting for a connection.
    pub fn get_filesystem_without_reconnect(&self) -> Arc<dyn ExecutorFileSystem> {
        match &self.remote_client {
            Some(client) => Arc::new(RemoteFileSystem::new(client.fail_fast())),
            None => Arc::clone(&self.filesystem),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;

    use super::Environment;
    use super::EnvironmentManager;
    use super::EnvironmentObservedStatus;
    use super::LOCAL_ENVIRONMENT_ID;
    use super::REMOTE_ENVIRONMENT_ID;
    use super::noise_environment_config_from_values;
    use crate::ExecServerRuntimePaths;
    use crate::ProcessId;
    use crate::client_api::ExecServerTransportParams;
    use crate::client_api::StdioExecServerCommand;
    use crate::environment_provider::EnvironmentDefault;
    use crate::environment_provider::EnvironmentProviderSnapshot;
    use codex_utils_path_uri::PathUri;
    use pretty_assertions::assert_eq;
    use tokio::net::TcpListener;
    use tokio::time::timeout;

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

    #[test]
    fn local_environment_info_includes_current_directory() {
        let info = super::EnvironmentInfo::local();

        assert_eq!(
            info.cwd,
            Some(
                PathUri::from_host_native_path(std::env::current_dir().expect("current directory"))
                    .expect("cwd URI")
            )
        );
    }

    #[tokio::test]
    async fn noise_environment_config_selects_remote_as_default() {
        let config = noise_environment_config_from_values(
            Some("http://registry.example/api".to_string()),
            Some("environment-requested".to_string()),
            Some("registry-token".to_string()),
            Some("workspace-123".to_string()),
        )
        .expect("parse noise environment configuration")
        .expect("noise environment configuration");

        let manager = EnvironmentManager::from_noise_environment_config(
            config, /*local_runtime_paths*/ None,
        )
        .expect("build environment manager");

        assert_eq!(
            manager.default_environment_id(),
            Some(REMOTE_ENVIRONMENT_ID)
        );
        assert!(
            manager
                .default_environment()
                .expect("remote environment")
                .is_remote()
        );
        assert_local_environment_unavailable(&manager);
    }

    #[tokio::test]
    async fn create_local_environment_does_not_connect() {
        let environment = Environment::create(/*exec_server_url*/ None, test_runtime_paths())
            .expect("create environment");

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
    async fn environment_manager_creates_remote_environment_for_url() {
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
            /*exec_server_url*/ None,
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
            .upsert_environment(
                "executor-a".to_string(),
                "ws://127.0.0.1:8765".to_string(),
                /*connect_timeout*/ None,
            )
            .expect("remote environment");
        let first = manager
            .get_environment("executor-a")
            .expect("first remote environment");
        assert!(first.is_remote());
        assert_eq!(manager.default_environment_id(), None);

        manager
            .upsert_environment(
                "executor-a".to_string(),
                "ws://127.0.0.1:9876".to_string(),
                /*connect_timeout*/ None,
            )
            .expect("updated remote environment");
        let second = manager
            .get_environment("executor-a")
            .expect("second remote environment");
        assert!(second.is_remote());
        assert!(!Arc::ptr_eq(&first, &second));
    }

    #[tokio::test]
    async fn environment_manager_starts_remote_environment_when_upserted() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind websocket listener");
        let manager = EnvironmentManager::without_environments();

        manager
            .upsert_environment(
                "executor-a".to_string(),
                format!("ws://{}", listener.local_addr().expect("listener address")),
                /*connect_timeout*/ None,
            )
            .expect("remote environment");

        timeout(Duration::from_secs(5), listener.accept())
            .await
            .expect("environment should start connecting when registered")
            .expect("accept connection");
    }

    #[tokio::test]
    async fn environment_status_keeps_stdio_environment_pending() {
        let environment = Environment::remote_with_transport(
            ExecServerTransportParams::StdioCommand {
                command: StdioExecServerCommand {
                    program: "codex-missing-exec-server-for-test".to_string(),
                    args: Vec::new(),
                    env: HashMap::new(),
                    cwd: None,
                },
                initialize_timeout: Duration::from_secs(1),
            },
            /*local_runtime_paths*/ None,
        );

        assert_eq!(
            environment.status().await,
            EnvironmentObservedStatus::Pending
        );
        assert!(!environment.startup_finished());
    }

    #[tokio::test]
    async fn environment_manager_leaves_stdio_environment_lazy() {
        let environment = Environment::remote_with_transport(
            ExecServerTransportParams::StdioCommand {
                command: StdioExecServerCommand {
                    program: "codex-missing-exec-server-for-test".to_string(),
                    args: Vec::new(),
                    env: HashMap::new(),
                    cwd: None,
                },
                initialize_timeout: Duration::from_secs(1),
            },
            /*local_runtime_paths*/ None,
        );
        let manager = EnvironmentManager::from_snapshot(
            EnvironmentProviderSnapshot {
                environments: vec![("stdio".to_string(), environment)],
                default: EnvironmentDefault::Disabled,
                include_local: false,
            },
            /*local_runtime_paths*/ None,
        )
        .expect("environment manager");
        let environment = manager.get_environment("stdio").expect("stdio environment");

        assert!(!environment.startup_finished());
        assert!(environment.wait_until_ready().await.is_err());
        assert!(environment.startup_finished());
    }

    #[tokio::test]
    async fn selected_capability_inspection_keeps_stdio_environment_lazy() {
        use codex_protocol::capabilities::CapabilityRootLocation;
        use codex_protocol::capabilities::SelectedCapabilityRoot;

        let environment = Environment::remote_with_transport(
            ExecServerTransportParams::StdioCommand {
                command: StdioExecServerCommand {
                    program: "codex-missing-exec-server-for-test".to_string(),
                    args: Vec::new(),
                    env: HashMap::new(),
                    cwd: None,
                },
                initialize_timeout: Duration::from_secs(1),
            },
            /*local_runtime_paths*/ None,
        );
        let manager = EnvironmentManager::from_snapshot(
            EnvironmentProviderSnapshot {
                environments: vec![("stdio".to_string(), environment)],
                default: EnvironmentDefault::Disabled,
                include_local: false,
            },
            /*local_runtime_paths*/ None,
        )
        .expect("environment manager");
        let environment = manager.get_environment("stdio").expect("stdio environment");
        let selected_root = SelectedCapabilityRoot {
            id: "demo@1".to_string(),
            location: CapabilityRootLocation::Environment {
                environment_id: "stdio".to_string(),
                path: PathUri::parse("file:///plugins/demo").expect("plugin path URI"),
            },
        };

        let status =
            manager.inspect_selected_capability_roots(std::slice::from_ref(&selected_root));
        assert!(status.ready_roots.is_empty());
        assert_eq!(status.warnings, Vec::<String>::new());
        assert!(!environment.startup_finished());

        let missing_root = SelectedCapabilityRoot {
            id: "missing@1".to_string(),
            location: CapabilityRootLocation::Environment {
                environment_id: "missing".to_string(),
                path: PathUri::parse("file:///plugins/missing").expect("missing plugin path URI"),
            },
        };
        let status = manager.inspect_selected_capability_roots(&[missing_root]);
        assert!(status.ready_roots.is_empty());
        assert_eq!(
            status.warnings,
            vec![
                "selected capability root `missing@1` references unavailable environment `missing`"
                    .to_string()
            ]
        );

        assert!(environment.wait_until_ready().await.is_err());

        let status = manager.inspect_selected_capability_roots(&[selected_root]);
        assert!(status.ready_roots.is_empty());
        assert_eq!(status.warnings.len(), 1);
        assert!(status.warnings[0].contains("environment `stdio` is unavailable"));
    }

    #[tokio::test]
    async fn replacing_environment_stops_its_startup_task() {
        let first_listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind first websocket listener");
        let second_listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind second websocket listener");
        let manager = EnvironmentManager::without_environments();
        manager
            .upsert_environment(
                "executor-a".to_string(),
                format!(
                    "ws://{}",
                    first_listener.local_addr().expect("first listener address")
                ),
                /*connect_timeout*/ None,
            )
            .expect("first remote environment");
        let environment = manager
            .get_environment("executor-a")
            .expect("first remote environment");
        let startup_abort = environment
            .startup_task
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .expect("startup task")
            .abort_handle();
        assert!(!startup_abort.is_finished());
        drop(environment);

        manager
            .upsert_environment(
                "executor-a".to_string(),
                format!(
                    "ws://{}",
                    second_listener
                        .local_addr()
                        .expect("second listener address")
                ),
                /*connect_timeout*/ None,
            )
            .expect("replacement remote environment");

        timeout(Duration::from_secs(1), async {
            while !startup_abort.is_finished() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("replacing the environment should cancel its startup task");
    }

    #[tokio::test]
    async fn environment_manager_rejects_empty_remote_environment_url() {
        let manager = EnvironmentManager::without_environments();

        let err = manager
            .upsert_environment(
                "executor-a".to_string(),
                String::new(),
                /*connect_timeout*/ None,
            )
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
                cwd: PathUri::from_host_native_path(
                    std::env::current_dir().expect("read current dir"),
                )
                .expect("cwd URI"),
                env_policy: None,
                env: Default::default(),
                tty: false,
                pipe_stdin: false,
                arg0: None,
                sandbox: None,
                enforce_managed_network: false,
                managed_network: None,
            })
            .await
            .expect("start process");

        assert_eq!(response.process.process_id().as_str(), "default-env-proc");
    }

    #[tokio::test]
    async fn local_environment_passes_runtime_paths_to_exec_backend() {
        let environment = Environment::local(test_runtime_paths());
        #[cfg(unix)]
        let uri = "file://server/share/checkout";
        #[cfg(windows)]
        let uri = "file:///usr/local/checkout";
        let sandbox_cwd = PathUri::parse(uri).expect("non-native sandbox cwd URI");
        let source = sandbox_cwd
            .to_abs_path()
            .expect_err("sandbox cwd should not be native to this host");
        let sandbox = crate::FileSystemSandboxContext::from_permission_profile_with_cwd(
            codex_protocol::models::PermissionProfile::workspace_write(),
            sandbox_cwd.clone(),
        );

        let result = environment
            .get_exec_backend()
            .start(crate::ExecParams {
                process_id: ProcessId::from("local-sandbox-proc"),
                argv: vec!["true".to_string()],
                cwd: PathUri::from_host_native_path(
                    std::env::current_dir().expect("read current dir"),
                )
                .expect("cwd URI"),
                env_policy: None,
                env: Default::default(),
                tty: false,
                pipe_stdin: false,
                arg0: None,
                sandbox: Some(sandbox),
                enforce_managed_network: false,
                managed_network: None,
            })
            .await;
        let Err(err) = result else {
            panic!("sandbox cwd should be rejected after resolving runtime paths");
        };

        assert_eq!(
            err.to_string(),
            format!(
                "exec-server rejected request (-32602): sandbox cwd URI `{sandbox_cwd}` is not valid on this exec-server host: {source}"
            )
        );
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
