//! One atomically publishable generation of MCP connections.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use crate::McpAuthStatusEntry;
use crate::codex_apps::CodexAppsToolsCacheContext;
use crate::codex_apps::CodexAppsToolsCacheKey;
use crate::connection_manager::emit_update;
use crate::connection_manager::mcp_init_error_display;
use crate::elicitation::ElicitationRequestManager;
use crate::elicitation::ElicitationReviewerHandle;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::mcp::ToolPluginProvenance;
use crate::rmcp_client::AsyncManagedClient;
use crate::rmcp_client::StartupOutcomeError;
use crate::runtime::McpRuntimeContext;
use crate::server::EffectiveMcpServer;
use crate::server::McpServerMetadata;
use async_channel::Sender;
use codex_config::McpServerTransportConfig;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_login::CodexAuth;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::McpStartupCompleteEvent;
use codex_protocol::protocol::McpStartupFailure;
use codex_protocol::protocol::McpStartupStatus;
use codex_protocol::protocol::McpStartupUpdateEvent;
use rmcp::model::ElicitationCapability;
use tokio::sync::Notify;
use tokio::task::JoinHandle;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

/// Everything needed to start one complete generation of MCP connections.
pub struct McpConnectionStartParams {
    pub mcp_servers: HashMap<String, EffectiveMcpServer>,
    pub store_mode: OAuthCredentialsStoreMode,
    pub auth_entries: HashMap<String, McpAuthStatusEntry>,
    pub approval_policy: AskForApproval,
    pub submit_id: String,
    pub tx_event: Sender<Event>,
    pub permission_profile: PermissionProfile,
    pub runtime_context: McpRuntimeContext,
    pub codex_home: PathBuf,
    pub codex_apps_tools_cache_key: CodexAppsToolsCacheKey,
    pub host_owned_codex_apps_enabled: bool,
    pub prefix_mcp_tool_names: bool,
    pub client_elicitation_capability: ElicitationCapability,
    pub tool_plugin_provenance: ToolPluginProvenance,
    pub auth: Option<CodexAuth>,
    pub elicitation_reviewer: Option<ElicitationReviewerHandle>,
}

pub(crate) struct ConnectionGeneration {
    pub(crate) clients: HashMap<String, AsyncManagedClient>,
    pub(crate) server_metadata: HashMap<String, McpServerMetadata>,
    pub(crate) tool_plugin_provenance: Arc<ToolPluginProvenance>,
    pub(crate) host_owned_codex_apps_enabled: bool,
    pub(crate) prefix_mcp_tool_names: bool,
    startup_cancellation_token: CancellationToken,
    startup_completion: Arc<StdMutex<Option<JoinHandle<()>>>>,
    active_operations: AtomicUsize,
    idle: Notify,
}

impl ConnectionGeneration {
    pub(crate) fn empty(prefix_mcp_tool_names: bool) -> Self {
        Self {
            clients: HashMap::new(),
            server_metadata: HashMap::new(),
            tool_plugin_provenance: Arc::new(ToolPluginProvenance::default()),
            host_owned_codex_apps_enabled: false,
            prefix_mcp_tool_names,
            startup_cancellation_token: CancellationToken::new(),
            startup_completion: Arc::new(StdMutex::new(None)),
            active_operations: AtomicUsize::new(0),
            idle: Notify::new(),
        }
    }

    pub(crate) async fn start(
        params: McpConnectionStartParams,
        elicitation_requests: ElicitationRequestManager,
        startup_cancellation_token: CancellationToken,
    ) -> Self {
        let McpConnectionStartParams {
            mcp_servers,
            store_mode,
            auth_entries,
            approval_policy: _,
            submit_id,
            tx_event,
            permission_profile: _,
            runtime_context,
            codex_home,
            codex_apps_tools_cache_key,
            host_owned_codex_apps_enabled,
            prefix_mcp_tool_names,
            client_elicitation_capability,
            tool_plugin_provenance,
            auth,
            elicitation_reviewer: _,
        } = params;
        let mut clients = HashMap::new();
        let mut server_metadata = HashMap::new();
        let mut join_set = JoinSet::new();
        let tool_plugin_provenance = Arc::new(tool_plugin_provenance);
        let startup_submit_id = submit_id.clone();
        let codex_apps_auth_provider = auth
            .as_ref()
            .filter(|auth| auth.uses_codex_backend())
            .map(codex_model_provider::auth_provider_from_auth);

        for (server_name, server) in mcp_servers
            .into_iter()
            .filter(|(_, server)| server.enabled())
        {
            server_metadata.insert(server_name.clone(), McpServerMetadata::from(&server));
            let cancel_token = startup_cancellation_token.child_token();
            let _ = emit_update(
                startup_submit_id.as_str(),
                &tx_event,
                McpStartupUpdateEvent {
                    server: server_name.clone(),
                    status: McpStartupStatus::Starting,
                },
            )
            .await;
            let codex_apps_tools_cache_context = if server_name == CODEX_APPS_MCP_SERVER_NAME {
                Some(CodexAppsToolsCacheContext {
                    codex_home: codex_home.clone(),
                    user_key: codex_apps_tools_cache_key.clone(),
                })
            } else {
                None
            };
            let uses_env_bearer_token =
                server
                    .configured_config()
                    .is_some_and(|config| match &config.transport {
                        McpServerTransportConfig::StreamableHttp {
                            bearer_token_env_var,
                            ..
                        } => bearer_token_env_var.is_some(),
                        McpServerTransportConfig::Stdio { .. } => false,
                    });
            let runtime_auth_provider =
                if server_name == CODEX_APPS_MCP_SERVER_NAME && !uses_env_bearer_token {
                    codex_apps_auth_provider.clone()
                } else {
                    None
                };
            let async_managed_client = AsyncManagedClient::new(
                server_name.clone(),
                server,
                store_mode,
                cancel_token.clone(),
                tx_event.clone(),
                elicitation_requests.clone(),
                codex_apps_tools_cache_context,
                Arc::clone(&tool_plugin_provenance),
                runtime_context.clone(),
                runtime_auth_provider,
                client_elicitation_capability.clone(),
            );
            clients.insert(server_name.clone(), async_managed_client.clone());
            let tx_event = tx_event.clone();
            let submit_id = startup_submit_id.clone();
            let auth_entry = auth_entries.get(&server_name).cloned();
            join_set.spawn(async move {
                let mut outcome = async_managed_client.client().await;
                if cancel_token.is_cancelled() {
                    outcome = Err(StartupOutcomeError::Cancelled);
                }
                let status = match &outcome {
                    Ok(_) => McpStartupStatus::Ready,
                    Err(StartupOutcomeError::Cancelled) => McpStartupStatus::Cancelled,
                    Err(error) => McpStartupStatus::Failed {
                        error: mcp_init_error_display(
                            server_name.as_str(),
                            auth_entry.as_ref(),
                            error,
                        ),
                    },
                };

                let _ = emit_update(
                    submit_id.as_str(),
                    &tx_event,
                    McpStartupUpdateEvent {
                        server: server_name.clone(),
                        status,
                    },
                )
                .await;

                (server_name, outcome)
            });
        }

        let startup_completion = tokio::spawn(async move {
            let outcomes = join_set.join_all().await;
            let mut summary = McpStartupCompleteEvent::default();
            for (server_name, outcome) in outcomes {
                match outcome {
                    Ok(_) => summary.ready.push(server_name),
                    Err(StartupOutcomeError::Cancelled) => summary.cancelled.push(server_name),
                    Err(StartupOutcomeError::Failed { error }) => {
                        summary.failed.push(McpStartupFailure {
                            server: server_name,
                            error,
                        });
                    }
                }
            }
            let _ = tx_event
                .send(Event {
                    id: startup_submit_id,
                    msg: EventMsg::McpStartupComplete(summary),
                })
                .await;
        });

        Self {
            clients,
            server_metadata,
            tool_plugin_provenance,
            host_owned_codex_apps_enabled,
            prefix_mcp_tool_names,
            startup_cancellation_token,
            startup_completion: Arc::new(StdMutex::new(Some(startup_completion))),
            active_operations: AtomicUsize::new(0),
            idle: Notify::new(),
        }
    }

    pub(crate) fn acquire(self: &Arc<Self>) -> ConnectionGenerationLease {
        self.active_operations.fetch_add(1, Ordering::AcqRel);
        ConnectionGenerationLease {
            generation: Arc::clone(self),
        }
    }

    pub(crate) async fn wait_until_idle(&self) {
        loop {
            let idle = self.idle.notified();
            if self.active_operations.load(Ordering::Acquire) == 0 {
                return;
            }
            idle.await;
        }
    }

    pub(crate) fn cancel_startup(&self) {
        self.startup_cancellation_token.cancel();
    }

    pub(crate) async fn shutdown(&self) {
        self.cancel_startup();
        for client in self.clients.values() {
            client.shutdown().await;
        }
        let startup_completion = self
            .startup_completion
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(startup_completion) = startup_completion {
            let _ = startup_completion.await;
        }
    }
}

impl Clone for ConnectionGeneration {
    fn clone(&self) -> Self {
        Self {
            clients: self.clients.clone(),
            server_metadata: self.server_metadata.clone(),
            tool_plugin_provenance: Arc::clone(&self.tool_plugin_provenance),
            host_owned_codex_apps_enabled: self.host_owned_codex_apps_enabled,
            prefix_mcp_tool_names: self.prefix_mcp_tool_names,
            startup_cancellation_token: self.startup_cancellation_token.clone(),
            startup_completion: Arc::clone(&self.startup_completion),
            active_operations: AtomicUsize::new(0),
            idle: Notify::new(),
        }
    }
}

pub(crate) struct ConnectionGenerationLease {
    generation: Arc<ConnectionGeneration>,
}

impl std::ops::Deref for ConnectionGenerationLease {
    type Target = ConnectionGeneration;

    fn deref(&self) -> &Self::Target {
        &self.generation
    }
}

impl Drop for ConnectionGenerationLease {
    fn drop(&mut self) {
        if self
            .generation
            .active_operations
            .fetch_sub(1, Ordering::AcqRel)
            == 1
        {
            self.generation.idle.notify_waiters();
        }
    }
}

impl Drop for ConnectionGeneration {
    fn drop(&mut self) {
        self.startup_cancellation_token.cancel();
    }
}
