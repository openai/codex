//! RMCP client lifecycle for MCP server connections.
//!
//! This module owns startup of individual RMCP clients: building the transport,
//! initializing the server, listing raw tools, applying per-server tool filters,
//! and exposing cached startup snapshots while a client is still connecting.
//! Higher-level aggregation and resource/tool APIs live in
//! [`crate::connection_manager`].

use std::borrow::Cow;
use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use crate::codex_apps::CachedCodexAppsToolsLoad;
use crate::codex_apps::CodexAppsToolsCacheContext;
use crate::codex_apps::filter_disallowed_codex_apps_tools;
use crate::codex_apps::load_cached_codex_apps_tools;
use crate::codex_apps::load_startup_cached_codex_apps_tools_snapshot;
use crate::codex_apps::normalize_codex_apps_callable_name;
use crate::codex_apps::normalize_codex_apps_callable_namespace;
use crate::codex_apps::normalize_codex_apps_tool_title;
use crate::codex_apps::write_cached_codex_apps_tools_if_needed;
use crate::elicitation::ElicitationRequestManager;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::mcp::ToolPluginProvenance;
use crate::runtime::McpRuntimeEnvironment;
use crate::runtime::emit_duration;
use crate::tools::ToolFilter;
use crate::tools::ToolInfo;
use crate::tools::filter_tools;
use crate::tools::tool_with_model_visible_input_schema;
use anyhow::Result;
use anyhow::anyhow;
use async_channel::Sender;
use codex_api::SharedAuthProvider;
use codex_async_utils::CancelErr;
use codex_async_utils::OrCancelExt;
use codex_config::McpServerConfig;
use codex_config::McpServerTransportConfig;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::HttpClient;
use codex_exec_server::ReqwestHttpClient;
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
use rmcp::model::ProtocolVersion;
use rmcp::model::Tool as RmcpTool;
use tokio_util::sync::CancellationToken;
use tracing::warn;

/// MCP server capability indicating that Codex should include [`SandboxState`]
/// in tool-call request `_meta` under this key.
pub const MCP_SANDBOX_STATE_META_CAPABILITY: &str = "codex/sandbox-state-meta";

pub(crate) const MCP_TOOLS_LIST_DURATION_METRIC: &str = "codex.mcp.tools.list.duration_ms";
pub(crate) const MCP_TOOLS_FETCH_UNCACHED_DURATION_METRIC: &str =
    "codex.mcp.tools.fetch_uncached.duration_ms";
pub(crate) const DEFAULT_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
pub(crate) const DEFAULT_TOOL_TIMEOUT: Duration = Duration::from_secs(120);

const UNTRUSTED_CONNECTOR_META_KEYS: &[&str] = &[
    "connector_id",
    "connector_name",
    "connector_display_name",
    "connector_description",
    "connectorDescription",
];

#[derive(Clone)]
pub(crate) struct ManagedClient {
    pub(crate) client: Arc<RmcpClient>,
    pub(crate) tools: Vec<ToolInfo>,
    pub(crate) tool_filter: ToolFilter,
    pub(crate) tool_timeout: Option<Duration>,
    pub(crate) server_instructions: Option<String>,
    pub(crate) server_supports_sandbox_state_meta_capability: bool,
    pub(crate) codex_apps_tools_cache_context: Option<CodexAppsToolsCacheContext>,
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
}

#[derive(Clone)]
pub(crate) struct AsyncManagedClient {
    pub(crate) client: Shared<BoxFuture<'static, Result<ManagedClient, StartupOutcomeError>>>,
    pub(crate) startup_snapshot: Option<Vec<ToolInfo>>,
    pub(crate) startup_complete: Arc<AtomicBool>,
    pub(crate) tool_plugin_provenance: Arc<ToolPluginProvenance>,
    pub(crate) cancel_token: CancellationToken,
}

impl AsyncManagedClient {
    // Keep this constructor flat so the startup inputs remain readable at the
    // single call site instead of introducing a one-off params wrapper.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        server_name: String,
        config: McpServerConfig,
        store_mode: OAuthCredentialsStoreMode,
        cancel_token: CancellationToken,
        tx_event: Sender<Event>,
        elicitation_requests: ElicitationRequestManager,
        codex_apps_tools_cache_context: Option<CodexAppsToolsCacheContext>,
        tool_plugin_provenance: Arc<ToolPluginProvenance>,
        runtime_environment: McpRuntimeEnvironment,
        runtime_auth_provider: Option<SharedAuthProvider>,
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
        let cancel_token_for_fut = cancel_token.clone();
        let fut = async move {
            let outcome = match async {
                if let Err(error) = validate_mcp_server_name(&server_name) {
                    return Err(error.into());
                }

                let client = Arc::new(
                    make_rmcp_client(
                        &server_name,
                        config.clone(),
                        store_mode,
                        runtime_environment,
                        runtime_auth_provider,
                    )
                    .await?,
                );
                start_server_task(
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
                .await
            }
            .or_cancel(&cancel_token_for_fut)
            .await
            {
                Ok(result) => result,
                Err(CancelErr::Cancelled) => Err(StartupOutcomeError::Cancelled),
            };

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
            cancel_token,
        }
    }

    pub(crate) async fn client(&self) -> Result<ManagedClient, StartupOutcomeError> {
        self.client.clone().await
    }

    pub(crate) async fn shutdown(&self) {
        self.cancel_token.cancel();
        match self.client().await {
            Ok(client) => client.client.shutdown().await,
            Err(StartupOutcomeError::Cancelled) => {}
            Err(error) => {
                warn!("failed to initialize MCP client during shutdown: {error:#}");
            }
        }
    }

    fn startup_snapshot_while_initializing(&self) -> Option<Vec<ToolInfo>> {
        if !self.startup_complete.load(Ordering::Acquire) {
            return self.startup_snapshot.clone();
        }
        None
    }

    pub(crate) async fn listed_tools(&self) -> Option<Vec<ToolInfo>> {
        let annotate_tools = |tools: Vec<ToolInfo>| {
            let mut tools = tools;
            for tool in &mut tools {
                if tool.server_name == CODEX_APPS_MCP_SERVER_NAME {
                    tool.tool = tool_with_model_visible_input_schema(&tool.tool);
                }

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
}

#[derive(Debug, Clone, thiserror::Error)]
pub(crate) enum StartupOutcomeError {
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

pub(crate) fn elicitation_capability_for_server(
    _server_name: &str,
) -> Option<ElicitationCapability> {
    // https://modelcontextprotocol.io/specification/2025-06-18/client/elicitation#capabilities
    // indicates this should be an empty object.
    Some(ElicitationCapability::default())
}

pub(crate) async fn list_tools_for_client_uncached(
    server_name: &str,
    client: &Arc<RmcpClient>,
    timeout: Option<Duration>,
    server_instructions: Option<&str>,
) -> Result<Vec<ToolInfo>> {
    let resp = client
        .list_tools_with_connector_ids(/*params*/ None, timeout)
        .await?;
    let tools = resp
        .tools
        .into_iter()
        .map(|tool| {
            let mut tool_def = tool.tool;
            let (connector_id, connector_name, connector_description) =
                sanitize_tool_connector_metadata(
                    server_name,
                    &mut tool_def,
                    tool.connector_id,
                    tool.connector_name,
                    tool.connector_description,
                );
            let callable_name = normalize_codex_apps_callable_name(
                server_name,
                &tool_def.name,
                connector_id.as_deref(),
                connector_name.as_deref(),
            );
            let callable_namespace =
                normalize_codex_apps_callable_namespace(server_name, connector_name.as_deref());
            if let Some(title) = tool_def.title.as_deref() {
                let normalized_title =
                    normalize_codex_apps_tool_title(server_name, connector_name.as_deref(), title);
                if tool_def.title.as_deref() != Some(normalized_title.as_str()) {
                    tool_def.title = Some(normalized_title);
                }
            }
            ToolInfo {
                server_name: server_name.to_owned(),
                callable_name,
                callable_namespace,
                server_instructions: server_instructions.map(str::to_string),
                tool: tool_def,
                connector_id,
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

fn sanitize_tool_connector_metadata(
    server_name: &str,
    tool: &mut RmcpTool,
    connector_id: Option<String>,
    connector_name: Option<String>,
    connector_description: Option<String>,
) -> (Option<String>, Option<String>, Option<String>) {
    if server_name == CODEX_APPS_MCP_SERVER_NAME {
        return (connector_id, connector_name, connector_description);
    }

    strip_untrusted_connector_meta(tool);
    (None, None, None)
}

fn strip_untrusted_connector_meta(tool: &mut RmcpTool) {
    if let Some(meta) = tool.meta.as_mut() {
        meta.retain(|key, _| !is_untrusted_connector_meta_key(key));
    }
}

fn is_untrusted_connector_meta_key(key: &str) -> bool {
    UNTRUSTED_CONNECTOR_META_KEYS.contains(&key)
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

    let server_supports_sandbox_state_meta_capability = initialize_result
        .capabilities
        .experimental
        .as_ref()
        .and_then(|exp| exp.get(MCP_SANDBOX_STATE_META_CAPABILITY))
        .is_some();
    let list_start = Instant::now();
    let fetch_start = Instant::now();
    let tools = list_tools_for_client_uncached(
        &server_name,
        &client,
        startup_timeout,
        initialize_result.instructions.as_deref(),
    )
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

    let managed = ManagedClient {
        client: Arc::clone(&client),
        tools,
        tool_timeout: Some(tool_timeout),
        tool_filter,
        server_instructions: initialize_result.instructions,
        server_supports_sandbox_state_meta_capability,
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
    config: McpServerConfig,
    store_mode: OAuthCredentialsStoreMode,
    runtime_environment: McpRuntimeEnvironment,
    runtime_auth_provider: Option<SharedAuthProvider>,
) -> Result<RmcpClient, StartupOutcomeError> {
    let McpServerConfig {
        transport,
        experimental_environment,
        ..
    } = config;
    let remote_environment = match experimental_environment.as_deref() {
        None | Some("local") => false,
        Some("remote") => {
            if !runtime_environment.environment().is_remote() {
                return Err(StartupOutcomeError::from(anyhow!(
                    "remote MCP server `{server_name}` requires a remote environment"
                )));
            }
            true
        }
        Some(environment) => {
            return Err(StartupOutcomeError::from(anyhow!(
                "unsupported experimental_environment `{environment}` for MCP server `{server_name}`"
            )));
        }
    };

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
            let launcher = if remote_environment {
                Arc::new(ExecutorStdioServerLauncher::new(
                    runtime_environment.environment().get_exec_backend(),
                    runtime_environment.fallback_cwd(),
                ))
            } else {
                Arc::new(LocalStdioServerLauncher::new(
                    runtime_environment.fallback_cwd(),
                )) as Arc<dyn StdioServerLauncher>
            };

            // `RmcpClient` always sees a launched MCP stdio server. The
            // launcher hides whether that means a local child process or an
            // executor process whose stdin/stdout bytes cross the process API.
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
            let http_client: Arc<dyn HttpClient> = if remote_environment {
                runtime_environment.environment().get_http_client()
            } else {
                Arc::new(ReqwestHttpClient)
            };
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
    use rmcp::model::JsonObject;
    use rmcp::model::Meta;

    fn tool_with_connector_meta() -> RmcpTool {
        RmcpTool {
            name: "capture_file_upload".to_string().into(),
            title: None,
            description: Some("test tool".to_string().into()),
            input_schema: Arc::new(JsonObject::default()),
            output_schema: None,
            annotations: None,
            execution: None,
            icons: None,
            meta: Some(Meta(
                serde_json::json!({
                    "connector_id": "connector_gmail",
                    "connector_name": "Gmail",
                    "connector_display_name": "Gmail",
                    "connector_description": "Mail connector",
                    "connectorDescription": "Mail connector",
                    "connectorFutureField": "future connector metadata",
                    "CONNECTOR_UPPERCASE": "uppercase connector metadata",
                    "openai/fileParams": ["file"],
                    "custom": "kept"
                })
                .as_object()
                .expect("object")
                .clone(),
            )),
        }
    }

    #[test]
    fn custom_mcp_connector_metadata_is_stripped() {
        let mut tool = tool_with_connector_meta();

        let (connector_id, connector_name, connector_description) =
            sanitize_tool_connector_metadata(
                "minimaltest",
                &mut tool,
                Some("connector_gmail".to_string()),
                Some("Gmail".to_string()),
                Some("Mail connector".to_string()),
            );

        assert_eq!(connector_id, None);
        assert_eq!(connector_name, None);
        assert_eq!(connector_description, None);

        let meta = tool.meta.as_ref().expect("meta");
        for key in [
            "connector_id",
            "connector_name",
            "connector_display_name",
            "connector_description",
            "connectorDescription",
        ] {
            assert!(!meta.0.contains_key(key), "{key} should be stripped");
        }
        assert!(meta.0.contains_key("connectorFutureField"));
        assert!(meta.0.contains_key("CONNECTOR_UPPERCASE"));
        assert!(meta.0.contains_key("openai/fileParams"));
        assert_eq!(
            meta.0.get("custom").and_then(|value| value.as_str()),
            Some("kept")
        );
    }

    #[test]
    fn codex_apps_connector_metadata_is_preserved() {
        let mut tool = tool_with_connector_meta();

        let (connector_id, connector_name, connector_description) =
            sanitize_tool_connector_metadata(
                CODEX_APPS_MCP_SERVER_NAME,
                &mut tool,
                Some("connector_gmail".to_string()),
                Some("Gmail".to_string()),
                Some("Mail connector".to_string()),
            );

        assert_eq!(connector_id.as_deref(), Some("connector_gmail"));
        assert_eq!(connector_name.as_deref(), Some("Gmail"));
        assert_eq!(connector_description.as_deref(), Some("Mail connector"));

        let meta = tool.meta.as_ref().expect("meta");
        for key in [
            "connector_id",
            "connector_name",
            "connector_display_name",
            "connector_description",
            "connectorDescription",
            "connectorFutureField",
            "CONNECTOR_UPPERCASE",
        ] {
            assert!(meta.0.contains_key(key), "{key} should be preserved");
        }
    }
}
