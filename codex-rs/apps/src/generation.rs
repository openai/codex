use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Weak;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID;
use codex_config::McpServerAuth;
use codex_config::McpServerConfig;
use codex_config::McpServerTransportConfig;
use codex_connectors::metadata::CODEX_APPS_MCP_SERVER_NAME;
use codex_connectors::metadata::connector_mcp_server_name;
use codex_mcp::EffectiveMcpServer;
use codex_mcp::McpServerRuntimeMetadata;
use rmcp::model::Meta;
use rmcp::model::Tool;
use serde_json::Value as JsonValue;
use tokio_util::sync::CancellationToken;

use crate::AppsRefreshCoordinator;
use crate::AppsUpstream;
use crate::CodexAppsAccessGuard;
use crate::connector_server::CodexAppServer;
use crate::connector_server::ConnectorServerContext;
use crate::file_upload::AppsFileSupport;
use crate::http::AppsHttpServer;
use crate::names::allocate_deterministic_names;
use crate::resource_server::CodexAppsResourceServer;
use crate::upstream::CODEX_APPS_RESOURCE_MCP_SERVER_NAME;

const META_CONNECTOR_ID: &str = "connector_id";
const META_CONNECTOR_NAME: &str = "connector_name";
const META_CONNECTOR_DESCRIPTION: &str = "connector_description";

/// A handle to one Apps endpoint generation and its live inventory.
#[derive(Clone)]
pub struct CodexAppsSnapshot {
    pub(super) owner: Arc<CodexAppsSnapshotOwner>,
}

pub(super) struct CodexAppsSnapshotOwner {
    pub(super) generation: Arc<CodexAppsGeneration>,
    // Connector servers hold this coordinator weakly to avoid a coordinator/generation cycle.
    // Keeping it beside the pinned generation preserves refresh behavior for as long as an
    // effective MCP registration from this snapshot remains alive.
    pub(super) _refresh_coordinator: Arc<AppsRefreshCoordinator>,
}

impl CodexAppsSnapshot {
    #[cfg(test)]
    pub(crate) fn loopback_bearer_token_for_test(&self, server_name: &str) -> String {
        self.owner
            .generation
            .http_server
            .bearer_token(server_name)
            .expect("loopback route bearer")
            .to_string()
    }

    /// Returns whether this generation was built from a successful live upstream inventory.
    ///
    /// A cached generation remains usable while its owner refreshes in the background, but hosts
    /// can use this fact to wait for a later live generation when freshness matters.
    pub fn is_live_inventory(&self) -> bool {
        self.owner.generation.inventory_provenance == InventoryProvenance::Live
    }

    /// Returns installed Apps that have at least one non-synthetic tool.
    pub fn apps(&self) -> &[CodexApp] {
        &self.owner.generation.inventory.apps
    }

    /// Returns every discovered connector, including connectors represented only by synthetic
    /// link tools and therefore omitted from [`Self::apps`].
    pub fn all_connectors(&self) -> &[CodexApp] {
        &self.owner.generation.inventory.all_connectors
    }

    /// Returns ordinary configured HTTP MCP servers for this endpoint generation.
    ///
    /// Each server's bearer credential lives only in its non-serializable effective runtime state.
    /// Returned registrations retain this snapshot's listener generation.
    pub fn effective_mcp_servers(&self) -> HashMap<String, EffectiveMcpServer> {
        self.owner
            .generation
            .effective_mcp_servers
            .iter()
            .map(|(name, server)| {
                (
                    name.clone(),
                    server.clone().with_runtime_owner(Arc::clone(&self.owner)),
                )
            })
            .collect()
    }

    /// Returns the resource-only MCP proxy backed by a session-scoped upstream client.
    pub fn resource_mcp_server(&self) -> EffectiveMcpServer {
        self.owner
            .generation
            .resource_mcp_server
            .clone()
            .with_runtime_owner(Arc::clone(&self.owner))
    }

    /// Looks up trusted Apps metadata for one raw tool exposed by a connector HTTP server.
    ///
    /// The lookup is exact and uses the protocol-routing server and tool names, not their
    /// model-visible normalized names. Hosts can use this as the trust boundary for Apps-only
    /// behavior without teaching the generic MCP manager about connectors.
    pub fn tool_metadata(
        &self,
        server_name: &str,
        tool_name: &str,
    ) -> Option<&CodexAppToolMetadata> {
        self.owner
            .generation
            .inventory
            .tool_metadata
            .get(server_name)
            .and_then(|tools| tools.get(tool_name))
    }

    /// Returns every raw tool currently exposed by this generation.
    pub fn tools(&self) -> impl Iterator<Item = (&str, &str, &CodexAppToolMetadata)> {
        self.owner
            .generation
            .inventory
            .tool_metadata
            .iter()
            .flat_map(|(server_name, tools)| {
                tools.iter().map(move |(tool_name, metadata)| {
                    (server_name.as_str(), tool_name.as_str(), metadata)
                })
            })
    }
}

/// Direct, connector-level inventory exposed without consulting the MCP manager.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexApp {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) description: Option<String>,
    pub(super) mcp_server_name: String,
}

impl CodexApp {
    /// Returns the stable upstream connector identifier.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the connector's display name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the connector description supplied by the upstream Apps server.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Returns the logical name of this connector's MCP server.
    pub fn mcp_server_name(&self) -> &str {
        &self.mcp_server_name
    }
}

/// Trusted source metadata for one tool exposed by a connector's MCP server.
#[derive(Clone, Debug, PartialEq)]
pub struct CodexAppToolMetadata {
    pub(super) connector_id: String,
    pub(super) connector_name: String,
    pub(super) connector_description: Option<String>,
    pub(super) upstream_tool_name: String,
    pub(super) tool_title: Option<String>,
    pub(super) destructive_hint: Option<bool>,
    pub(super) open_world_hint: Option<bool>,
    pub(super) link_id: Option<String>,
    pub(super) mcp_app_resource_uri: Option<String>,
    pub(super) template_id: Option<String>,
    pub(super) action_name: Option<String>,
}

impl CodexAppToolMetadata {
    pub fn connector_id(&self) -> &str {
        &self.connector_id
    }

    pub fn connector_name(&self) -> &str {
        &self.connector_name
    }

    pub fn connector_description(&self) -> Option<&str> {
        self.connector_description.as_deref()
    }

    /// Returns the name understood by the hosted Apps MCP server.
    pub fn upstream_tool_name(&self) -> &str {
        &self.upstream_tool_name
    }

    pub fn tool_title(&self) -> Option<&str> {
        self.tool_title.as_deref()
    }

    pub fn destructive_hint(&self) -> Option<bool> {
        self.destructive_hint
    }

    pub fn open_world_hint(&self) -> Option<bool> {
        self.open_world_hint
    }

    pub fn link_id(&self) -> Option<&str> {
        self.link_id.as_deref()
    }

    pub fn mcp_app_resource_uri(&self) -> Option<&str> {
        self.mcp_app_resource_uri.as_deref()
    }

    pub fn template_id(&self) -> Option<&str> {
        self.template_id.as_deref()
    }

    pub fn action_name(&self) -> Option<&str> {
        self.action_name.as_deref()
    }
}

pub(super) struct CodexAppsGeneration {
    pub(super) inventory_provenance: InventoryProvenance,
    inventory: CodexAppsInventory,
    effective_mcp_servers: HashMap<String, EffectiveMcpServer>,
    resource_mcp_server: EffectiveMcpServer,
    http_server: AppsHttpServer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum InventoryProvenance {
    Cached,
    Live,
}

struct CodexAppsInventory {
    apps: Vec<CodexApp>,
    all_connectors: Vec<CodexApp>,
    tool_metadata: HashMap<String, HashMap<String, CodexAppToolMetadata>>,
}

pub(super) struct CodexAppsGenerationInput {
    pub(super) upstream: Arc<AppsUpstream>,
    pub(super) raw_tools: Vec<Tool>,
    pub(super) inventory_provenance: InventoryProvenance,
    pub(super) file_support: Option<Arc<AppsFileSupport>>,
    pub(super) refresh_coordinator: Weak<AppsRefreshCoordinator>,
    pub(super) access_guard: CodexAppsAccessGuard,
    pub(super) shutdown: CancellationToken,
}

impl CodexAppsGeneration {
    pub(super) async fn from_tools(input: CodexAppsGenerationInput) -> Result<Self> {
        let CodexAppsGenerationInput {
            upstream,
            raw_tools,
            inventory_provenance,
            file_support,
            refresh_coordinator,
            access_guard,
            shutdown,
        } = input;
        let mut builders = BTreeMap::<String, ConnectorServerBuilder>::new();
        for tool in raw_tools {
            if tool.name.trim().is_empty() {
                continue;
            }
            let meta = tool.meta.as_ref();
            let listed_tool = ListedAppTool {
                connector_id: app_meta_string(meta, &[META_CONNECTOR_ID]),
                connector_name: app_meta_string(
                    meta,
                    &[META_CONNECTOR_NAME, "connector_display_name"],
                ),
                connector_description: app_meta_string(
                    meta,
                    &[META_CONNECTOR_DESCRIPTION, "connectorDescription"],
                ),
                tool,
            };
            let (Some(connector_id), Some(connector_name)) =
                (listed_tool.connector_id, listed_tool.connector_name)
            else {
                continue;
            };
            let connector_id = connector_id.trim();
            let connector_name = connector_name.trim();
            if connector_id.is_empty() || connector_name.is_empty() {
                continue;
            }

            let builder = builders.entry(connector_id.to_string()).or_insert_with(|| {
                ConnectorServerBuilder {
                    connector_name: connector_name.to_string(),
                    connector_description: listed_tool.connector_description.clone(),
                    tools: Vec::new(),
                    has_non_synthetic_tool: false,
                }
            });
            if builder.connector_name != connector_name {
                bail!("connector `{connector_id}` has inconsistent names in one tool snapshot");
            }
            if builder.connector_description.is_none() {
                builder.connector_description = listed_tool.connector_description;
            }
            builder.has_non_synthetic_tool |= !is_synthetic_link(&listed_tool.tool);
            builder.tools.push(listed_tool.tool);
        }

        let connector_candidates = builders
            .into_iter()
            .map(|(connector_id, builder)| {
                let base_server_name = connector_mcp_server_name(&builder.connector_name);
                let raw_namespace_identity =
                    format!("{CODEX_APPS_MCP_SERVER_NAME}\0{base_server_name}\0{connector_id}");
                (
                    connector_id,
                    builder,
                    base_server_name,
                    raw_namespace_identity,
                )
            })
            .collect::<Vec<_>>();
        let server_names = allocate_deterministic_names(connector_candidates.iter().map(
            |(_, _, base_server_name, raw_namespace_identity)| {
                (base_server_name.as_str(), raw_namespace_identity.as_str())
            },
        ));
        let mut servers = Vec::with_capacity(connector_candidates.len());
        let mut apps = Vec::with_capacity(connector_candidates.len());
        let mut all_connectors = Vec::with_capacity(connector_candidates.len());
        for ((connector_id, builder, _base_server_name, raw_namespace_identity), server_name) in
            connector_candidates.into_iter().zip(server_names)
        {
            let server = CodexAppServer::new(
                connector_id,
                builder,
                server_name,
                raw_namespace_identity,
                ConnectorServerContext {
                    upstream: Arc::clone(&upstream),
                    file_support: file_support.clone(),
                    refresh_coordinator: refresh_coordinator.clone(),
                    access_guard: access_guard.clone(),
                    shutdown: shutdown.clone(),
                },
            );
            let connector = server.inventory_connector();
            if server.include_in_app_inventory() {
                apps.push(connector.clone());
            }
            all_connectors.push(connector);
            servers.push(server);
        }
        let tool_metadata = servers
            .iter()
            .map(|server| {
                (
                    server.server_name().to_string(),
                    server.tool_metadata().collect(),
                )
            })
            .collect();
        let resource_server = CodexAppsResourceServer {
            upstream: Arc::clone(&upstream),
            access_guard: access_guard.clone(),
            shutdown: shutdown.clone(),
        };
        let http_server =
            AppsHttpServer::start(&servers, resource_server, access_guard, shutdown).await?;
        let mut effective_mcp_servers = HashMap::with_capacity(servers.len());
        for server in &servers {
            let effective = effective_loopback_mcp_server(
                &http_server,
                server.server_name(),
                /*enabled_tools*/ None,
            )?
            .with_runtime_metadata(with_upstream_telemetry_origin(
                McpServerRuntimeMetadata::default()
                    .without_physical_tools_list_metric()
                    .with_tools(server.runtime_tool_metadata())
                    .with_trusted_tool_input()
                    .with_trusted_approval_context()
                    .with_primary_turn_sandbox_state(),
                &upstream,
            ));
            effective_mcp_servers.insert(server.server_name().to_string(), effective);
        }
        let resource_mcp_server = effective_loopback_mcp_server(
            &http_server,
            CODEX_APPS_RESOURCE_MCP_SERVER_NAME,
            Some(Vec::new()),
        )?
        .with_runtime_metadata(with_upstream_telemetry_origin(
            McpServerRuntimeMetadata::default().without_physical_tools_list_metric(),
            &upstream,
        ));
        Ok(Self {
            inventory_provenance,
            inventory: CodexAppsInventory {
                apps,
                all_connectors,
                tool_metadata,
            },
            effective_mcp_servers,
            resource_mcp_server,
            http_server,
        })
    }

    pub(super) async fn shutdown(&self) {
        self.http_server.shutdown().await;
    }
}

fn with_upstream_telemetry_origin(
    metadata: McpServerRuntimeMetadata,
    upstream: &AppsUpstream,
) -> McpServerRuntimeMetadata {
    metadata.with_telemetry_origin(upstream.telemetry_url())
}

fn effective_loopback_mcp_server(
    http_server: &AppsHttpServer,
    server_name: &str,
    enabled_tools: Option<Vec<String>>,
) -> Result<EffectiveMcpServer> {
    let config = McpServerConfig {
        transport: McpServerTransportConfig::StreamableHttp {
            url: http_server.url(server_name),
            bearer_token_env_var: None,
            http_headers: None,
            env_http_headers: None,
        },
        auth: McpServerAuth::default(),
        environment_id: DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
        enabled: true,
        required: false,
        supports_parallel_tool_calls: false,
        disabled_reason: None,
        startup_timeout_sec: None,
        tool_timeout_sec: None,
        default_tools_approval_mode: None,
        enabled_tools,
        disabled_tools: None,
        scopes: None,
        oauth: None,
        oauth_resource: None,
        tools: HashMap::new(),
    };
    let bearer_token = http_server
        .bearer_token(server_name)
        .with_context(|| format!("missing runtime bearer token for MCP server `{server_name}`"))?
        .to_string();
    EffectiveMcpServer::configured_with_runtime_bearer_token(config, bearer_token)
        .context("failed to configure Codex Apps loopback MCP authentication")
}

struct ListedAppTool {
    tool: Tool,
    connector_id: Option<String>,
    connector_name: Option<String>,
    connector_description: Option<String>,
}

pub(super) struct ConnectorServerBuilder {
    pub(super) connector_name: String,
    pub(super) connector_description: Option<String>,
    pub(super) tools: Vec<Tool>,
    pub(super) has_non_synthetic_tool: bool,
}

pub(super) fn app_meta_string(meta: Option<&Meta>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        meta.and_then(|meta| meta.get(*key))
            .and_then(JsonValue::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn is_synthetic_link(tool: &Tool) -> bool {
    tool.meta
        .as_deref()
        .and_then(|meta| meta.get("_codex_apps"))
        .and_then(serde_json::Value::as_object)
        .and_then(|meta| meta.get("synthetic_link"))
        .and_then(serde_json::Value::as_bool)
        == Some(true)
}

#[cfg(test)]
#[path = "generation_tests.rs"]
mod tests;
