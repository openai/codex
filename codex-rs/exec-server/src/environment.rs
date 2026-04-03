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

pub trait ExecutorEnvironment: Send + Sync {
    fn get_exec_backend(&self) -> Arc<dyn ExecBackend>;
}

#[derive(Debug, Default)]
pub struct EnvironmentManager {
    exec_server_url: Option<String>,
    current_environment: OnceCell<Arc<Environment>>,
}

impl EnvironmentManager {
    pub fn new(exec_server_url: Option<String>) -> Self {
        Self {
            exec_server_url: normalize_exec_server_url(exec_server_url),
            current_environment: OnceCell::new(),
        }
    }

    pub fn from_env() -> Self {
        Self::new(std::env::var(CODEX_EXEC_SERVER_URL_ENV_VAR).ok())
    }

    pub fn exec_server_url(&self) -> Option<&str> {
        self.exec_server_url.as_deref()
    }

    pub async fn current(&self) -> Result<Arc<Environment>, ExecServerError> {
        self.current_environment
            .get_or_try_init(|| async {
                Ok(Arc::new(
                    Environment::create(self.exec_server_url.clone()).await?,
                ))
            })
            .await
            .map(Arc::clone)
    }
}

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
    pub async fn create(exec_server_url: Option<String>) -> Result<Self, ExecServerError> {
        let exec_server_url = normalize_exec_server_url(exec_server_url);
        let remote_exec_server_client = if let Some(url) = &exec_server_url {
            Some(
                ExecServerClient::connect_websocket(RemoteExecServerConnectArgs {
                    websocket_url: url.clone(),
                    client_name: "codex-environment".to_string(),
                    connect_timeout: std::time::Duration::from_secs(5),
                    initialize_timeout: std::time::Duration::from_secs(5),
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
                let local_process = LocalProcess::default();
                local_process
                    .initialize()
                    .map_err(|err| ExecServerError::Protocol(err.message))?;
                local_process
                    .initialized()
                    .map_err(ExecServerError::Protocol)?;
                Arc::new(local_process)
            };

        Ok(Self {
            exec_server_url,
            remote_exec_server_client,
            exec_backend,
        })
    }

    pub fn exec_server_url(&self) -> Option<&str> {
        self.exec_server_url.as_deref()
    }

    pub fn get_exec_backend(&self) -> Arc<dyn ExecBackend> {
        Arc::clone(&self.exec_backend)
    }

    pub fn get_filesystem(&self) -> Arc<dyn ExecutorFileSystem> {
        if let Some(client) = self.remote_exec_server_client.clone() {
            Arc::new(RemoteFileSystem::new(client))
        } else {
            Arc::new(LocalFileSystem)
        }
    }
}

fn normalize_exec_server_url(exec_server_url: Option<String>) -> Option<String> {
    exec_server_url.and_then(|url| {
        let url = url.trim();
        (!url.is_empty()).then(|| url.to_string())
    })
}

impl ExecutorEnvironment for Environment {
    fn get_exec_backend(&self) -> Arc<dyn ExecBackend> {
        Arc::clone(&self.exec_backend)
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
    async fn create_without_remote_exec_server_url_does_not_connect() {
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
    }

    #[tokio::test]
    async fn environment_manager_current_caches_environment() {
        let manager = EnvironmentManager::new(/*exec_server_url*/ None);

        let first = manager.current().await.expect("get current environment");
        let second = manager.current().await.expect("get current environment");

        assert!(Arc::ptr_eq(&first, &second));
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
                sandbox: None,
            })
            .await
            .expect("start process");

        assert_eq!(response.process.process_id().as_str(), "default-env-proc");
    }
}
