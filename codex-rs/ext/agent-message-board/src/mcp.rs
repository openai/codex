//! Publishes the existing in-memory launcher through ordinary MCP discovery.

use codex_config::AppToolApproval;
use codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID;
use codex_config::McpServerConfig;
use codex_config::McpServerTransportConfig;
use codex_extension_api::CatalogRefreshInput;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributor;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStopInput;
use codex_extension_api::TurnLifecycleContributor;
use codex_message_board::MessageBoardHost;
use codex_message_board_mcp::LocalServer;
use codex_message_board_mcp::launch_in_memory;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;
use tokio::sync::OnceCell;

/// Shared launchers opened by the existing board factory when the MCP flag is enabled.
#[derive(Default)]
pub struct McpBoards {
    servers: Mutex<HashMap<SessionId, Weak<OnceCell<LocalServer>>>>,
}
struct Binding(Arc<OnceCell<LocalServer>>);
struct PendingMcpRefresh;

impl McpBoards {
    pub async fn open(
        &self,
        store: &ExtensionData,
        tree: SessionId,
        caller: ThreadId,
        host: Arc<dyn MessageBoardHost>,
    ) -> Result<()> {
        let slot = {
            let mut servers = self
                .servers
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            servers.retain(|_, server| server.strong_count() > 0);
            let entry = servers.entry(tree).or_default();
            entry.upgrade().unwrap_or_else(|| {
                let slot = Arc::new(OnceCell::new());
                *entry = Arc::downgrade(&slot);
                slot
            })
        };
        let server = slot
            .get_or_try_init(launch_in_memory)
            .await
            .map_err(|error| CodexErr::Io(std::io::Error::other(error)))?;
        server
            .bind_agent(tree, caller, host)
            .await
            .map_err(|error| CodexErr::Io(std::io::Error::other(error)))?;
        store.insert(Binding(slot));
        store.insert(PendingMcpRefresh);
        Ok(())
    }
}

impl<C: Sync> ThreadLifecycleContributor<C> for McpBoards {
    fn on_thread_stop<'a>(&'a self, input: ThreadStopInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            input.thread_store.remove::<Binding>();
        })
    }
}

impl TurnLifecycleContributor for McpBoards {
    fn refresh_catalog_for_step<'a>(
        &'a self,
        input: CatalogRefreshInput<'a>,
    ) -> ExtensionFuture<'a, bool> {
        // Initial MCP projection precedes the board factory's startup hook.
        Box::pin(async move { input.thread_store.remove::<PendingMcpRefresh>().is_some() })
    }
}

impl<C: Sync> McpServerContributor<C> for McpBoards {
    fn id(&self) -> &'static str {
        "agent_message_board_mcp"
    }

    fn contribute<'a>(
        &'a self,
        context: McpServerContributionContext<'a, C>,
    ) -> ExtensionFuture<'a, Vec<McpServerContribution>> {
        Box::pin(async move {
            let Some(binding) = context
                .thread_store()
                .and_then(ExtensionData::get::<Binding>)
            else {
                return Vec::new();
            };
            let Some(server) = binding.0.get() else {
                return Vec::new();
            };
            let Ok(authorization) = server.authorization().to_str() else {
                return Vec::new();
            };
            vec![McpServerContribution::Set {
                name: "codex_message_board".into(),
                config: Box::new(McpServerConfig {
                    transport: McpServerTransportConfig::StreamableHttp {
                        url: server.url().into(),
                        bearer_token_env_var: None,
                        http_headers: Some([("Authorization".into(), authorization.into())].into()),
                        env_http_headers: None,
                        http_headers_helper: None,
                    },
                    auth: Default::default(),
                    environment_id: DEFAULT_MCP_SERVER_ENVIRONMENT_ID.into(),
                    enabled: true,
                    required: true,
                    startup_readiness: Default::default(),
                    supports_parallel_tool_calls: false,
                    tool_input_schema_max_bytes: None,
                    omit_tools_from: None,
                    disabled_reason: None,
                    startup_timeout_sec: None,
                    tool_timeout_sec: None,
                    default_tools_approval_mode: Some(AppToolApproval::Approve),
                    enabled_tools: None,
                    disabled_tools: None,
                    scopes: None,
                    oauth: None,
                    oauth_resource: None,
                    tools: HashMap::new(),
                }),
            }]
        })
    }
}

/// Registers discovery for launchers created by the board factory.
pub fn install_mcp<C: Sync + 'static>(
    registry: &mut ExtensionRegistryBuilder<C>,
) -> Arc<McpBoards> {
    let boards = Arc::new(McpBoards::default());
    registry.thread_lifecycle_contributor(boards.clone());
    registry.turn_lifecycle_contributor(boards.clone());
    registry.mcp_server_contributor(boards.clone());
    boards
}
