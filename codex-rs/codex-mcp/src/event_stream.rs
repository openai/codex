//! Opens MCP event streams with clients that remain connected after task unloading.

use std::sync::Arc;

use anyhow::Result;
use anyhow::bail;
use codex_api::SharedAuthProvider;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::Environment;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_protocol::mcp::ClientMcpExtensions;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::McpOAuthRefreshMode;
use rmcp::model::ElicitationAction;
use rmcp::model::ElicitationCapability;
use serde_json::Map;
use serde_json::Value;
use tokio::sync::watch;

use crate::CODEX_APPS_MCP_SERVER_NAME;
use crate::EffectiveMcpServer;
use crate::McpEventStream;
use crate::McpProtocolMode;
use crate::McpRuntimeContext;
use crate::rmcp_client::DEFAULT_STARTUP_TIMEOUT;
use crate::rmcp_client::make_rmcp_client;
use crate::rmcp_client::mcp_initialize_request_params;

pub(crate) struct EventStreamConnectionSettings {
    pub server: EffectiveMcpServer,
    pub store_mode: OAuthCredentialsStoreMode,
    pub keyring_backend_kind: AuthKeyringBackendKind,
    pub oauth_refresh_mode: McpOAuthRefreshMode,
    pub runtime_context: McpRuntimeContext,
    pub resolved_environment: std::result::Result<Option<Arc<Environment>>, String>,
    pub auth_provider: Option<SharedAuthProvider>,
    pub auth_manager: Option<Arc<AuthManager>>,
    pub auth: Option<CodexAuth>,
    pub protocol_mode: McpProtocolMode,
    pub client_mcp_extensions: ClientMcpExtensions,
}

/// Opens each event stream with its own MCP client.
/// Owners watch `wait_for_access_change` to cancel streams when access changes.
#[derive(Clone)]
pub struct McpEventStreamOpener {
    pub(crate) connection: Arc<EventStreamConnectionSettings>,
    pub(crate) cancellation_receiver: watch::Receiver<()>,
    pub(crate) cancel_event_streams_on_server_removal: watch::Sender<()>,
}

impl McpEventStreamOpener {
    /// Retains cancellation for this task's subscriptions across MCP runtime replacement.
    pub fn event_stream_cancellation_sender(&self) -> watch::Sender<()> {
        self.cancel_event_streams_on_server_removal.clone()
    }

    /// Creates an MCP client and opens an event stream.
    pub async fn open(
        &self,
        event_name: &str,
        arguments: &Value,
        request_meta: Option<&Map<String, Value>>,
    ) -> Result<McpEventStream> {
        tokio::select! {
            biased;
            () = self.wait_for_access_change() => bail!("event subscription access changed"),
            result = async {
                let connection = &self.connection;
                if let Some(manager) = &connection.auth_manager {
                    let auth = manager.auth().await;
                    if !self.matches_auth(auth.as_ref()) {
                        bail!("event subscription account changed");
                    }
                }

                let startup_timeout = connection.server.config().startup_timeout_sec
                    .unwrap_or(DEFAULT_STARTUP_TIMEOUT);
                let client = Arc::new(tokio::time::timeout(startup_timeout, make_rmcp_client(
                    /*ema_auth*/ None,
                    CODEX_APPS_MCP_SERVER_NAME,
                    connection.server.clone(),
                    connection.store_mode,
                    connection.keyring_backend_kind,
                    connection.oauth_refresh_mode,
                    connection.runtime_context.clone(),
                    connection.resolved_environment.clone(),
                    connection.auth_provider.clone(),
                    connection.protocol_mode,
                )).await??);
                client.initialize(
                    mcp_initialize_request_params(
                        ElicitationCapability::default(),
                        connection.client_mcp_extensions.clone(),
                    ),
                    Some(startup_timeout),
                    Box::new(|_, _| Box::pin(async {
                        Ok(ElicitationResponse {
                            action: ElicitationAction::Decline,
                            content: None,
                            meta: None,
                        })
                    })),
                ).await?;
                McpEventStream::open(
                    client,
                    self.cancellation_receiver.clone(),
                    event_name,
                    arguments,
                    request_meta,
                ).await
            } => result,
        }
    }

    /// Waits for an account change or removal of the event server from the task.
    pub async fn wait_for_access_change(&self) {
        let mut cancellation_receiver = self.cancellation_receiver.clone();
        let auth_change = async {
            let Some(manager) = &self.connection.auth_manager else {
                return std::future::pending().await;
            };
            let mut changes = manager.auth_change_receiver();
            loop {
                if !self.matches_auth(manager.auth_cached().as_ref()) {
                    return;
                }
                if changes.changed().await.is_err() {
                    return;
                }
            }
        };
        tokio::select! {
            Ok(()) = cancellation_receiver.changed() => {},
            () = auth_change => {},
        }
    }

    fn matches_auth(&self, current: Option<&CodexAuth>) -> bool {
        match (self.connection.auth.as_ref(), current) {
            (Some(CodexAuth::AgentIdentity(expected)), Some(CodexAuth::AgentIdentity(current))) => {
                expected.record() == current.record()
            }
            (Some(CodexAuth::AgentIdentity(_)), _) | (_, Some(CodexAuth::AgentIdentity(_))) => {
                false
            }
            (Some(expected), Some(current)) => {
                expected.get_account_id() == current.get_account_id()
                    && expected.get_chatgpt_user_id() == current.get_chatgpt_user_id()
                    && expected.is_workspace_account() == current.is_workspace_account()
                    && expected.is_fedramp_account() == current.is_fedramp_account()
                    && (expected.get_account_id().is_some() || expected == current)
            }
            (None, None) => true,
            _ => false,
        }
    }
}
