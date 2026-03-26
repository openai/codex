//! Connection manager for Model Context Protocol (MCP) servers.
//!
//! The [`McpConnectionManager`] owns one [`codex_rmcp_client::RmcpClient`] per
//! configured server (keyed by the *server name*). It offers convenience
//! helpers to query the available tools across *all* servers and returns them
//! in a single aggregated map using the fully-qualified tool name
//! `"<server><MCP_TOOL_NAME_DELIMITER><tool>"` as the key.

use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::mcp::ToolPluginProvenance;
use crate::mcp::auth::McpAuthStatusEntry;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use async_channel::Sender;
use codex_async_utils::CancelErr;
use codex_async_utils::OrCancelExt;
use codex_config::Constrained;
use codex_protocol::approvals::ElicitationRequest;
use codex_protocol::approvals::ElicitationRequestEvent;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::mcp::RequestId as ProtocolRequestId;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::McpStartupCompleteEvent;
use codex_protocol::protocol::McpStartupFailure;
use codex_protocol::protocol::McpStartupStatus;
use codex_protocol::protocol::McpStartupUpdateEvent;
use codex_protocol::protocol::SandboxPolicy;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::OAuthCredentialsStoreMode;
use codex_rmcp_client::RmcpClient;
use codex_rmcp_client::SendElicitation;
use futures::future::BoxFuture;
use futures::future::FutureExt;
use futures::future::Shared;
use rmcp::model::ClientCapabilities;
use rmcp::model::CreateElicitationRequestParams;
use rmcp::model::ElicitationAction;
use rmcp::model::ElicitationCapability;
use rmcp::model::FormElicitationCapability;
use rmcp::model::Implementation;
use rmcp::model::InitializeRequestParams;
use rmcp::model::ListResourceTemplatesResult;
use rmcp::model::ListResourcesResult;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ProtocolVersion;
use rmcp::model::ReadResourceRequestParams;
use rmcp::model::ReadResourceResult;
use rmcp::model::RequestId;
use rmcp::model::Resource;
use rmcp::model::ResourceTemplate;
use rmcp::model::Tool;

use serde::Deserialize;
use serde::Serialize;
use sha1::Digest;
use sha1::Sha1;
use tokio::sync::Mutex;
use tokio::sync::oneshot;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::instrument;
use tracing::warn;
use url::Url;

use crate::codex::INITIAL_SUBMIT_ID;
use crate::config::types::McpServerConfig;
use crate::config::types::McpServerTransportConfig;
use crate::connectors::is_connector_id_allowed;
use crate::connectors::sanitize_name;

/// Delimiter used to separate the server name from the tool name in a fully
/// qualified tool name.
///
/// OpenAI requires tool names to conform to `^[a-zA-Z0-9_-]+$`, so we must
/// choose a delimiter from this character set.
const MCP_TOOL_NAME_DELIMITER: &str = "__";
const MAX_TOOL_NAME_LENGTH: usize = 64;

/// Default timeout for initializing MCP server & initially listing tools.
pub const DEFAULT_STARTUP_TIMEOUT: Duration = Duration::from_secs(10);

/// Default timeout for individual tool calls.
const DEFAULT_TOOL_TIMEOUT: Duration = Duration::from_secs(120);

const CODEX_APPS_TOOLS_CACHE_SCHEMA_VERSION: u8 = 1;
const CODEX_APPS_TOOLS_CACHE_DIR: &str = "cache/codex_apps_tools";
const MCP_TOOLS_LIST_DURATION_METRIC: &str = "codex.mcp.tools.list.duration_ms";
const MCP_TOOLS_FETCH_UNCACHED_DURATION_METRIC: &str = "codex.mcp.tools.fetch_uncached.duration_ms";
const MCP_TOOLS_CACHE_WRITE_DURATION_METRIC: &str = "codex.mcp.tools.cache_write.duration_ms";

/// The Responses API requires tool names to match `^[a-zA-Z0-9_-]+$`.
/// MCP server/tool names are user-controlled, so sanitize the fully-qualified
/// name we expose to the model by replacing any disallowed character with `_`.
fn sanitize_responses_api_tool_name(name: &str) -> String {
    let mut sanitized = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            sanitized.push(c);
        } else {
            sanitized.push('_');
        }
    }

    if sanitized.is_empty() {
        "_".to_string()
    } else {
        sanitized
    }
}

fn sha1_hex(s: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(s.as_bytes());
    let sha1 = hasher.finalize();
    format!("{sha1:x}")
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

fn qualify_tools<I>(tools: I) -> HashMap<String, ToolInfo>
where
    I: IntoIterator<Item = ToolInfo>,
{
    let mut used_names = HashSet::new();
    let mut seen_raw_names = HashSet::new();
    let mut qualified_tools = HashMap::new();
    for tool in tools {
        let qualified_name_raw = if tool.server_name != CODEX_APPS_MCP_SERVER_NAME {
            format!(
                "mcp{}{}{}{}",
                MCP_TOOL_NAME_DELIMITER, tool.server_name, MCP_TOOL_NAME_DELIMITER, tool.tool_name
            )
        } else {
            format!("{}{}", tool.tool_namespace, tool.tool_name)
        };
        if !seen_raw_names.insert(qualified_name_raw.clone()) {
            warn!("skipping duplicated tool {}", qualified_name_raw);
            continue;
        }

        // Start from a "pretty" name (sanitized), then deterministically disambiguate on
        // collisions by appending a hash of the *raw* (unsanitized) qualified name. This
        // ensures tools like `foo.bar` and `foo_bar` don't collapse to the same key.
        let mut qualified_name = sanitize_responses_api_tool_name(&qualified_name_raw);

        // Enforce length constraints early; use the raw name for the hash input so the
        // output remains stable even when sanitization changes.
        if qualified_name.len() > MAX_TOOL_NAME_LENGTH {
            let sha1_str = sha1_hex(&qualified_name_raw);
            let prefix_len = MAX_TOOL_NAME_LENGTH - sha1_str.len();
            qualified_name = format!("{}{}", &qualified_name[..prefix_len], sha1_str);
        }

        if used_names.contains(&qualified_name) {
            warn!("skipping duplicated tool {}", qualified_name);
            continue;
        }

        used_names.insert(qualified_name.clone());
        qualified_tools.insert(qualified_name, tool);
    }

    qualified_tools
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CodexAppsToolsCacheKey {
    account_id: Option<String>,
    chatgpt_user_id: Option<String>,
    is_workspace_account: bool,
}

#[derive(Clone)]
struct CodexAppsToolsCacheContext {
    codex_home: PathBuf,
    user_key: CodexAppsToolsCacheKey,
}

impl CodexAppsToolsCacheContext {
    fn cache_path(&self) -> PathBuf {
        let user_key_json = serde_json::to_string(&self.user_key).unwrap_or_default();
        let user_key_hash = sha1_hex(&user_key_json);
        self.codex_home
            .join(CODEX_APPS_TOOLS_CACHE_DIR)
            .join(format!("{user_key_hash}.json"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CodexAppsToolsDiskCache {
    schema_version: u8,
    tools: Vec<ToolInfo>,
}

enum CachedCodexAppsToolsLoad {
    Hit(Vec<ToolInfo>),
    Missing,
    Invalid,
}

type ResponderMap = HashMap<(String, RequestId), oneshot::Sender<ElicitationResponse>>;

fn elicitation_is_rejected_by_policy(approval_policy: AskForApproval) -> bool {
    match approval_policy {
        AskForApproval::Never => true,
        AskForApproval::OnFailure => false,
        AskForApproval::OnRequest => false,
        AskForApproval::UnlessTrusted => false,
        AskForApproval::Granular(granular_config) => !granular_config.allows_mcp_elicitations(),
    }
}

#[derive(Clone)]
struct ElicitationRequestManager {
    requests: Arc<Mutex<ResponderMap>>,
    approval_policy: Arc<StdMutex<AskForApproval>>,
}

impl ElicitationRequestManager {
    fn new(approval_policy: AskForApproval) -> Self {
        Self {
            requests: Arc::new(Mutex::new(HashMap::new())),
            approval_policy: Arc::new(StdMutex::new(approval_policy)),
        }
    }

    async fn resolve(
        &self,
        server_name: String,
        id: RequestId,
        response: ElicitationResponse,
    ) -> Result<()> {
        self.requests
            .lock()
            .await
            .remove(&(server_name, id))
            .ok_or_else(|| anyhow!("elicitation request not found"))?
            .send(response)
            .map_err(|e| anyhow!("failed to send elicitation response: {e:?}"))
    }

    fn make_sender(&self, server_name: String, tx_event: Sender<Event>) -> SendElicitation {
        let elicitation_requests = self.requests.clone();
        let approval_policy = self.approval_policy.clone();
        Box::new(move |id, elicitation| {
            let elicitation_requests = elicitation_requests.clone();
            let tx_event = tx_event.clone();
            let server_name = server_name.clone();
            let approval_policy = approval_policy.clone();
            async move {
                if approval_policy
                    .lock()
                    .is_ok_and(|policy| elicitation_is_rejected_by_policy(*policy))
                {
                    return Ok(ElicitationResponse {
                        action: ElicitationAction::Decline,
                        content: None,
                        meta: None,
                    });
                }

                let request = match elicitation {
                    CreateElicitationRequestParams::FormElicitationParams {
                        meta,
                        message,
                        requested_schema,
                    } => ElicitationRequest::Form {
                        meta: meta
                            .map(serde_json::to_value)
                            .transpose()
                            .context("failed to serialize MCP elicitation metadata")?,
                        message,
                        requested_schema: serde_json::to_value(requested_schema)
                            .context("failed to serialize MCP elicitation schema")?,
                    },
                    CreateElicitationRequestParams::UrlElicitationParams {
                        meta,
                        message,
                        url,
                        elicitation_id,
                    } => ElicitationRequest::Url {
                        meta: meta
                            .map(serde_json::to_value)
                            .transpose()
                            .context("failed to serialize MCP elicitation metadata")?,
                        message,
                        url,
                        elicitation_id,
                    },
                };
                let (tx, rx) = oneshot::channel();
                {
                    let mut lock = elicitation_requests.lock().await;
                    lock.insert((server_name.clone(), id.clone()), tx);
                }
                let _ = tx_event
                    .send(Event {
                        id: "mcp_elicitation_request".to_string(),
                        msg: EventMsg::ElicitationRequest(ElicitationRequestEvent {
                            turn_id: None,
                            server_name,
                            id: match id.clone() {
                                rmcp::model::NumberOrString::String(value) => {
                                    ProtocolRequestId::String(value.to_string())
                                }
                                rmcp::model::NumberOrString::Number(value) => {
                                    ProtocolRequestId::Integer(value)
                                }
                            },
                            request,
                        }),
                    })
                    .await;
                rx.await
                    .context("elicitation request channel closed unexpectedly")
            }
            .boxed()
        })
    }
}

#[derive(Clone)]
struct ManagedClient {
    client: Arc<RmcpClient>,
    tools: Vec<ToolInfo>,
    tool_filter: ToolFilter,
    tool_timeout: Option<Duration>,
    server_supports_sandbox_state_capability: bool,
    codex_apps_tools_cache_context: Option<CodexAppsToolsCacheContext>,
}

impl ManagedClient {
    fn listed_tools(&self) -> Vec<ToolInfo> {
        let total_start = Instant::now();
        if let Some(cache_context) = self.codex_apps_tools_cache_context.as_ref()
            && let CachedCodexAppsToolsLoad::Hit(tools) =
                load_cached_codex_apps_tools(cache_context)
        {
            emit_duration(
                MCP_TOOLS_LIST_DURATION_METRIC,
                total_start.elapsed(),
                &[("cache", "hit")],
            );
            return filter_tools(tools, &self.tool_filter);
        }

        if self.codex_apps_tools_cache_context.is_some() {
            emit_duration(
                MCP_TOOLS_LIST_DURATION_METRIC,
                total_start.elapsed(),
                &[("cache", "miss")],
            );
        }

        self.tools.clone()
    }

    /// Returns once the server has ack'd the sandbox state update.
    async fn notify_sandbox_state_change(&self, sandbox_state: &SandboxState) -> Result<()> {
        if !self.server_supports_sandbox_state_capability {
            return Ok(());
        }

        let _response = self
            .client
            .send_custom_request(
                MCP_SANDBOX_STATE_METHOD,
                Some(serde_json::to_value(sandbox_state)?),
            )
            .await?;
        Ok(())
    }
}

#[derive(Clone)]
struct AsyncManagedClient {
    client: Shared<BoxFuture<'static, Result<ManagedClient, StartupOutcomeError>>>,
    startup_snapshot: Option<Vec<ToolInfo>>,
    startup_complete: Arc<AtomicBool>,
    tool_plugin_provenance: Arc<ToolPluginProvenance>,
}

impl AsyncManagedClient {
    // Keep this constructor flat so the startup inputs remain readable at the
    // single call site instead of introducing a one-off params wrapper.
    #[allow(clippy::too_many_arguments)]
    fn new(
        server_name: String,
        config: McpServerConfig,
        store_mode: OAuthCredentialsStoreMode,
        cancel_token: CancellationToken,
        tx_event: Sender<Event>,
        elicitation_requests: ElicitationRequestManager,
        codex_apps_tools_cache_context: Option<CodexAppsToolsCacheContext>,
        tool_plugin_provenance: Arc<ToolPluginProvenance>,
    ) -> Self {
        let tool_filter = ToolFilter::from_config(&config);
        let startup_snapshot = load_startup_cached_codex_apps_tools_snapshot(
            &server_name,
            codex_apps_tools_cache_context.as_ref(),
        )
        .map(|tools| filter_tools(tools, &tool_filter));
        let startup_tool_filter = tool_filter;
        let startup_complete = Arc::new(AtomicBool::new(false));
        let startup_complete_for_fut = Arc::clone(&startup_complete);
        let fut = async move {
            let outcome = async {
                if let Err(error) = validate_mcp_server_name(&server_name) {
                    return Err(error.into());
                }

                let client =
                    Arc::new(make_rmcp_client(&server_name, config.transport, store_mode).await?);
                match start_server_task(
                    server_name,
                    client,
                    StartServerTaskParams {
                        startup_timeout: config
                            .startup_timeout_sec
                            .or(Some(DEFAULT_STARTUP_TIMEOUT)),
                        tool_timeout: config.tool_timeout_sec.unwrap_or(DEFAULT_TOOL_TIMEOUT),
                        tool_filter: startup_tool_filter,
                        tx_event,
                        elicitation_requests,
                        codex_apps_tools_cache_context,
                    },
                )
                .or_cancel(&cancel_token)
                .await
                {
                    Ok(result) => result,
                    Err(CancelErr::Cancelled) => Err(StartupOutcomeError::Cancelled),
                }
            }
            .await;

            startup_complete_for_fut.store(true, Ordering::Release);
            outcome
        };
        let client = fut.boxed().shared();
        if startup_snapshot.is_some() {
            let startup_task = client.clone();
            tokio::spawn(async move {
                let _ = startup_task.await;
            });
        }

        Self {
            client,
            startup_snapshot,
            startup_complete,
            tool_plugin_provenance,
        }
    }

    async fn client(&self) -> Result<ManagedClient, StartupOutcomeError> {
        self.client.clone().await
    }

    fn startup_snapshot_while_initializing(&self) -> Option<Vec<ToolInfo>> {
        if !self.startup_complete.load(Ordering::Acquire) {
            return self.startup_snapshot.clone();
        }
        None
    }

    async fn listed_tools(&self) -> Option<Vec<ToolInfo>> {
        let annotate_tools = |tools: Vec<ToolInfo>| {
            let mut tools = tools;
            for tool in &mut tools {
                let plugin_names = match tool.connector_id.as_deref() {
                    Some(connector_id) => self
                        .tool_plugin_provenance
                        .plugin_display_names_for_connector_id(connector_id),
                    None => self
                        .tool_plugin_provenance
                        .plugin_display_names_for_mcp_server_name(tool.server_name.as_str()),
                };
                tool.plugin_display_names = plugin_names.to_vec();

                if plugin_names.is_empty() {
                    continue;
                }

                let plugin_source_note = if plugin_names.len() == 1 {
                    format!("This tool is part of plugin `{}`.", plugin_names[0])
                } else {
                    format!(
                        "This tool is part of plugins {}.",
                        plugin_names
                            .iter()
                            .map(|plugin_name| format!("`{plugin_name}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                let description = tool
                    .tool
                    .description
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or("");
                let annotated_description = if description.is_empty() {
                    plugin_source_note
                } else if matches!(description.chars().last(), Some('.' | '!' | '?')) {
                    format!("{description} {plugin_source_note}")
                } else {
                    format!("{description}. {plugin_source_note}")
                };
                tool.tool.description = Some(Cow::Owned(annotated_description));
            }
            tools
        };

        // Keep cache payloads raw; plugin provenance is resolved per-session at read time.
        let tools = if let Some(startup_tools) = self.startup_snapshot_while_initializing() {
            Some(startup_tools)
        } else {
            match self.client().await {
                Ok(client) => Some(client.listed_tools()),
                Err(_) => self.startup_snapshot.clone(),
            }
        };
        tools.map(annotate_tools)
    }

    async fn notify_sandbox_state_change(&self, sandbox_state: &SandboxState) -> Result<()> {
        let managed = self.client().await?;
        managed.notify_sandbox_state_change(sandbox_state).await
    }
}

pub const MCP_SANDBOX_STATE_CAPABILITY: &str = "codex/sandbox-state";

/// Custom MCP request to push sandbox state updates.
/// When used, the `params` field of the notification is [`SandboxState`].
pub const MCP_SANDBOX_STATE_METHOD: &str = "codex/sandbox-state/update";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxState {
    pub sandbox_policy: SandboxPolicy,
    pub codex_linux_sandbox_exe: Option<PathBuf>,
    pub sandbox_cwd: PathBuf,
    #[serde(default)]
    pub use_legacy_landlock: bool,
}

/// A thin wrapper around a set of running [`RmcpClient`] instances.
pub(crate) struct McpConnectionManager {
    clients: HashMap<String, AsyncManagedClient>,
    server_origins: HashMap<String, String>,
    elicitation_requests: ElicitationRequestManager,
}

impl McpConnectionManager {
    pub(crate) fn new_uninitialized(approval_policy: &Constrained<AskForApproval>) -> Self {
        Self {
            clients: HashMap::new(),
            server_origins: HashMap::new(),
            elicitation_requests: ElicitationRequestManager::new(approval_policy.value()),
        }
    }

    #[cfg(test)]
    pub(crate) fn new_mcp_connection_manager_for_tests(
        approval_policy: &Constrained<AskForApproval>,
    ) -> Self {
        Self::new_uninitialized(approval_policy)
    }

    pub(crate) fn has_servers(&self) -> bool {
        !self.clients.is_empty()
    }

    pub(crate) fn server_origin(&self, server_name: &str) -> Option<&str> {
        self.server_origins.get(server_name).map(String::as_str)
    }

    pub fn set_approval_policy(&self, approval_policy: &Constrained<AskForApproval>) {
        if let Ok(mut policy) = self.elicitation_requests.approval_policy.lock() {
            *policy = approval_policy.value();
        }
    }

    #[allow(clippy::new_ret_no_self, clippy::too_many_arguments)]
    pub async fn new(
        mcp_servers: &HashMap<String, McpServerConfig>,
        store_mode: OAuthCredentialsStoreMode,
        auth_entries: HashMap<String, McpAuthStatusEntry>,
        approval_policy: &Constrained<AskForApproval>,
        tx_event: Sender<Event>,
        initial_sandbox_state: SandboxState,
        codex_home: PathBuf,
        codex_apps_tools_cache_key: CodexAppsToolsCacheKey,
        tool_plugin_provenance: ToolPluginProvenance,
    ) -> (Self, CancellationToken) {
        let cancel_token = CancellationToken::new();
        let mut clients = HashMap::new();
        let mut server_origins = HashMap::new();
        let mut join_set = JoinSet::new();
        let elicitation_requests = ElicitationRequestManager::new(approval_policy.value());
        let tool_plugin_provenance = Arc::new(tool_plugin_provenance);
        let mcp_servers = mcp_servers.clone();
        for (server_name, cfg) in mcp_servers.into_iter().filter(|(_, cfg)| cfg.enabled) {
            if let Some(origin) = transport_origin(&cfg.transport) {
                server_origins.insert(server_name.clone(), origin);
            }
            let cancel_token = cancel_token.child_token();
            let _ = emit_update(
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
            let async_managed_client = AsyncManagedClient::new(
                server_name.clone(),
                cfg,
                store_mode,
                cancel_token.clone(),
                tx_event.clone(),
                elicitation_requests.clone(),
                codex_apps_tools_cache_context,
                Arc::clone(&tool_plugin_provenance),
            );
            clients.insert(server_name.clone(), async_managed_client.clone());
            let tx_event = tx_event.clone();
            let auth_entry = auth_entries.get(&server_name).cloned();
            let sandbox_state = initial_sandbox_state.clone();
            join_set.spawn(async move {
                let outcome = async_managed_client.client().await;
                if cancel_token.is_cancelled() {
                    return (server_name, Err(StartupOutcomeError::Cancelled));
                }
                let status = match &outcome {
                    Ok(_) => {
                        // Send sandbox state notification immediately after Ready
                        if let Err(e) = async_managed_client
                            .notify_sandbox_state_change(&sandbox_state)
                            .await
                        {
                            warn!(
                                "Failed to notify sandbox state to MCP server {server_name}: {e:#}",
                            );
                        }
                        McpStartupStatus::Ready
                    }
                    Err(error) => {
                        let error_str = mcp_init_error_display(
                            server_name.as_str(),
                            auth_entry.as_ref(),
                            error,
                        );
                        McpStartupStatus::Failed { error: error_str }
                    }
                };

                let _ = emit_update(
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
        let manager = Self {
            clients,
            server_origins,
            elicitation_requests: elicitation_requests.clone(),
        };
        tokio::spawn(async move {
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
                        })
                    }
                }
            }
            let _ = tx_event
                .send(Event {
                    id: INITIAL_SUBMIT_ID.to_owned(),
                    msg: EventMsg::McpStartupComplete(summary),
                })
                .await;
        });
        (manager, cancel_token)
    }

    async fn client_by_name(&self, name: &str) -> Result<ManagedClient> {
        self.clients
            .get(name)
            .ok_or_else(|| anyhow!("unknown MCP server '{name}'"))?
            .client()
            .await
            .context("failed to get client")
    }

    pub async fn resolve_elicitation(
        &self,
        server_name: String,
        id: RequestId,
        response: ElicitationResponse,
    ) -> Result<()> {
        self.elicitation_requests
            .resolve(server_name, id, response)
            .await
    }

    pub(crate) async fn wait_for_server_ready(&self, server_name: &str, timeout: Duration) -> bool {
        let Some(async_managed_client) = self.clients.get(server_name) else {
            return false;
        };

        match tokio::time::timeout(timeout, async_managed_client.client()).await {
            Ok(Ok(_)) => true,
            Ok(Err(_)) | Err(_) => false,
        }
    }

    pub(crate) async fn required_startup_failures(
        &self,
        required_servers: &[String],
    ) -> Vec<McpStartupFailure> {
        let mut failures = Vec::new();
        for server_name in required_servers {
            let Some(async_managed_client) = self.clients.get(server_name).cloned() else {
                failures.push(McpStartupFailure {
                    server: server_name.clone(),
                    error: format!("required MCP server `{server_name}` was not initialized"),
                });
                continue;
            };

            match async_managed_client.client().await {
                Ok(_) => {}
                Err(error) => failures.push(McpStartupFailure {
                    server: server_name.clone(),
                    error: startup_outcome_error_message(error),
                }),
            }
        }
        failures
    }

    /// Returns a single map that contains all tools. Each key is the
    /// fully-qualified name for the tool.
    #[instrument(level = "trace", skip_all)]
    pub async fn list_all_tools(&self) -> HashMap<String, ToolInfo> {
        let mut tools = HashMap::new();
        for managed_client in self.clients.values() {
            let Some(server_tools) = managed_client.listed_tools().await else {
                continue;
            };
            tools.extend(qualify_tools(server_tools));
        }
        tools
    }

    /// Force-refresh codex apps tools by bypassing the in-process cache.
    ///
    /// On success, the refreshed tools replace the cache contents and the
    /// latest filtered tool map is returned directly to the caller. On
    /// failure, the existing cache remains unchanged.
    pub async fn hard_refresh_codex_apps_tools_cache(&self) -> Result<HashMap<String, ToolInfo>> {
        let managed_client = self
            .clients
            .get(CODEX_APPS_MCP_SERVER_NAME)
            .ok_or_else(|| anyhow!("unknown MCP server '{CODEX_APPS_MCP_SERVER_NAME}'"))?
            .client()
            .await
            .context("failed to get client")?;

        let list_start = Instant::now();
        let fetch_start = Instant::now();
        let tools = list_tools_for_client_uncached(
            CODEX_APPS_MCP_SERVER_NAME,
            &managed_client.client,
            managed_client.tool_timeout,
        )
        .await
        .with_context(|| {
            format!("failed to refresh tools for MCP server '{CODEX_APPS_MCP_SERVER_NAME}'")
        })?;
        emit_duration(
            MCP_TOOLS_FETCH_UNCACHED_DURATION_METRIC,
            fetch_start.elapsed(),
            &[],
        );

        write_cached_codex_apps_tools_if_needed(
            CODEX_APPS_MCP_SERVER_NAME,
            managed_client.codex_apps_tools_cache_context.as_ref(),
            &tools,
        );
        emit_duration(
            MCP_TOOLS_LIST_DURATION_METRIC,
            list_start.elapsed(),
            &[("cache", "miss")],
        );
        Ok(qualify_tools(filter_tools(
            tools,
            &managed_client.tool_filter,
        )))
    }

    /// Returns a single map that contains all resources. Each key is the
    /// server name and the value is a vector of resources.
    pub async fn list_all_resources(&self) -> HashMap<String, Vec<Resource>> {
        let mut join_set = JoinSet::new();

        let clients_snapshot = &self.clients;

        for (server_name, async_managed_client) in clients_snapshot {
            let server_name = server_name.clone();
            let Ok(managed_client) = async_managed_client.client().await else {
                continue;
            };
            let timeout = managed_client.tool_timeout;
            let client = managed_client.client.clone();

            join_set.spawn(async move {
                let mut collected: Vec<Resource> = Vec::new();
                let mut cursor: Option<String> = None;

                loop {
                    let params = cursor.as_ref().map(|next| PaginatedRequestParams {
                        meta: None,
                        cursor: Some(next.clone()),
                    });
                    let response = match client.list_resources(params, timeout).await {
                        Ok(result) => result,
                        Err(err) => return (server_name, Err(err)),
                    };

                    collected.extend(response.resources);

                    match response.next_cursor {
                        Some(next) => {
                            if cursor.as_ref() == Some(&next) {
                                return (
                                    server_name,
                                    Err(anyhow!("resources/list returned duplicate cursor")),
                                );
                            }
                            cursor = Some(next);
                        }
                        None => return (server_name, Ok(collected)),
                    }
                }
            });
        }

        let mut aggregated: HashMap<String, Vec<Resource>> = HashMap::new();

        while let Some(join_res) = join_set.join_next().await {
            match join_res {
                Ok((server_name, Ok(resources))) => {
                    aggregated.insert(server_name, resources);
                }
                Ok((server_name, Err(err))) => {
                    warn!("Failed to list resources for MCP server '{server_name}': {err:#}");
                }
                Err(err) => {
                    warn!("Task panic when listing resources for MCP server: {err:#}");
                }
            }
        }

        aggregated
    }

    /// Returns a single map that contains all resource templates. Each key is the
    /// server name and the value is a vector of resource templates.
    pub async fn list_all_resource_templates(&self) -> HashMap<String, Vec<ResourceTemplate>> {
        let mut join_set = JoinSet::new();

        let clients_snapshot = &self.clients;

        for (server_name, async_managed_client) in clients_snapshot {
            let server_name_cloned = server_name.clone();
            let Ok(managed_client) = async_managed_client.client().await else {
                continue;
            };
            let client = managed_client.client.clone();
            let timeout = managed_client.tool_timeout;

            join_set.spawn(async move {
                let mut collected: Vec<ResourceTemplate> = Vec::new();
                let mut cursor: Option<String> = None;

                loop {
                    let params = cursor.as_ref().map(|next| PaginatedRequestParams {
                        meta: None,
                        cursor: Some(next.clone()),
                    });
                    let response = match client.list_resource_templates(params, timeout).await {
                        Ok(result) => result,
                        Err(err) => return (server_name_cloned, Err(err)),
                    };

                    collected.extend(response.resource_templates);

                    match response.next_cursor {
                        Some(next) => {
                            if cursor.as_ref() == Some(&next) {
                                return (
                                    server_name_cloned,
                                    Err(anyhow!(
                                        "resources/templates/list returned duplicate cursor"
                                    )),
                                );
                            }
                            cursor = Some(next);
                        }
                        None => return (server_name_cloned, Ok(collected)),
                    }
                }
            });
        }

        let mut aggregated: HashMap<String, Vec<ResourceTemplate>> = HashMap::new();

        while let Some(join_res) = join_set.join_next().await {
            match join_res {
                Ok((server_name, Ok(templates))) => {
                    aggregated.insert(server_name, templates);
                }
                Ok((server_name, Err(err))) => {
                    warn!(
                        "Failed to list resource templates for MCP server '{server_name}': {err:#}"
                    );
                }
                Err(err) => {
                    warn!("Task panic when listing resource templates for MCP server: {err:#}");
                }
            }
        }

        aggregated
    }

    /// Invoke the tool indicated by the (server, tool) pair.
    pub async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        arguments: Option<serde_json::Value>,
        meta: Option<serde_json::Value>,
    ) -> Result<CallToolResult> {
        let client = self.client_by_name(server).await?;
        if !client.tool_filter.allows(tool) {
            return Err(anyhow!(
                "tool '{tool}' is disabled for MCP server '{server}'"
            ));
        }

        let result: rmcp::model::CallToolResult = client
            .client
            .call_tool(tool.to_string(), arguments, meta, client.tool_timeout)
            .await
            .with_context(|| format!("tool call failed for `{server}/{tool}`"))?;

        let content = result
            .content
            .into_iter()
            .map(|content| {
                serde_json::to_value(content)
                    .unwrap_or_else(|_| serde_json::Value::String("<content>".to_string()))
            })
            .collect();

        Ok(CallToolResult {
            content,
            structured_content: result.structured_content,
            is_error: result.is_error,
            meta: result.meta.and_then(|meta| serde_json::to_value(meta).ok()),
        })
    }

    /// List resources from the specified server.
    pub async fn list_resources(
        &self,
        server: &str,
        params: Option<PaginatedRequestParams>,
    ) -> Result<ListResourcesResult> {
        let managed = self.client_by_name(server).await?;
        let timeout = managed.tool_timeout;

        managed
            .client
            .list_resources(params, timeout)
            .await
            .with_context(|| format!("resources/list failed for `{server}`"))
    }

    /// List resource templates from the specified server.
    pub async fn list_resource_templates(
        &self,
        server: &str,
        params: Option<PaginatedRequestParams>,
    ) -> Result<ListResourceTemplatesResult> {
        let managed = self.client_by_name(server).await?;
        let client = managed.client.clone();
        let timeout = managed.tool_timeout;

        client
            .list_resource_templates(params, timeout)
            .await
            .with_context(|| format!("resources/templates/list failed for `{server}`"))
    }

    /// Read a resource from the specified server.
    pub async fn read_resource(
        &self,
        server: &str,
        params: ReadResourceRequestParams,
    ) -> Result<ReadResourceResult> {
        let managed = self.client_by_name(server).await?;
        let client = managed.client.clone();
        let timeout = managed.tool_timeout;
        let uri = params.uri.clone();

        client
            .read_resource(params, timeout)
            .await
            .with_context(|| format!("resources/read failed for `{server}` ({uri})"))
    }

    pub async fn parse_tool_name(&self, tool_name: &str) -> Option<(String, String)> {
        self.list_all_tools()
            .await
            .get(tool_name)
            .map(|tool| (tool.server_name.clone(), tool.tool.name.to_string()))
    }

    pub async fn notify_sandbox_state_change(&self, sandbox_state: &SandboxState) -> Result<()> {
        let mut join_set = JoinSet::new();

        for async_managed_client in self.clients.values() {
            let sandbox_state = sandbox_state.clone();
            let async_managed_client = async_managed_client.clone();
            join_set.spawn(async move {
                async_managed_client
                    .notify_sandbox_state_change(&sandbox_state)
                    .await
            });
        }

        while let Some(join_res) = join_set.join_next().await {
            match join_res {
                Ok(Ok(())) => {}
                Ok(Err(err)) => {
                    warn!("Failed to notify sandbox state change to MCP server: {err:#}");
                }
                Err(err) => {
                    warn!("Task panic when notifying sandbox state change to MCP server: {err:#}");
                }
            }
        }

        Ok(())
    }
}

async fn emit_update(
    tx_event: &Sender<Event>,
    update: McpStartupUpdateEvent,
) -> Result<(), async_channel::SendError<Event>> {
    tx_event
        .send(Event {
            id: INITIAL_SUBMIT_ID.to_owned(),
            msg: EventMsg::McpStartupUpdate(update),
        })
        .await
}

/// A tool is allowed to be used if both are true:
/// 1. enabled is None (no allowlist is set) or the tool is explicitly enabled.
/// 2. The tool is not explicitly disabled.
#[derive(Default, Clone)]
pub(crate) struct ToolFilter {
    enabled: Option<HashSet<String>>,
    disabled: HashSet<String>,
}

impl ToolFilter {
    fn from_config(cfg: &McpServerConfig) -> Self {
        let enabled = cfg
            .enabled_tools
            .as_ref()
            .map(|tools| tools.iter().cloned().collect::<HashSet<_>>());
        let disabled = cfg
            .disabled_tools
            .as_ref()
            .map(|tools| tools.iter().cloned().collect::<HashSet<_>>())
            .unwrap_or_default();

        Self { enabled, disabled }
    }

    fn allows(&self, tool_name: &str) -> bool {
        if let Some(enabled) = &self.enabled
            && !enabled.contains(tool_name)
        {
            return false;
        }

        !self.disabled.contains(tool_name)
    }
}

fn filter_tools(tools: Vec<ToolInfo>, filter: &ToolFilter) -> Vec<ToolInfo> {
    tools
        .into_iter()
        .filter(|tool| filter.allows(&tool.tool.name))
        .collect()
}

pub(crate) fn filter_non_codex_apps_mcp_tools_only(
    mcp_tools: &HashMap<String, ToolInfo>,
) -> HashMap<String, ToolInfo> {
    mcp_tools
        .iter()
        .filter(|(_, tool)| tool.server_name != CODEX_APPS_MCP_SERVER_NAME)
        .map(|(name, tool)| (name.clone(), tool.clone()))
        .collect()
}

fn normalize_codex_apps_tool_title(
    server_name: &str,
    connector_name: Option<&str>,
    value: &str,
) -> String {
    if server_name != CODEX_APPS_MCP_SERVER_NAME {
        return value.to_string();
    }

    let Some(connector_name) = connector_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
    else {
        return value.to_string();
    };

    let prefix = format!("{connector_name}_");
    if let Some(stripped) = value.strip_prefix(&prefix)
        && !stripped.is_empty()
    {
        return stripped.to_string();
    }

    value.to_string()
}

fn normalize_codex_apps_tool_name(
    server_name: &str,
    tool_name: &str,
    connector_id: Option<&str>,
    connector_name: Option<&str>,
) -> String {
    if server_name != CODEX_APPS_MCP_SERVER_NAME {
        return tool_name.to_string();
    }

    let tool_name = sanitize_name(tool_name);

    if let Some(connector_name) = connector_name
        .map(str::trim)
        .map(sanitize_name)
        .filter(|name| !name.is_empty())
        && let Some(stripped) = tool_name.strip_prefix(&connector_name)
        && !stripped.is_empty()
    {
        return stripped.to_string();
    }

    if let Some(connector_id) = connector_id
        .map(str::trim)
        .map(sanitize_name)
        .filter(|name| !name.is_empty())
        && let Some(stripped) = tool_name.strip_prefix(&connector_id)
        && !stripped.is_empty()
    {
        return stripped.to_string();
    }

    tool_name
}

fn normalize_codex_apps_namespace(server_name: &str, connector_name: Option<&str>) -> String {
    if server_name != CODEX_APPS_MCP_SERVER_NAME {
        server_name.to_string()
    } else if let Some(connector_name) = connector_name {
        format!(
            "mcp{}{}{}{}",
            MCP_TOOL_NAME_DELIMITER,
            server_name,
            MCP_TOOL_NAME_DELIMITER,
            sanitize_name(connector_name)
        )
    } else {
        server_name.to_string()
    }
}

fn resolve_bearer_token(
    server_name: &str,
    bearer_token_env_var: Option<&str>,
) -> Result<Option<String>> {
    let Some(env_var) = bearer_token_env_var else {
        return Ok(None);
    };

    match env::var(env_var) {
        Ok(value) => {
            if value.is_empty() {
                Err(anyhow!(
                    "Environment variable {env_var} for MCP server '{server_name}' is empty"
                ))
            } else {
                Ok(Some(value))
            }
        }
        Err(env::VarError::NotPresent) => Err(anyhow!(
            "Environment variable {env_var} for MCP server '{server_name}' is not set"
        )),
        Err(env::VarError::NotUnicode(_)) => Err(anyhow!(
            "Environment variable {env_var} for MCP server '{server_name}' contains invalid Unicode"
        )),
    }
}

#[derive(Debug, Clone, thiserror::Error)]
enum StartupOutcomeError {
    #[error("MCP startup cancelled")]
    Cancelled,
    // We can't store the original error here because anyhow::Error doesn't implement
    // `Clone`.
    #[error("MCP startup failed: {error}")]
    Failed { error: String },
}

impl From<anyhow::Error> for StartupOutcomeError {
    fn from(error: anyhow::Error) -> Self {
        Self::Failed {
            error: error.to_string(),
        }
    }
}

fn elicitation_capability_for_server(server_name: &str) -> Option<ElicitationCapability> {
    if server_name == CODEX_APPS_MCP_SERVER_NAME {
        // https://modelcontextprotocol.io/specification/2025-06-18/client/elicitation#capabilities
        // indicates this should be an empty object.
        Some(ElicitationCapability {
            form: Some(FormElicitationCapability {
                schema_validation: None,
            }),
            url: None,
        })
    } else {
        None
    }
}

async fn start_server_task(
    server_name: String,
    client: Arc<RmcpClient>,
    params: StartServerTaskParams,
) -> Result<ManagedClient, StartupOutcomeError> {
    let StartServerTaskParams {
        startup_timeout,
        tool_timeout,
        tool_filter,
        tx_event,
        elicitation_requests,
        codex_apps_tools_cache_context,
    } = params;
    let elicitation = elicitation_capability_for_server(&server_name);
    let params = InitializeRequestParams {
        meta: None,
        capabilities: ClientCapabilities {
            experimental: None,
            extensions: None,
            roots: None,
            sampling: None,
            elicitation,
            tasks: None,
        },
        client_info: Implementation {
            name: "codex-mcp-client".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            title: Some("Codex".into()),
            description: None,
            icons: None,
            website_url: None,
        },
        protocol_version: ProtocolVersion::V_2025_06_18,
    };

    let send_elicitation = elicitation_requests.make_sender(server_name.clone(), tx_event);

    let initialize_result = client
        .initialize(params, startup_timeout, send_elicitation)
        .await
        .map_err(StartupOutcomeError::from)?;

    let list_start = Instant::now();
    let fetch_start = Instant::now();
    let tools = list_tools_for_client_uncached(&server_name, &client, startup_timeout)
        .await
        .map_err(StartupOutcomeError::from)?;
    emit_duration(
        MCP_TOOLS_FETCH_UNCACHED_DURATION_METRIC,
        fetch_start.elapsed(),
        &[],
    );
    write_cached_codex_apps_tools_if_needed(
        &server_name,
        codex_apps_tools_cache_context.as_ref(),
        &tools,
    );
    if server_name == CODEX_APPS_MCP_SERVER_NAME {
        emit_duration(
            MCP_TOOLS_LIST_DURATION_METRIC,
            list_start.elapsed(),
            &[("cache", "miss")],
        );
    }
    let tools = filter_tools(tools, &tool_filter);

    let server_supports_sandbox_state_capability = initialize_result
        .capabilities
        .experimental
        .as_ref()
        .and_then(|exp| exp.get(MCP_SANDBOX_STATE_CAPABILITY))
        .is_some();
    let managed = ManagedClient {
        client: Arc::clone(&client),
        tools,
        tool_timeout: Some(tool_timeout),
        tool_filter,
        server_supports_sandbox_state_capability,
        codex_apps_tools_cache_context,
    };

    Ok(managed)
}

struct StartServerTaskParams {
    startup_timeout: Option<Duration>, // TODO: cancel_token should handle this.
    tool_timeout: Duration,
    tool_filter: ToolFilter,
    tx_event: Sender<Event>,
    elicitation_requests: ElicitationRequestManager,
    codex_apps_tools_cache_context: Option<CodexAppsToolsCacheContext>,
}

async fn make_rmcp_client(
    server_name: &str,
    transport: McpServerTransportConfig,
    store_mode: OAuthCredentialsStoreMode,
) -> Result<RmcpClient, StartupOutcomeError> {
    match transport {
        McpServerTransportConfig::Stdio {
            command,
            args,
            env,
            env_vars,
            cwd,
        } => {
            let command_os: OsString = command.into();
            let args_os: Vec<OsString> = args.into_iter().map(Into::into).collect();
            let env_os = env.map(|env| {
                env.into_iter()
                    .map(|(key, value)| (key.into(), value.into()))
                    .collect::<HashMap<_, _>>()
            });
            RmcpClient::new_stdio_client(command_os, args_os, env_os, &env_vars, cwd)
                .await
                .map_err(|err| StartupOutcomeError::from(anyhow!(err)))
        }
        McpServerTransportConfig::StreamableHttp {
            url,
            http_headers,
            env_http_headers,
            bearer_token_env_var,
        } => {
            let resolved_bearer_token =
                match resolve_bearer_token(server_name, bearer_token_env_var.as_deref()) {
                    Ok(token) => token,
                    Err(error) => return Err(error.into()),
                };
            RmcpClient::new_streamable_http_client(
                server_name,
                &url,
                resolved_bearer_token,
                http_headers,
                env_http_headers,
                store_mode,
            )
            .await
            .map_err(StartupOutcomeError::from)
        }
    }
}

fn write_cached_codex_apps_tools_if_needed(
    server_name: &str,
    cache_context: Option<&CodexAppsToolsCacheContext>,
    tools: &[ToolInfo],
) {
    if server_name != CODEX_APPS_MCP_SERVER_NAME {
        return;
    }

    if let Some(cache_context) = cache_context {
        let cache_write_start = Instant::now();
        write_cached_codex_apps_tools(cache_context, tools);
        emit_duration(
            MCP_TOOLS_CACHE_WRITE_DURATION_METRIC,
            cache_write_start.elapsed(),
            &[],
        );
    }
}

fn load_startup_cached_codex_apps_tools_snapshot(
    server_name: &str,
    cache_context: Option<&CodexAppsToolsCacheContext>,
) -> Option<Vec<ToolInfo>> {
    if server_name != CODEX_APPS_MCP_SERVER_NAME {
        return None;
    }

    let cache_context = cache_context?;

    match load_cached_codex_apps_tools(cache_context) {
        CachedCodexAppsToolsLoad::Hit(tools) => Some(tools),
        CachedCodexAppsToolsLoad::Missing | CachedCodexAppsToolsLoad::Invalid => None,
    }
}

#[cfg(test)]
fn read_cached_codex_apps_tools(
    cache_context: &CodexAppsToolsCacheContext,
) -> Option<Vec<ToolInfo>> {
    match load_cached_codex_apps_tools(cache_context) {
        CachedCodexAppsToolsLoad::Hit(tools) => Some(tools),
        CachedCodexAppsToolsLoad::Missing | CachedCodexAppsToolsLoad::Invalid => None,
    }
}

fn load_cached_codex_apps_tools(
    cache_context: &CodexAppsToolsCacheContext,
) -> CachedCodexAppsToolsLoad {
    let cache_path = cache_context.cache_path();
    let bytes = match std::fs::read(cache_path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return CachedCodexAppsToolsLoad::Missing;
        }
        Err(_) => return CachedCodexAppsToolsLoad::Invalid,
    };
    let cache: CodexAppsToolsDiskCache = match serde_json::from_slice(&bytes) {
        Ok(cache) => cache,
        Err(_) => return CachedCodexAppsToolsLoad::Invalid,
    };
    if cache.schema_version != CODEX_APPS_TOOLS_CACHE_SCHEMA_VERSION {
        return CachedCodexAppsToolsLoad::Invalid;
    }
    CachedCodexAppsToolsLoad::Hit(filter_disallowed_codex_apps_tools(cache.tools))
}

fn write_cached_codex_apps_tools(cache_context: &CodexAppsToolsCacheContext, tools: &[ToolInfo]) {
    let cache_path = cache_context.cache_path();
    if let Some(parent) = cache_path.parent()
        && std::fs::create_dir_all(parent).is_err()
    {
        return;
    }
    let tools = filter_disallowed_codex_apps_tools(tools.to_vec());
    let Ok(bytes) = serde_json::to_vec_pretty(&CodexAppsToolsDiskCache {
        schema_version: CODEX_APPS_TOOLS_CACHE_SCHEMA_VERSION,
        tools,
    }) else {
        return;
    };
    let _ = std::fs::write(cache_path, bytes);
}

fn filter_disallowed_codex_apps_tools(tools: Vec<ToolInfo>) -> Vec<ToolInfo> {
    tools
        .into_iter()
        .filter(|tool| {
            tool.connector_id
                .as_deref()
                .is_none_or(is_connector_id_allowed)
        })
        .collect()
}

fn emit_duration(metric: &str, duration: Duration, tags: &[(&str, &str)]) {
    if let Some(metrics) = codex_otel::metrics::global() {
        let _ = metrics.record_duration(metric, duration, tags);
    }
}

fn transport_origin(transport: &McpServerTransportConfig) -> Option<String> {
    match transport {
        McpServerTransportConfig::StreamableHttp { url, .. } => {
            let parsed = Url::parse(url).ok()?;
            Some(parsed.origin().ascii_serialization())
        }
        McpServerTransportConfig::Stdio { .. } => Some("stdio".to_string()),
    }
}

async fn list_tools_for_client_uncached(
    server_name: &str,
    client: &Arc<RmcpClient>,
    timeout: Option<Duration>,
) -> Result<Vec<ToolInfo>> {
    let resp = client
        .list_tools_with_connector_ids(/*params*/ None, timeout)
        .await?;
    let tools = resp
        .tools
        .into_iter()
        .map(|tool| {
            let tool_name = normalize_codex_apps_tool_name(
                server_name,
                &tool.tool.name,
                tool.connector_id.as_deref(),
                tool.connector_name.as_deref(),
            );
            let tool_namespace =
                normalize_codex_apps_namespace(server_name, tool.connector_name.as_deref());
            let connector_name = tool.connector_name;
            let connector_description = tool.connector_description;
            let mut tool_def = tool.tool;
            if let Some(title) = tool_def.title.as_deref() {
                let normalized_title =
                    normalize_codex_apps_tool_title(server_name, connector_name.as_deref(), title);
                if tool_def.title.as_deref() != Some(normalized_title.as_str()) {
                    tool_def.title = Some(normalized_title);
                }
            }
            ToolInfo {
                server_name: server_name.to_owned(),
                tool_name,
                tool_namespace,
                tool: tool_def,
                connector_id: tool.connector_id,
                connector_name,
                plugin_display_names: Vec::new(),
                connector_description,
            }
        })
        .collect();
    if server_name == CODEX_APPS_MCP_SERVER_NAME {
        return Ok(filter_disallowed_codex_apps_tools(tools));
    }
    Ok(tools)
}

fn validate_mcp_server_name(server_name: &str) -> Result<()> {
    let re = regex_lite::Regex::new(r"^[a-zA-Z0-9_-]+$")?;
    if !re.is_match(server_name) {
        return Err(anyhow!(
            "Invalid MCP server name '{server_name}': must match pattern {pattern}",
            pattern = re.as_str()
        ));
    }
    Ok(())
}

fn mcp_init_error_display(
    server_name: &str,
    entry: Option<&McpAuthStatusEntry>,
    err: &StartupOutcomeError,
) -> String {
    if let Some(McpServerTransportConfig::StreamableHttp {
        url,
        bearer_token_env_var,
        http_headers,
        ..
    }) = &entry.map(|entry| &entry.config.transport)
        && url == "https://api.githubcopilot.com/mcp/"
        && bearer_token_env_var.is_none()
        && http_headers.as_ref().map(HashMap::is_empty).unwrap_or(true)
    {
        format!(
            "GitHub MCP does not support OAuth. Log in by adding a personal access token (https://github.com/settings/personal-access-tokens) to your environment and config.toml:\n[mcp_servers.{server_name}]\nbearer_token_env_var = CODEX_GITHUB_PERSONAL_ACCESS_TOKEN"
        )
    } else if is_mcp_client_auth_required_error(err) {
        format!(
            "The {server_name} MCP server is not logged in. Run `codex mcp login {server_name}`."
        )
    } else if is_mcp_client_startup_timeout_error(err) {
        let startup_timeout_secs = match entry {
            Some(entry) => match entry.config.startup_timeout_sec {
                Some(timeout) => timeout,
                None => DEFAULT_STARTUP_TIMEOUT,
            },
            None => DEFAULT_STARTUP_TIMEOUT,
        }
        .as_secs();
        format!(
            "MCP client for `{server_name}` timed out after {startup_timeout_secs} seconds. Add or adjust `startup_timeout_sec` in your config.toml:\n[mcp_servers.{server_name}]\nstartup_timeout_sec = XX"
        )
    } else {
        format!("MCP client for `{server_name}` failed to start: {err:#}")
    }
}

fn is_mcp_client_auth_required_error(error: &StartupOutcomeError) -> bool {
    match error {
        StartupOutcomeError::Failed { error } => error.contains("Auth required"),
        _ => false,
    }
}

fn is_mcp_client_startup_timeout_error(error: &StartupOutcomeError) -> bool {
    match error {
        StartupOutcomeError::Failed { error } => {
            error.contains("request timed out")
                || error.contains("timed out handshaking with MCP server")
        }
        _ => false,
    }
}

fn startup_outcome_error_message(error: StartupOutcomeError) -> String {
    match error {
        StartupOutcomeError::Cancelled => "MCP startup cancelled".to_string(),
        StartupOutcomeError::Failed { error } => error,
    }
}

#[cfg(test)]
mod mcp_init_error_display_tests {}

#[cfg(test)]
#[path = "mcp_connection_manager_tests.rs"]
mod tests;
