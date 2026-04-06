use std::sync::Arc;

use tokio::sync::OnceCell;

use crate::ExecServerClient;
use crate::ExecServerError;
use crate::RemoteExecServerConnectArgs;
use crate::file_system::ExecutorFileSystem;
use crate::local_file_system::LocalFileSystem;
use crate::local_process::LocalProcess;
use crate::process::ExecBackend;
use crate::remote_file_system::RemoteFileSystem;
use crate::remote_process::RemoteProcess;

pub const CODEX_EXEC_SERVER_URL_ENV_VAR: &str = "CODEX_EXEC_SERVER_URL";

/// Lazily creates and caches the active environment for a session.
///
/// The manager keeps the session's environment selection stable so subagents
/// and follow-up turns preserve an explicit disabled state.
#[derive(Debug)]
pub struct EnvironmentManager {
    exec_server_url: Option<String>,
    disabled: bool,
    current_environment: OnceCell<Option<Arc<Environment>>>,
}

impl Default for EnvironmentManager {
    fn default() -> Self {
        Self::new(/*exec_server_url*/ None)
    }
}

impl EnvironmentManager {
    /// Builds a manager from the raw `CODEX_EXEC_SERVER_URL` value.
    pub fn new(exec_server_url: Option<String>) -> Self {
        let (exec_server_url, disabled) = normalize_exec_server_url(exec_server_url);
        Self {
            exec_server_url,
            disabled,
            current_environment: OnceCell::new(),
        }
    }

    /// Builds a manager from process environment variables.
    pub fn from_env() -> Self {
        Self::new(std::env::var(CODEX_EXEC_SERVER_URL_ENV_VAR).ok())
    }

    /// Builds a manager from the currently selected environment, or from the
    /// disabled mode when no environment is available.
    pub fn from_environment(environment: Option<&Environment>) -> Self {
        match environment {
            Some(environment) => Self {
                exec_server_url: environment.exec_server_url().map(str::to_owned),
                disabled: false,
                current_environment: OnceCell::new(),
            },
            None => Self {
                exec_server_url: None,
                disabled: true,
                current_environment: OnceCell::new(),
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
                        Environment::create(self.exec_server_url.clone()).await?,
                    )))
                }
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
    remote_exec_server_client: Option<ExecServerClient>,
    exec_backend: Arc<dyn ExecBackend>,
}

impl Default for Environment {
    fn default() -> Self {
        let local_process = LocalProcess::default();
        if let Err(err) = local_process.initialize() {
            panic!("default local process initialization should succeed: {err:?}");
        }
        if let Err(err) = local_process.initialized() {
            panic!("default local process should accept initialized notification: {err}");
        }

        Self {
            exec_server_url: None,
            remote_exec_server_client: None,
            exec_backend: Arc::new(local_process),
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
                })
                .await?,
            )
        } else {
            None
        };

        let exec_backend: Arc<dyn ExecBackend> = match remote_exec_server_client.clone() {
            Some(client) => Arc::new(RemoteProcess::new(client)),
            None if exec_server_url.is_some() => {
                return Err(ExecServerError::Protocol(
                    "remote mode should have an exec-server client".to_string(),
                ));
            }
            None => {
                let local_process = LocalProcess::default();
                local_process
                    .initialize()
                    .map_err(|err| ExecServerError::Protocol(err.message))?;
                local_process
                    .initialized()
                    .map_err(ExecServerError::Protocol)?;
                Arc::new(local_process)
            }
        };

        Ok(Self {
            exec_server_url,
            remote_exec_server_client,
            exec_backend,
        })
    }

    pub fn is_remote(&self) -> bool {
        self.exec_server_url.is_some()
    }

    /// Returns the remote exec-server URL when this environment is remote.
    pub fn exec_server_url(&self) -> Option<&str> {
        self.exec_server_url.as_deref()
    }

    pub fn get_exec_backend(&self) -> Arc<dyn ExecBackend> {
        Arc::clone(&self.exec_backend)
    }

    pub fn get_filesystem(&self) -> Arc<dyn ExecutorFileSystem> {
        match self.remote_exec_server_client.clone() {
            Some(client) => Arc::new(RemoteFileSystem::new(client)),
            None => Arc::new(LocalFileSystem),
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
                env: Default::default(),
                tty: false,
                arg0: None,
                startup_exec_approval: None,
            })
            .await
            .expect("start process");

        assert_eq!(response.process.process_id().as_str(), "default-env-proc");
    }
    #[tokio::test]
    async fn disabled_environment_rejects_exec_and_filesystem_access() {
        let environment = Environment::create_for_mode(EnvironmentMode::Disabled)
            .await
            .expect("create disabled environment");

        let exec_error = match environment
            .get_exec_backend()
            .start(crate::ExecParams {
                process_id: ProcessId::from("disabled-proc"),
                argv: vec!["true".to_string()],
                cwd: std::env::current_dir().expect("read current dir"),
                env: Default::default(),
                tty: false,
                arg0: None,
                startup_exec_approval: None,
            })
            .await
        {
            Ok(_) => panic!("disabled environment should reject exec"),
            Err(err) => err,
        };
        assert_eq!(
            exec_error.to_string(),
            "exec-server protocol error: environment is disabled; cannot start process `disabled-proc`"
        );

        let path =
            codex_utils_absolute_path::AbsolutePathBuf::try_from(std::env::temp_dir().as_path())
                .expect("temp dir");
        let fs_error = environment
            .get_filesystem()
            .get_metadata(&path)
            .await
            .expect_err("disabled environment should reject filesystem access");
        assert_eq!(fs_error.kind(), io::ErrorKind::Unsupported);
    }
}
