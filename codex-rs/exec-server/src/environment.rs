use std::sync::Arc;

use crate::ExecServerClient;
use crate::ExecServerError;
use crate::RemoteExecServerConnectArgs;
use crate::file_system::ExecutorFileSystem;
use crate::local_file_system::LocalFileSystem;
use crate::local_process::LocalProcess;
use crate::process::ExecProcess;
use crate::remote_file_system::RemoteFileSystem;
use crate::remote_process::RemoteProcess;

pub trait ExecutorEnvironment: Send + Sync {
    fn get_executor(&self) -> Arc<dyn ExecProcess>;
}

#[derive(Clone)]
pub struct Environment {
    experimental_exec_server_url: Option<String>,
    remote_exec_server_client: Option<ExecServerClient>,
    executor: Arc<dyn ExecProcess>,
}

impl Default for Environment {
    fn default() -> Self {
        let local_process = LocalProcess::default();
        local_process
            .initialize()
            .expect("default local process initialization should succeed");
        local_process
            .initialized()
            .expect("default local process should accept initialized notification");

        Self {
            experimental_exec_server_url: None,
            remote_exec_server_client: None,
            executor: Arc::new(local_process),
        }
    }
}

impl std::fmt::Debug for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Environment")
            .field(
                "experimental_exec_server_url",
                &self.experimental_exec_server_url,
            )
            .finish_non_exhaustive()
    }
}

impl Environment {
    pub async fn create(
        experimental_exec_server_url: Option<String>,
    ) -> Result<Self, ExecServerError> {
        let remote_exec_server_client = if let Some(url) = &experimental_exec_server_url {
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

        let executor: Arc<dyn ExecProcess> = if let Some(client) = remote_exec_server_client.clone()
        {
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
            experimental_exec_server_url,
            remote_exec_server_client,
            executor,
        })
    }

    pub fn experimental_exec_server_url(&self) -> Option<&str> {
        self.experimental_exec_server_url.as_deref()
    }

    pub fn remote_exec_server_client(&self) -> Option<ExecServerClient> {
        self.remote_exec_server_client.clone()
    }

    pub fn get_executor(&self) -> Arc<dyn ExecProcess> {
        Arc::clone(&self.executor)
    }

    pub fn get_filesystem(&self) -> Arc<dyn ExecutorFileSystem> {
        if let Some(client) = self.remote_exec_server_client.clone() {
            Arc::new(RemoteFileSystem::new(client))
        } else {
            Arc::new(LocalFileSystem)
        }
    }
}

impl ExecutorEnvironment for Environment {
    fn get_executor(&self) -> Arc<dyn ExecProcess> {
        self.get_executor()
    }
}

#[cfg(test)]
mod tests {
    use super::Environment;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn create_without_remote_exec_server_url_does_not_connect() {
        let environment = Environment::create(None).await.expect("create environment");

        assert_eq!(environment.experimental_exec_server_url(), None);
        assert!(environment.remote_exec_server_client().is_none());
    }

    #[tokio::test]
    async fn default_environment_has_ready_local_executor() {
        let environment = Environment::default();

        let response = environment
            .get_executor()
            .start(crate::ExecParams {
                process_id: "default-env-proc".to_string(),
                argv: vec!["true".to_string()],
                cwd: std::env::current_dir().expect("read current dir"),
                env: Default::default(),
                tty: false,
                arg0: None,
            })
            .await
            .expect("start process");

        assert_eq!(
            response,
            crate::ExecResponse {
                process_id: "default-env-proc".to_string(),
            }
        );
    }
}
