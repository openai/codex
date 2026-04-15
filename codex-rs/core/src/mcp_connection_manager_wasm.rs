use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use anyhow::anyhow;
use async_channel::Sender;
use codex_config::Constrained;
use codex_protocol::approvals::ElicitationRequest;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::McpStartupFailure;
use codex_protocol::protocol::SandboxPolicy;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::OAuthCredentialsStoreMode;
use serde::Deserialize;
use serde::Serialize;
use tokio_util::sync::CancellationToken;

use crate::config::types::McpServerConfig;
use crate::mcp::ToolPluginProvenance;
use crate::mcp::auth::McpAuthStatusEntry;
use crate::mcp_types::ListResourceTemplatesResult;
use crate::mcp_types::ListResourcesResult;
use crate::mcp_types::PaginatedRequestParams;
use crate::mcp_types::ReadResourceRequestParams;
use crate::mcp_types::ReadResourceResult;
use crate::mcp_types::RequestId;
use crate::mcp_types::Resource;
use crate::mcp_types::ResourceTemplate;
use crate::mcp_types::Tool;

pub const DEFAULT_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
pub const MCP_SANDBOX_STATE_CAPABILITY: &str = "codex/sandbox-state";
pub const MCP_SANDBOX_STATE_METHOD: &str = "codex/sandbox-state/update";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CodexAppsToolsCacheKey {
    account_id: Option<String>,
    chatgpt_user_id: Option<String>,
    is_workspace_account: bool,
}

pub(crate) fn codex_apps_tools_cache_key(
    auth: Option<&crate::CodexAuth>,
) -> CodexAppsToolsCacheKey {
    let token_data = auth.and_then(|auth| auth.get_token_data().ok());
    let account_id = token_data
        .as_ref()
        .and_then(|token_data| token_data.account_id.clone());
    let chatgpt_user_id = token_data
        .as_ref()
        .and_then(|token_data| token_data.id_token.chatgpt_user_id.clone());
    let is_workspace_account = token_data
        .as_ref()
        .is_some_and(|token_data| token_data.id_token.is_workspace_account());

    CodexAppsToolsCacheKey {
        account_id,
        chatgpt_user_id,
        is_workspace_account,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ToolInfo {
    pub(crate) server_name: String,
    pub(crate) tool_name: String,
    pub(crate) tool_namespace: String,
    pub(crate) tool: Tool,
    pub(crate) connector_id: Option<String>,
    pub(crate) connector_name: Option<String>,
    #[serde(default)]
    pub(crate) plugin_display_names: Vec<String>,
    pub(crate) connector_description: Option<String>,
}

pub(crate) fn filter_non_codex_apps_mcp_tools_only(
    mcp_tools: &HashMap<String, ToolInfo>,
) -> HashMap<String, ToolInfo> {
    mcp_tools.clone()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxState {
    pub sandbox_policy: SandboxPolicy,
    pub codex_linux_sandbox_exe: Option<PathBuf>,
    pub sandbox_cwd: PathBuf,
    #[serde(default)]
    pub use_legacy_landlock: bool,
}

pub(crate) struct McpConnectionManager;

impl McpConnectionManager {
    pub(crate) fn new_uninitialized(_approval_policy: &Constrained<AskForApproval>) -> Self {
        Self
    }

    pub(crate) fn has_servers(&self) -> bool {
        false
    }

    pub(crate) fn server_origin(&self, _server_name: &str) -> Option<&str> {
        None
    }

    pub fn set_approval_policy(&self, _approval_policy: &Constrained<AskForApproval>) {}

    #[allow(clippy::new_ret_no_self, clippy::too_many_arguments)]
    pub async fn new(
        _mcp_servers: &HashMap<String, McpServerConfig>,
        _store_mode: OAuthCredentialsStoreMode,
        _auth_entries: HashMap<String, McpAuthStatusEntry>,
        _approval_policy: &Constrained<AskForApproval>,
        _tx_event: Sender<Event>,
        _initial_sandbox_state: SandboxState,
        _codex_home: PathBuf,
        _codex_apps_tools_cache_key: CodexAppsToolsCacheKey,
        _tool_plugin_provenance: ToolPluginProvenance,
    ) -> (Self, CancellationToken) {
        (Self, CancellationToken::new())
    }

    pub async fn resolve_elicitation(
        &self,
        _server_name: String,
        _id: RequestId,
        _response: ElicitationResponse,
    ) -> Result<()> {
        Err(anyhow!("MCP elicitations are unavailable on wasm32"))
    }

    pub(crate) async fn wait_for_server_ready(
        &self,
        _server_name: &str,
        _timeout: Duration,
    ) -> bool {
        false
    }

    pub(crate) async fn required_startup_failures(
        &self,
        required_servers: &[String],
    ) -> Vec<McpStartupFailure> {
        required_servers
            .iter()
            .map(|server_name| McpStartupFailure {
                server: server_name.clone(),
                error: "MCP is unavailable on wasm32".to_string(),
            })
            .collect()
    }

    pub async fn list_all_tools(&self) -> HashMap<String, ToolInfo> {
        HashMap::new()
    }

    pub async fn hard_refresh_codex_apps_tools_cache(&self) -> Result<HashMap<String, ToolInfo>> {
        Ok(HashMap::new())
    }

    pub async fn list_all_resources(&self) -> HashMap<String, Vec<Resource>> {
        HashMap::new()
    }

    pub async fn list_all_resource_templates(&self) -> HashMap<String, Vec<ResourceTemplate>> {
        HashMap::new()
    }

    pub async fn call_tool(
        &self,
        _server: &str,
        _tool: &str,
        _arguments: Option<serde_json::Value>,
        _meta: Option<serde_json::Value>,
    ) -> Result<CallToolResult> {
        Err(anyhow!("MCP tool calls are unavailable on wasm32"))
    }

    pub async fn list_resources(
        &self,
        _server: &str,
        _params: Option<PaginatedRequestParams>,
    ) -> Result<ListResourcesResult> {
        Err(anyhow!("MCP resources are unavailable on wasm32"))
    }

    pub async fn list_resource_templates(
        &self,
        _server: &str,
        _params: Option<PaginatedRequestParams>,
    ) -> Result<ListResourceTemplatesResult> {
        Err(anyhow!("MCP resources are unavailable on wasm32"))
    }

    pub async fn read_resource(
        &self,
        _server: &str,
        _params: ReadResourceRequestParams,
    ) -> Result<ReadResourceResult> {
        Err(anyhow!("MCP resources are unavailable on wasm32"))
    }

    pub async fn parse_tool_name(&self, _tool_name: &str) -> Option<(String, String)> {
        None
    }

    pub async fn notify_sandbox_state_change(&self, _sandbox_state: &SandboxState) -> Result<()> {
        Ok(())
    }
}
