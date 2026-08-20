use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;

use super::Environment;
use super::EnvironmentManager;
use super::validate_environment_id;
use crate::ExecServerClient;
use crate::ExecServerClientConnectOptions;
use crate::ExecServerError;
use crate::client::LazyRemoteExecServerClient;
use axum::extract::ws::WebSocket;
use codex_http_client::HttpClientFactory;

impl EnvironmentManager {
    /// Builds a manager around a WebSocket already accepted and authenticated by its host.
    ///
    /// The manager owns client construction, session initialization, and later
    /// recovery. The host only supplies the initial socket and authenticated
    /// replacement sockets through [`Self::replace_accepted_websocket`].
    pub async fn from_accepted_websocket(
        environment_id: String,
        websocket: WebSocket,
        options: ExecServerClientConnectOptions,
        http_client_factory: HttpClientFactory,
    ) -> Result<Self, ExecServerError> {
        validate_environment_id(&environment_id)?;
        let client = ExecServerClient::connect_accepted_websocket(websocket, options).await?;
        let client =
            LazyRemoteExecServerClient::from_connected(client, http_client_factory.clone());
        let environment = Arc::new(Environment::remote_with_client(
            client, /*local_runtime_paths*/ None,
        ));
        Ok(Self {
            default_environment: Some(environment_id.clone()),
            environments: RwLock::new(HashMap::from([(environment_id, environment)])),
            local_environment: None,
            local_runtime_paths: None,
            http_client_factory,
        })
    }

    /// Hands a replacement WebSocket to an existing accepted environment.
    /// Returns after handoff; recovery continues asynchronously.
    pub async fn replace_accepted_websocket(
        &self,
        environment_id: &str,
        websocket: WebSocket,
    ) -> Result<(), ExecServerError> {
        let environment = self.get_environment(environment_id).ok_or_else(|| {
            ExecServerError::Protocol(format!("environment `{environment_id}` is not configured"))
        })?;
        environment
            .remote_client
            .as_ref()
            .ok_or_else(|| {
                ExecServerError::Protocol(
                    "local environment does not have a replaceable exec-server client".to_string(),
                )
            })?
            .replace_accepted_websocket(websocket)
            .await
    }
}
