//! RMCP client lifecycle for MCP server connections.
//!
//! This module owns startup of individual RMCP clients: building the transport,
//! initializing the server, listing raw tools, applying per-server tool filters,
//! and reporting startup state while a client is still connecting.
//! Higher-level aggregation and resource/tool APIs live in
//! [`crate::connection_manager`].

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use std::sync::Arc;
use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::elicitation::ElicitationRequestManager;
use crate::mcp::ToolPluginProvenance;
use crate::runtime::McpRuntimeContext;
use crate::runtime::emit_duration;
use crate::server::EffectiveMcpServer;
use crate::tools::ToolFilter;
use crate::tools::ToolInfo;
use crate::tools::filter_tools;
use anyhow::Result;
use anyhow::anyhow;
use async_channel::Sender;
use codex_api::SharedAuthProvider;
use codex_async_utils::CancelErr;
use codex_async_utils::OrCancelExt;
use codex_config::McpServerConfig;
use codex_config::McpServerTransportConfig;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::HttpClient;
use codex_exec_server::ReqwestHttpClient;
use codex_protocol::mcp::McpServerInfo;
use codex_protocol::protocol::Event;
use codex_rmcp_client::ExecutorStdioServerLauncher;
use codex_rmcp_client::LocalStdioServerLauncher;
use codex_rmcp_client::RmcpClient;
use codex_rmcp_client::StdioServerLauncher;
use futures::future::BoxFuture;
use futures::future::FutureExt;
use futures::future::Shared;
use rmcp::model::ClientCapabilities;
use rmcp::model::ElicitationCapability;
use rmcp::model::Implementation;
use rmcp::model::InitializeRequestParams;
use rmcp::model::JsonObject;
use rmcp::model::ProtocolVersion;
use rmcp::model::Tool as RmcpTool;
use rmcp::transport::auth::AuthError;
use tokio_util::sync::CancellationToken;
use tracing::Instrument;
use tracing::instrument;
use tracing::warn;

/// MCP server capability indicating that Codex should include [`SandboxState`]
/// in tool-call request `_meta` under this key.
pub const MCP_SANDBOX_STATE_META_CAPABILITY: &str = "codex/sandbox-state-meta";
pub const MCP_TOOL_INPUT_META_CAPABILITY: &str = "codex/tool-input-meta";
pub const OPENAI_FORM_CAPABILITY: &str = "openai/form";

pub(crate) const MCP_TOOLS_FETCH_UNCACHED_DURATION_METRIC: &str =
    "codex.mcp.tools.fetch_uncached.duration_ms";
pub(crate) const DEFAULT_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
pub(crate) const DEFAULT_TOOL_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Clone)]
pub(crate) struct ManagedClient {
    pub(crate) client: Arc<RmcpClient>,
    pub(crate) server_info: McpServerInfo,
    pub(crate) tools: Vec<ToolInfo>,
    pub(crate) tool_filter: ToolFilter,
    pub(crate) tool_timeout: Option<Duration>,
    pub(crate) server_supports_sandbox_state_meta_capability: bool,
    pub(crate) server_supports_tool_input_meta_capability: bool,
}

#[derive(Clone)]
pub(crate) struct AsyncManagedClient {
    pub(crate) client: Shared<BoxFuture<'static, Result<ManagedClient, StartupOutcomeError>>>,
    startup_state: Arc<AtomicU8>,
    lifecycle: Arc<AsyncManagedClientLifecycle>,
}

const STARTUP_PENDING: u8 = 0;
const STARTUP_SUCCEEDED: u8 = 1;
const STARTUP_TERMINAL_ERROR: u8 = 2;

struct AsyncManagedClientLifecycle {
    cancel_token: CancellationToken,
}

impl Drop for AsyncManagedClientLifecycle {
    fn drop(&mut self) {
        self.cancel_token.cancel();
    }
}

impl AsyncManagedClient {
    #[cfg(test)]
    pub(crate) fn for_test(
        client: Shared<BoxFuture<'static, Result<ManagedClient, StartupOutcomeError>>>,
        cancel_token: CancellationToken,
    ) -> Self {
        Self {
            client,
            startup_state: Arc::new(AtomicU8::new(STARTUP_PENDING)),
            lifecycle: Arc::new(AsyncManagedClientLifecycle { cancel_token }),
        }
    }

    // Keep this constructor flat so the startup inputs remain readable at the
    // single call site instead of introducing a one-off params wrapper.
    #[instrument(level = "trace", skip_all, fields(server_name = %server_name))]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        server_name: String,
        server: EffectiveMcpServer,
        store_mode: OAuthCredentialsStoreMode,
        keyring_backend_kind: AuthKeyringBackendKind,
        cancel_token: CancellationToken,
        tx_event: Sender<Event>,
        elicitation_requests: ElicitationRequestManager,
        runtime_context: McpRuntimeContext,
        runtime_auth_provider: Option<SharedAuthProvider>,
        client_elicitation_capability: ElicitationCapability,
        supports_openai_form_elicitation: bool,
    ) -> Self {
        let tool_filter = ToolFilter::from_config(server.config());
        let startup_tool_filter = tool_filter;
        let startup_state = Arc::new(AtomicU8::new(STARTUP_PENDING));
        let startup_state_for_fut = Arc::clone(&startup_state);
        let cancel_token_for_fut = cancel_token.clone();
        let fut = async move {
            let outcome = match async {
                if let Err(error) = validate_mcp_server_name(&server_name) {
                    return Err(error.into());
                }

                let record_physical_tools_list_metric = server
                    .runtime_metadata()
                    .records_physical_tools_list_metric();

                let client = Arc::new(
                    make_rmcp_client(
                        &server_name,
                        server.clone(),
                        store_mode,
                        keyring_backend_kind,
                        runtime_context,
                        runtime_auth_provider,
                    )
                    .await?,
                );
                start_server_task(
                    server_name,
                    client,
                    StartServerTaskParams {
                        startup_timeout: server
                            .config()
                            .startup_timeout_sec
                            .or(Some(DEFAULT_STARTUP_TIMEOUT)),
                        tool_timeout: server
                            .config()
                            .tool_timeout_sec
                            .unwrap_or(DEFAULT_TOOL_TIMEOUT),
                        tool_filter: startup_tool_filter,
                        tx_event,
                        elicitation_requests,
                        client_elicitation_capability,
                        supports_openai_form_elicitation,
                        record_physical_tools_list_metric,
                    },
                )
                .await
            }
            .or_cancel(&cancel_token_for_fut)
            .await
            {
                Ok(result) => result,
                Err(CancelErr::Cancelled) => Err(StartupOutcomeError::Cancelled),
            };

            startup_state_for_fut.store(
                if outcome.is_ok() {
                    STARTUP_SUCCEEDED
                } else {
                    STARTUP_TERMINAL_ERROR
                },
                Ordering::Release,
            );
            outcome
        };
        let client = fut.in_current_span().boxed().shared();

        Self {
            client,
            startup_state,
            lifecycle: Arc::new(AsyncManagedClientLifecycle { cancel_token }),
        }
    }

    pub(crate) async fn client(&self) -> Result<ManagedClient, StartupOutcomeError> {
        self.client.clone().await
    }

    pub(crate) async fn shutdown(&self) {
        self.lifecycle.cancel_token.cancel();
        match self.client().await {
            Ok(client) => client.client.shutdown().await,
            Err(StartupOutcomeError::Cancelled) => {}
            Err(error) => {
                warn!("failed to initialize MCP client during shutdown: {error:#}");
            }
        }
    }

    pub(crate) async fn listed_tools(&self) -> Option<Vec<ToolInfo>> {
        let client = self.client().await.ok()?;
        let namespace_title = client.server_info.title.clone();
        let mut tools = client.tools;
        for tool in &mut tools {
            tool.namespace_title = namespace_title.clone();
        }
        Some(tools)
    }

    pub(crate) fn same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.lifecycle, &other.lifecycle)
    }

    pub(crate) fn can_reuse(&self) -> bool {
        !self.lifecycle.cancel_token.is_cancelled()
            && self.startup_state.load(Ordering::Acquire) != STARTUP_TERMINAL_ERROR
    }

    pub(crate) fn startup_is_complete(&self) -> bool {
        self.startup_state.load(Ordering::Acquire) != STARTUP_PENDING
    }

    pub(crate) fn cancel_startup(&self) {
        self.lifecycle.cancel_token.cancel();
    }

    #[cfg(test)]
    pub(crate) fn startup_is_cancelled(&self) -> bool {
        self.lifecycle.cancel_token.is_cancelled()
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub(crate) enum StartupOutcomeError {
    #[error("MCP startup cancelled")]
    Cancelled,
    // We can't store the original error here because anyhow::Error doesn't implement
    // `Clone`.
    #[error("MCP startup failed: {error}")]
    Failed {
        error: String,
        is_authentication_required: bool,
    },
}

impl StartupOutcomeError {
    pub(crate) fn is_authentication_required(&self) -> bool {
        match self {
            Self::Cancelled => false,
            Self::Failed {
                is_authentication_required,
                ..
            } => *is_authentication_required,
        }
    }
}

impl From<anyhow::Error> for StartupOutcomeError {
    fn from(error: anyhow::Error) -> Self {
        let is_authentication_required = error.chain().any(|source| {
            source
                .downcast_ref::<AuthError>()
                .is_some_and(|auth_error| {
                    matches!(
                        auth_error,
                        AuthError::AuthorizationRequired | AuthError::TokenExpired
                    )
                })
        });
        Self::Failed {
            error: error.to_string(),
            is_authentication_required,
        }
    }
}

#[instrument(level = "trace", skip_all, fields(server_name = %server_name))]
pub(crate) async fn list_tools_for_client_uncached(
    server_name: &str,
    client: &Arc<RmcpClient>,
    timeout: Option<Duration>,
    server_instructions: Option<&str>,
) -> Result<Vec<ToolInfo>> {
    let resp = client.list_tools(/*params*/ None, timeout).await?;
    let tools = resp
        .tools
        .into_iter()
        .map(|tool| regular_mcp_tool_info_from_listed_tool(server_name, server_instructions, tool))
        .collect();
    Ok(tools)
}

/// Stores plugin names on the tool and appends a model-visible plugin membership note.
pub(crate) fn add_plugin_provenance_to_tool(tool: &mut ToolInfo, plugin_names: &[String]) {
    tool.plugin_display_names = plugin_names.to_vec();
    if plugin_names.is_empty() {
        return;
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

/// Adds server-scoped plugin names to regular MCP tools without changing their input schemas.
pub(crate) fn prepare_regular_mcp_tools_for_model(
    mut tools: Vec<ToolInfo>,
    tool_plugin_provenance: &ToolPluginProvenance,
) -> Vec<ToolInfo> {
    for tool in &mut tools {
        let plugin_names = tool_plugin_provenance
            .plugin_display_names_for_mcp_server_name(tool.server_name.as_str());
        add_plugin_provenance_to_tool(tool, plugin_names);
    }
    tools
}

/// Converts an MCP tool while keeping its raw name and protocol metadata intact.
fn regular_mcp_tool_info_from_listed_tool(
    server_name: &str,
    server_instructions: Option<&str>,
    tool_def: RmcpTool,
) -> ToolInfo {
    ToolInfo {
        server_name: server_name.to_owned(),
        supports_parallel_tool_calls: false,
        server_origin: None,
        callable_name: tool_def.name.to_string(),
        callable_namespace: server_name.to_string(),
        namespace_description: server_instructions.map(str::to_string),
        namespace_title: None,
        search_aliases: Vec::new(),
        tool: tool_def,
        plugin_display_names: Vec::new(),
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

#[instrument(level = "trace", skip_all, fields(server_name = %server_name))]
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
        client_elicitation_capability,
        supports_openai_form_elicitation,
        record_physical_tools_list_metric,
    } = params;
    let params = mcp_initialize_request_params(
        client_elicitation_capability,
        supports_openai_form_elicitation,
    );

    let send_elicitation = elicitation_requests.make_sender(server_name.clone(), tx_event);

    let initialize_result = client
        .initialize(params, startup_timeout, send_elicitation)
        .await
        .map_err(StartupOutcomeError::from)?;

    let server_supports_sandbox_state_meta_capability = initialize_result
        .capabilities
        .experimental
        .as_ref()
        .and_then(|exp| exp.get(MCP_SANDBOX_STATE_META_CAPABILITY))
        .is_some();
    let server_supports_tool_input_meta_capability = initialize_result
        .capabilities
        .experimental
        .as_ref()
        .and_then(|exp| exp.get(MCP_TOOL_INPUT_META_CAPABILITY))
        .is_some();
    let fetch_start = std::time::Instant::now();
    let tools = list_tools_for_client_uncached(
        &server_name,
        &client,
        startup_timeout,
        initialize_result.instructions.as_deref(),
    )
    .await
    .map_err(StartupOutcomeError::from)?;
    if record_physical_tools_list_metric {
        emit_duration(
            MCP_TOOLS_FETCH_UNCACHED_DURATION_METRIC,
            fetch_start.elapsed(),
            &[],
        );
    }
    let server_info = mcp_server_info_from_implementation(initialize_result.server_info);
    let tools = filter_tools(tools, &tool_filter);

    let managed = ManagedClient {
        client: Arc::clone(&client),
        server_info,
        tools,
        tool_timeout: Some(tool_timeout),
        tool_filter,
        server_supports_sandbox_state_meta_capability,
        server_supports_tool_input_meta_capability,
    };

    Ok(managed)
}

fn mcp_initialize_request_params(
    client_elicitation_capability: ElicitationCapability,
    supports_openai_form_elicitation: bool,
) -> InitializeRequestParams {
    let mut capabilities = ClientCapabilities::default();
    capabilities.elicitation = Some(client_elicitation_capability);
    if supports_openai_form_elicitation {
        capabilities.extensions = Some(BTreeMap::from([(
            OPENAI_FORM_CAPABILITY.to_string(),
            JsonObject::new(),
        )]));
    }
    InitializeRequestParams::new(
        capabilities,
        Implementation::new("codex-mcp-client", env!("CARGO_PKG_VERSION")).with_title("Codex"),
    )
    .with_protocol_version(ProtocolVersion::V_2025_06_18)
}

fn mcp_server_info_from_implementation(server_info: Implementation) -> McpServerInfo {
    McpServerInfo {
        name: server_info.name,
        title: server_info.title,
        version: server_info.version,
        description: server_info.description,
        icons: server_info.icons.map(|icons| {
            icons
                .into_iter()
                .filter_map(|icon| serde_json::to_value(icon).ok())
                .collect()
        }),
        website_url: server_info.website_url,
    }
}

struct StartServerTaskParams {
    startup_timeout: Option<Duration>, // TODO: cancel_token should handle this.
    tool_timeout: Duration,
    tool_filter: ToolFilter,
    tx_event: Sender<Event>,
    elicitation_requests: ElicitationRequestManager,
    client_elicitation_capability: ElicitationCapability,
    supports_openai_form_elicitation: bool,
    record_physical_tools_list_metric: bool,
}

#[instrument(level = "trace", skip_all, fields(server_name = %server_name))]
async fn make_rmcp_client(
    server_name: &str,
    server: EffectiveMcpServer,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
    runtime_context: McpRuntimeContext,
    runtime_auth_provider: Option<SharedAuthProvider>,
) -> Result<RmcpClient, StartupOutcomeError> {
    let runtime_bearer_token = server.runtime_bearer_token().map(str::to_string);
    let config = server.config().clone();
    let resolved_environment = runtime_context
        .resolve_server_environment(server_name, &config)
        .map_err(|err| StartupOutcomeError::from(anyhow!(err)))?;
    let is_local_environment = config.is_local_environment();
    let McpServerConfig { transport, .. } = config;

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
            let launcher = if is_local_environment {
                // TODO(starr): Unify local stdio MCP launch with
                // `ExecutorStdioServerLauncher` once the executor-backed path
                // preserves `LocalStdioServerLauncher` semantics.
                Arc::new(LocalStdioServerLauncher::new(
                    runtime_context.local_stdio_fallback_cwd(),
                )) as Arc<dyn StdioServerLauncher>
            } else {
                let Some(environment) = resolved_environment.as_ref() else {
                    unreachable!(
                        "non-local stdio MCP servers resolve an environment before launch"
                    );
                };
                Arc::new(ExecutorStdioServerLauncher::new(
                    environment.get_exec_backend(),
                )) as Arc<dyn StdioServerLauncher>
            };

            let cwd = cwd.map(codex_utils_path_uri::LegacyAppPathString::into_string);
            RmcpClient::new_stdio_client(command_os, args_os, env_os, &env_vars, cwd, launcher)
                .await
                .map_err(|err| StartupOutcomeError::from(anyhow!(err)))
        }
        McpServerTransportConfig::StreamableHttp {
            url,
            http_headers,
            env_http_headers,
            bearer_token_env_var,
        } => {
            let http_client = resolved_environment.as_ref().map_or_else(
                || Arc::new(ReqwestHttpClient) as Arc<dyn HttpClient>,
                |environment| environment.get_http_client(),
            );
            let resolved_bearer_token = match runtime_bearer_token {
                Some(token) => Some(token),
                None => match resolve_bearer_token(server_name, bearer_token_env_var.as_deref()) {
                    Ok(token) => token,
                    Err(error) => return Err(error.into()),
                },
            };
            RmcpClient::new_streamable_http_client(
                server_name,
                &url,
                resolved_bearer_token,
                http_headers,
                env_http_headers,
                store_mode,
                keyring_backend_kind,
                http_client,
                runtime_auth_provider,
            )
            .await
            .map_err(StartupOutcomeError::from)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use rmcp::model::JsonObject;
    use rmcp::model::Meta;

    #[test]
    fn startup_outcome_error_identifies_authentication_required() {
        let error = anyhow::Error::new(AuthError::AuthorizationRequired)
            .context("failed to initialize MCP server");

        let error = StartupOutcomeError::from(error);

        assert!(error.is_authentication_required());
    }

    #[test]
    fn mcp_initialize_advertises_openai_form_only_when_supported() {
        let unsupported = mcp_initialize_request_params(
            ElicitationCapability::default(),
            /*supports_openai_form_elicitation*/ false,
        );
        assert_eq!(unsupported.capabilities.extensions, None);

        let supported = mcp_initialize_request_params(
            ElicitationCapability::default(),
            /*supports_openai_form_elicitation*/ true,
        );
        assert_eq!(
            supported.capabilities.extensions,
            Some(BTreeMap::from([(
                OPENAI_FORM_CAPABILITY.to_string(),
                JsonObject::new(),
            )]))
        );
    }

    #[test]
    fn listed_tool_conversion_preserves_protocol_metadata() {
        let tool =
            RmcpTool::new("search", "test tool", Arc::new(JsonObject::default())).with_meta(Meta(
                serde_json::json!({"vendor.example/hint": "kept"})
                    .as_object()
                    .expect("object")
                    .clone(),
            ));

        let info = regular_mcp_tool_info_from_listed_tool("server", Some("instructions"), tool);
        let meta = info.tool.meta.as_ref().expect("meta");
        assert_eq!(
            meta.0
                .get("vendor.example/hint")
                .and_then(|value| value.as_str()),
            Some("kept")
        );
    }
}
