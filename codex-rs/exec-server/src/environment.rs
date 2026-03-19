use crate::ExecServerClient;
use crate::ExecServerError;
use crate::RemoteExecServerConnectArgs;
use crate::file_system::ExecutorFileSystem;
use crate::local_file_system::LocalFileSystem;
use crate::remote_file_system::RemoteFileSystem;
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct Environment {
    experimental_exec_server_url: Option<String>,
    remote_exec_server_client: Option<ExecServerClient>,
}

impl std::fmt::Debug for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Environment")
            .field(
                "experimental_exec_server_url",
                &self.experimental_exec_server_url,
            )
            .field(
                "has_remote_exec_server_client",
                &self.remote_exec_server_client.is_some(),
            )
            .finish()
    }
}

impl Environment {
    pub async fn create(
        experimental_exec_server_url: Option<String>,
    ) -> Result<Self, ExecServerError> {
        let remote_exec_server_client =
            if let Some(websocket_url) = experimental_exec_server_url.as_deref() {
                Some(
                    ExecServerClient::connect_websocket(RemoteExecServerConnectArgs::new(
                        websocket_url.to_string(),
                        "codex-core".to_string(),
                    ))
                    .await?,
                )
            } else {
                None
            };

        Ok(Self {
            experimental_exec_server_url,
            remote_exec_server_client,
        })
    }

    pub fn experimental_exec_server_url(&self) -> Option<&str> {
        self.experimental_exec_server_url.as_deref()
    }

    pub fn remote_exec_server_client(&self) -> Option<&ExecServerClient> {
        self.remote_exec_server_client.as_ref()
    }

    pub fn get_filesystem(&self) -> Arc<dyn ExecutorFileSystem> {
        if let Some(client) = self.remote_exec_server_client.clone() {
            Arc::new(RemoteFileSystem::new(client))
        } else {
            Arc::new(LocalFileSystem)
        }
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
}
