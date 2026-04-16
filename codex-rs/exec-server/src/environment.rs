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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExplicitEnvironmentId {
    Local,
    Remote,
}

/// Lazily creates and caches the active environment for a session.
///
/// The manager keeps the session's environment selection stable so subagents
/// and follow-up turns preserve an explicit disabled state.
#[derive(Debug)]
pub struct EnvironmentManager {
    exec_server_url: Option<String>,
    local_runtime_paths: Option<ExecServerRuntimePaths>,
    disabled: bool,
    current_environment: OnceCell<Option<Arc<Environment>>>,
    local_environment: OnceCell<Arc<Environment>>,
    remote_environment: OnceCell<Arc<Environment>>,
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
        let (exec_server_url, disabled) = normalize_exec_server_url(exec_server_url);
        Self {
            exec_server_url,
            local_runtime_paths,
            disabled,
            current_environment: OnceCell::new(),
            local_environment: OnceCell::new(),
            remote_environment: OnceCell::new(),
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
            Some(environment) => Self {
                exec_server_url: environment.exec_server_url().map(str::to_owned),
                local_runtime_paths: environment.local_runtime_paths().cloned(),
                disabled: false,
                current_environment: OnceCell::new(),
                local_environment: OnceCell::new(),
                remote_environment: OnceCell::new(),
            },
            None => Self {
                exec_server_url: None,
                local_runtime_paths: None,
                disabled: true,
                current_environment: OnceCell::new(),
                local_environment: OnceCell::new(),
                remote_environment: OnceCell::new(),
            },
        }
    }

    /// Returns the remote exec-server URL when one is configured.
    pub fn exec_server_url(&self) -> Option<&str> {
        self.exec_server_url.as_deref()
    }

    /// Returns true when this manager is configured to use a remote exec server.
    pub fn is_remote(&self) -> bool {
        self.exec_server_url.is_some()
    }

    /// Returns the cached environment, creating it on first access.
    pub async fn current(&self) -> Result<Option<Arc<Environment>>, ExecServerError> {
        self.current_environment
            .get_or_try_init(|| async {
                if self.disabled {
                    Ok(None)
                } else {
                    Ok(Some(Arc::new(
                        Environment::create_with_runtime_paths(
                            self.exec_server_url.clone(),
                            self.local_runtime_paths.clone(),
                        )
                        .await?,
                    )))
                }
            })
            .await
            .map(Option::as_ref)
            .map(std::option::Option::<&Arc<Environment>>::cloned)
    }

    pub async fn environment(
        &self,
        environment_id: Option<&str>,
    ) -> Result<Option<Arc<Environment>>, ExecServerError> {
        match parse_environment_id(environment_id)? {
            None => self.current().await,
            Some(ExplicitEnvironmentId::Local) => self.local_environment().await.map(Some),
            Some(ExplicitEnvironmentId::Remote) => self.remote_environment().await.map(Some),
        }
    }

    async fn local_environment(&self) -> Result<Arc<Environment>, ExecServerError> {
        if self.disabled {
            return Err(ExecServerError::Protocol(
                "environments are disabled for this session".to_string(),
            ));
        }

        self.local_environment
            .get_or_try_init(|| async {
                Environment::create_with_runtime_paths(
                    /*exec_server_url*/ None,
                    self.local_runtime_paths.clone(),
                )
                .await
                .map(Arc::new)
            })
            .await
            .map(Arc::clone)
    }

    async fn remote_environment(&self) -> Result<Arc<Environment>, ExecServerError> {
        if self.disabled {
            return Err(ExecServerError::Protocol(
                "environments are disabled for this session".to_string(),
            ));
        }
        let Some(exec_server_url) = &self.exec_server_url else {
            return Err(ExecServerError::Protocol(
                "remote environment is not configured".to_string(),
            ));
        };

        self.remote_environment
            .get_or_try_init(|| async {
                Environment::create_with_runtime_paths(
                    Some(exec_server_url.clone()),
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

fn parse_environment_id(
    environment_id: Option<&str>,
) -> Result<Option<ExplicitEnvironmentId>, ExecServerError> {
    match environment_id.map(str::trim) {
        None | Some("") => Ok(None),
        Some(environment_id) if environment_id.eq_ignore_ascii_case(LOCAL_ENVIRONMENT_ID) => {
            Ok(Some(ExplicitEnvironmentId::Local))
        }
        Some(environment_id) if environment_id.eq_ignore_ascii_case(REMOTE_ENVIRONMENT_ID) => {
            Ok(Some(ExplicitEnvironmentId::Remote))
        }
        Some(environment_id) => Err(ExecServerError::Protocol(format!(
            "unknown environment id: {environment_id}"
        ))),
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

        assert!(!manager.disabled);
        assert_eq!(manager.exec_server_url(), None);
        assert!(!manager.is_remote());
    }

    #[test]
    fn environment_manager_treats_none_value_as_disabled() {
        let manager = EnvironmentManager::new(Some("none".to_string()));

        assert!(manager.disabled);
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
        assert_eq!(
            EnvironmentManager::from_environment(Some(&environment)).local_runtime_paths,
            Some(runtime_paths)
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
            "exec-server protocol error: remote environment is not configured"
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
