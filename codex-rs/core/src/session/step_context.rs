use std::sync::Arc;

use crate::agents_md::LoadedAgentsMd;
use crate::environment_selection::TurnEnvironmentSnapshot;
use crate::session::McpRuntimeSnapshot;
use crate::session::turn_context::TurnContext;
use codex_exec_server::ResolvedSelectedCapabilityRoot;
use codex_mcp::ConnectorRuntimeSnapshot;
use codex_mcp::ToolInfo;

#[derive(Debug)]
pub(crate) enum ConnectorRuntimeCapture {
    /// Preserve the legacy behavior where every read observes the live Codex Apps client/cache.
    LegacyLive,
    /// Use exactly this committed state. `None` means no Codex Apps tools were committed at capture.
    Snapshot(Option<Arc<ConnectorRuntimeSnapshot>>),
}

/// Request-scoped state that may change between model sampling requests.
#[derive(Debug)]
pub(crate) struct StepContext {
    pub(crate) turn: Arc<TurnContext>,
    pub(crate) environments: TurnEnvironmentSnapshot,
    /// Capability roots bound to ready environments in this exact step.
    pub(crate) selected_capability_roots: Vec<ResolvedSelectedCapabilityRoot>,
    /// The exact MCP config and manager used to advertise and execute tools for this step.
    pub(crate) mcp: Arc<McpRuntimeSnapshot>,
    /// The committed hosted-connector state captured for this step.
    ///
    /// The snapshot variant is used only by the runtime-state refactor. Custom MCP servers remain
    /// live in both modes.
    pub(crate) connector_runtime: ConnectorRuntimeCapture,
    /// The canonical AGENTS.md value observed with this environment snapshot.
    pub(crate) loaded_agents_md: Option<Arc<LoadedAgentsMd>>,
}

impl StepContext {
    pub(crate) fn new(
        turn: Arc<TurnContext>,
        environments: TurnEnvironmentSnapshot,
        selected_capability_roots: Vec<ResolvedSelectedCapabilityRoot>,
        mcp: Arc<McpRuntimeSnapshot>,
        connector_runtime: ConnectorRuntimeCapture,
        loaded_agents_md: Option<Arc<LoadedAgentsMd>>,
    ) -> Self {
        Self {
            turn,
            environments,
            selected_capability_roots,
            mcp,
            connector_runtime,
            loaded_agents_md,
        }
    }

    /// Returns tools from the exact hosted-connector snapshot captured for this step while leaving
    /// unrelated MCP servers on their normal live path.
    pub(crate) async fn mcp_tools(&self) -> Vec<ToolInfo> {
        match &self.connector_runtime {
            ConnectorRuntimeCapture::LegacyLive => self.mcp.manager().list_all_tools().await,
            ConnectorRuntimeCapture::Snapshot(snapshot) => {
                self.mcp
                    .manager()
                    .list_all_tools_with_connector_runtime_snapshot(snapshot.as_deref())
                    .await
            }
        }
    }
}
