use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Weak;

use codex_connectors::metadata::CODEX_APPS_MCP_SERVER_NAME;
use codex_connectors::metadata::connector_install_url;
use codex_connectors::metadata::connector_tool_name;
use codex_connectors::metadata::connector_tool_title;
use codex_mcp::MCP_SANDBOX_STATE_META_CAPABILITY;
use codex_mcp::MCP_TOOL_INPUT_META_CAPABILITY;
use codex_mcp::McpToolApprovalIdentity;
use codex_mcp::McpToolApprovalParameterLabel;
use codex_mcp::McpToolApprovalPresentation;
use codex_mcp::McpToolRuntimeMetadata;
use codex_mcp::McpToolTelemetryIdentity;
use codex_mcp::SandboxState;
use codex_protocol::mcp::MCP_APPROVAL_CONTEXT_CONNECTED_ACCOUNT_EMAIL_KEY;
use codex_protocol::mcp::MCP_APPROVAL_CONTEXT_META_KEY;
use codex_protocol::mcp::MCP_TOOL_CALL_ID_META_KEY;
use codex_protocol::mcp::MCP_TOOL_INPUT_META_KEY;
use codex_protocol::mcp_approval_meta::CONNECTOR_DESCRIPTION_KEY;
use codex_protocol::mcp_approval_meta::CONNECTOR_ID_KEY;
use codex_protocol::mcp_approval_meta::CONNECTOR_NAME_KEY;
use codex_protocol::mcp_approval_meta::McpToolSource;
use codex_protocol::mcp_approval_meta::SOURCE_CONNECTOR;
use codex_protocol::mcp_approval_meta::SOURCE_KEY;
use rmcp::ServerHandler;
use rmcp::model::CallToolRequestParams;
use rmcp::model::CallToolResult;
use rmcp::model::CreateElicitationRequestParams;
use rmcp::model::ElicitationAction;
use rmcp::model::Implementation;
use rmcp::model::JsonObject;
use rmcp::model::ListResourceTemplatesResult;
use rmcp::model::ListResourcesResult;
use rmcp::model::ListToolsResult;
use rmcp::model::Meta;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ReadResourceRequestParams;
use rmcp::model::ReadResourceResult;
use rmcp::model::ServerCapabilities;
use rmcp::model::ServerInfo;
use rmcp::model::Tool;
use rmcp::service::RequestContext;
use rmcp::service::RoleServer;
use serde_json::Value as JsonValue;
use tokio_util::sync::CancellationToken;

use crate::AppsRefreshCoordinator;
use crate::AppsUpstream;
use crate::CodexAppsAccessGuard;
use crate::approval_presentation::render_approval_presentation;
use crate::auth_elicitation;
use crate::auth_elicitation::MCP_TOOL_CODEX_APPS_META_KEY;
use crate::elicitation_bridge::supports_url_elicitation;
use crate::file_upload::AppsFileSupport;
use crate::file_upload::declared_openai_file_input_param_names;
use crate::file_upload::rewrite_arguments_for_openai_files;
use crate::file_upload::rewrite_tool_schema_for_local_file_paths;
use crate::generation::CodexApp;
use crate::generation::CodexAppToolMetadata;
use crate::generation::ConnectorServerBuilder;
use crate::generation::app_meta_string;
use crate::names::allocate_deterministic_names;
use crate::resource_server::proxy_cancelled;
use crate::resource_server::proxy_error;
use crate::resource_server::proxy_read_resource;
use crate::resource_server::proxy_shutdown;

const META_LINK_ID: &str = "link_id";
const META_OPENAI_OUTPUT_TEMPLATE: &str = "openai/outputTemplate";
const META_RESOURCE_URI: &str = "resource_uri";
const META_TEMPLATE_ID: &str = "template_id";
const META_UI_RESOURCE_URI: &str = "ui/resourceUri";
pub(super) const META_CONNECTED_ACCOUNT_EMAIL: &str = "connected_account_email";
const APPROVAL_HEADER: &str = "Approve app tool call?";

/// One connector's loopback HTTP MCP registration.
pub(super) struct CodexAppServer {
    server_name: String,
    pub(super) service: ConnectorMcpServer,
}

pub(super) struct ConnectorServerContext {
    pub(super) upstream: Arc<AppsUpstream>,
    pub(super) file_support: Option<Arc<AppsFileSupport>>,
    pub(super) refresh_coordinator: Weak<AppsRefreshCoordinator>,
    pub(super) access_guard: CodexAppsAccessGuard,
    pub(super) shutdown: CancellationToken,
}

impl CodexAppServer {
    pub(super) fn new(
        connector_id: String,
        builder: ConnectorServerBuilder,
        server_name: String,
        raw_namespace_identity: String,
        context: ConnectorServerContext,
    ) -> Self {
        let mut candidates = Vec::with_capacity(builder.tools.len());
        let mut seen_raw_identities = HashSet::new();
        for mut tool in builder.tools {
            let upstream_name = tool.name.to_string();
            move_connected_account_to_approval_context(&mut tool);
            let base_callable = connector_tool_name(
                &upstream_name,
                Some(&connector_id),
                Some(&builder.connector_name),
            );
            let raw_tool_identity =
                format!("{raw_namespace_identity}\0{base_callable}\0{upstream_name}");
            if seen_raw_identities.insert(raw_tool_identity.clone()) {
                candidates.push(ToolCandidate {
                    tool,
                    upstream_name,
                    base_callable,
                    raw_tool_identity,
                });
            }
        }
        let exposed_names = allocate_deterministic_names(candidates.iter().map(|candidate| {
            (
                candidate.base_callable.as_str(),
                candidate.raw_tool_identity.as_str(),
            )
        }));

        let mut tools = Vec::with_capacity(candidates.len());
        let mut upstream_names = HashMap::with_capacity(candidates.len());
        let mut file_input_params = HashMap::with_capacity(candidates.len());
        for (mut candidate, exposed_name) in candidates.into_iter().zip(exposed_names) {
            candidate.tool.name = Cow::Owned(exposed_name.clone());
            if let Some(title) = candidate.tool.title.take() {
                candidate.tool.title =
                    Some(connector_tool_title(Some(&builder.connector_name), &title));
            }
            let declared_file_params = declared_openai_file_input_param_names(&candidate.tool);
            if context.file_support.is_some() && !declared_file_params.is_empty() {
                rewrite_tool_schema_for_local_file_paths(
                    &mut candidate.tool,
                    &declared_file_params,
                );
                file_input_params.insert(exposed_name.clone(), declared_file_params);
            }
            upstream_names.insert(exposed_name, candidate.upstream_name);
            tools.push(candidate.tool);
        }
        let resource_uris = tools
            .iter()
            .flat_map(|tool| mcp_app_resource_uris(tool.meta.as_ref()))
            .map(str::to_string)
            .collect();

        let state = ConnectorMcpState {
            connector_id,
            server_name: server_name.clone(),
            connector_name: builder.connector_name,
            connector_description: builder.connector_description,
            include_in_app_inventory: builder.has_non_synthetic_tool,
            tools: Arc::from(tools),
            upstream_names: Arc::new(upstream_names),
            file_input_params: Arc::new(file_input_params),
            resource_uris,
        };
        let service = ConnectorMcpServer {
            state: Arc::new(state),
            upstream: context.upstream,
            file_support: context.file_support,
            refresh_coordinator: context.refresh_coordinator,
            access_guard: context.access_guard,
            shutdown: context.shutdown,
        };
        Self {
            server_name,
            service,
        }
    }

    pub(super) fn inventory_connector(&self) -> CodexApp {
        let state = &self.service.state;
        CodexApp {
            id: state.connector_id.clone(),
            name: state.connector_name.clone(),
            description: state.connector_description.clone(),
            mcp_server_name: state.server_name.clone(),
        }
    }

    pub(super) fn include_in_app_inventory(&self) -> bool {
        self.service.state.include_in_app_inventory
    }

    pub(super) fn tool_metadata(
        &self,
    ) -> impl Iterator<Item = (String, CodexAppToolMetadata)> + '_ {
        let state = &self.service.state;
        state.tools.iter().filter_map(|tool| {
            let exposed_name = tool.name.as_ref();
            let upstream_name = state.upstream_names.get(exposed_name)?;
            Some((
                exposed_name.to_string(),
                CodexAppToolMetadata {
                    connector_id: state.connector_id.clone(),
                    connector_name: state.connector_name.clone(),
                    connector_description: state.connector_description.clone(),
                    upstream_tool_name: upstream_name.clone(),
                    tool_title: tool.title.clone(),
                    destructive_hint: tool
                        .annotations
                        .as_ref()
                        .and_then(|annotations| annotations.destructive_hint),
                    open_world_hint: tool
                        .annotations
                        .as_ref()
                        .and_then(|annotations| annotations.open_world_hint),
                    link_id: app_meta_string(tool.meta.as_ref(), &[META_LINK_ID]),
                    mcp_app_resource_uri: mcp_app_resource_uri(tool.meta.as_ref()),
                    template_id: private_app_meta_string(tool.meta.as_ref(), META_TEMPLATE_ID),
                    action_name: private_app_meta_string(tool.meta.as_ref(), META_RESOURCE_URI)
                        .and_then(|resource_uri| {
                            resource_uri
                                .trim_matches('/')
                                .rsplit('/')
                                .next()
                                .filter(|action_name| !action_name.is_empty())
                                .map(str::to_string)
                        }),
                },
            ))
        })
    }

    pub(super) fn runtime_tool_metadata(&self) -> HashMap<String, McpToolRuntimeMetadata> {
        let state = &self.service.state;
        let approval_form_metadata = connector_approval_form_metadata(state);
        state
            .tools
            .iter()
            .filter_map(|tool| {
                let tool_name = tool.name.to_string();
                let upstream_tool_name = state.upstream_names.get(tool_name.as_str())?;
                let mut metadata = McpToolRuntimeMetadata::default()
                    .with_approval_header(APPROVAL_HEADER)
                    .with_approval_form_metadata(approval_form_metadata.clone())
                    .with_metric_labels([
                        ("connector_id", state.connector_id.clone()),
                        ("connector_name", state.connector_name.clone()),
                    ])
                    .with_search_aliases([upstream_tool_name.clone()]);
                let identity = McpToolApprovalIdentity::new(
                    /*server_name*/ CODEX_APPS_MCP_SERVER_NAME,
                    /*source_id*/ state.connector_id.clone(),
                    /*tool_name*/ upstream_tool_name.clone(),
                )?;
                metadata = metadata.with_approval_identity(identity);
                if let Some(identity) = McpToolTelemetryIdentity::new(
                    CODEX_APPS_MCP_SERVER_NAME,
                    upstream_tool_name.clone(),
                ) {
                    metadata = metadata.with_telemetry_identity(identity);
                }
                if let Some(source) = McpToolSource::new(
                    state.connector_id.clone(),
                    state.connector_name.clone(),
                    state.connector_description.clone(),
                ) {
                    metadata = metadata.with_approval_source(source);
                }
                if let Some(presentation) = render_approval_presentation(
                    &state.connector_id,
                    Some(&state.connector_name),
                    tool.title.as_deref(),
                ) {
                    let parameter_labels = presentation
                        .parameter_labels
                        .into_iter()
                        .filter_map(|parameter| {
                            McpToolApprovalParameterLabel::new(parameter.name, parameter.label)
                        })
                        .collect();
                    if let Some(presentation) =
                        McpToolApprovalPresentation::new(presentation.question, parameter_labels)
                    {
                        metadata = metadata.with_approval_presentation(presentation);
                    }
                }
                Some((tool_name, metadata))
            })
            .collect()
    }

    /// Model-visible logical server name used for routing and registration.
    pub(super) fn server_name(&self) -> &str {
        &self.server_name
    }
}

fn connector_approval_form_metadata(
    state: &ConnectorMcpState,
) -> serde_json::Map<String, JsonValue> {
    let mut metadata = serde_json::Map::from_iter([
        (
            SOURCE_KEY.to_string(),
            JsonValue::String(SOURCE_CONNECTOR.to_string()),
        ),
        (
            CONNECTOR_ID_KEY.to_string(),
            JsonValue::String(state.connector_id.clone()),
        ),
        (
            CONNECTOR_NAME_KEY.to_string(),
            JsonValue::String(state.connector_name.clone()),
        ),
    ]);
    if let Some(description) = state.connector_description.as_ref() {
        metadata.insert(
            CONNECTOR_DESCRIPTION_KEY.to_string(),
            JsonValue::String(description.clone()),
        );
    }
    metadata
}

fn mcp_app_resource_uri(meta: Option<&Meta>) -> Option<String> {
    mcp_app_resource_uris(meta).next().map(str::to_string)
}

fn private_app_meta_string(meta: Option<&Meta>, key: &str) -> Option<String> {
    meta.and_then(|meta| meta.get(MCP_TOOL_CODEX_APPS_META_KEY))
        .and_then(JsonValue::as_object)
        .and_then(|private_meta| private_meta.get(key))
        .and_then(JsonValue::as_str)
        .map(str::to_string)
}

fn mcp_app_resource_uris(meta: Option<&Meta>) -> impl Iterator<Item = &str> {
    let nested = meta
        .and_then(|meta| meta.get("ui"))
        .and_then(JsonValue::as_object)
        .and_then(|ui| ui.get("resourceUri"))
        .and_then(JsonValue::as_str);
    let flat = meta
        .and_then(|meta| meta.get(META_UI_RESOURCE_URI))
        .and_then(JsonValue::as_str);
    let output_template = meta
        .and_then(|meta| meta.get(META_OPENAI_OUTPUT_TEMPLATE))
        .and_then(JsonValue::as_str);
    [nested, flat, output_template]
        .into_iter()
        .flatten()
        .map(str::trim)
        .filter(|uri| !uri.is_empty())
}

pub(super) fn move_connected_account_to_approval_context(tool: &mut Tool) {
    let meta = tool.meta.get_or_insert_with(Meta::new);
    let connected_account_email = meta
        .get(MCP_TOOL_CODEX_APPS_META_KEY)
        .and_then(JsonValue::as_object)
        .and_then(|source| source.get(META_CONNECTED_ACCOUNT_EMAIL))
        .and_then(JsonValue::as_str)
        .and_then(normalize_connected_account_email);

    // Generic approval context is trusted only because this proxy owns the registration. Never
    // relay an upstream value at the generic key: derive it from the authenticated Apps envelope.
    meta.remove(MCP_APPROVAL_CONTEXT_META_KEY);
    if let Some(codex_apps_meta) = meta
        .get_mut(MCP_TOOL_CODEX_APPS_META_KEY)
        .and_then(JsonValue::as_object_mut)
    {
        codex_apps_meta.remove(META_CONNECTED_ACCOUNT_EMAIL);
    }
    if let Some(connected_account_email) = connected_account_email {
        meta.insert(
            MCP_APPROVAL_CONTEXT_META_KEY.to_string(),
            JsonValue::Object(serde_json::Map::from_iter([(
                MCP_APPROVAL_CONTEXT_CONNECTED_ACCOUNT_EMAIL_KEY.to_string(),
                JsonValue::String(connected_account_email),
            )])),
        );
    }
}

fn normalize_connected_account_email(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()
        && value.len() <= 320
        && value.contains('@')
        && !value
            .chars()
            .any(|character| character.is_whitespace() || character.is_control()))
    .then(|| value.to_string())
}

struct ToolCandidate {
    tool: Tool,
    upstream_name: String,
    base_callable: String,
    raw_tool_identity: String,
}

#[derive(Clone)]
pub(super) struct ConnectorMcpServer {
    state: Arc<ConnectorMcpState>,
    upstream: Arc<AppsUpstream>,
    file_support: Option<Arc<AppsFileSupport>>,
    refresh_coordinator: Weak<AppsRefreshCoordinator>,
    access_guard: CodexAppsAccessGuard,
    shutdown: CancellationToken,
}

struct ConnectorMcpState {
    connector_id: String,
    server_name: String,
    connector_name: String,
    connector_description: Option<String>,
    include_in_app_inventory: bool,
    tools: Arc<[Tool]>,
    upstream_names: Arc<HashMap<String, String>>,
    file_input_params: Arc<HashMap<String, Vec<String>>>,
    resource_uris: HashSet<String>,
}

impl ServerHandler for ConnectorMcpServer {
    fn get_info(&self) -> ServerInfo {
        let state = &self.state;
        let implementation =
            Implementation::new(state.server_name.clone(), env!("CARGO_PKG_VERSION"))
                .with_title(state.connector_name.clone());
        let mut capabilities = ServerCapabilities::builder()
            .enable_tools()
            .enable_resources()
            .build();
        if self.file_support.is_some() {
            capabilities.experimental = Some(BTreeMap::from([
                (
                    MCP_SANDBOX_STATE_META_CAPABILITY.to_string(),
                    JsonObject::new(),
                ),
                (
                    MCP_TOOL_INPUT_META_CAPABILITY.to_string(),
                    JsonObject::new(),
                ),
            ]));
        }
        let mut info = ServerInfo::new(capabilities).with_server_info(implementation);
        info.instructions = Some(
            state
                .connector_description
                .clone()
                .unwrap_or_else(|| format!("Tools for working with {}.", state.connector_name)),
        );
        info
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        self.ensure_access_is_current()?;
        let state = &self.state;
        Ok(ListToolsResult {
            tools: state.tools.to_vec(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, rmcp::ErrorData> {
        self.ensure_access_is_current()?;
        Ok(ListResourcesResult {
            resources: Vec::new(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, rmcp::ErrorData> {
        self.ensure_access_is_current()?;
        Ok(ListResourceTemplatesResult {
            resource_templates: Vec::new(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, rmcp::ErrorData> {
        self.ensure_access_is_current()?;
        if !self.state.resource_uris.contains(request.uri.as_str()) {
            return Err(rmcp::ErrorData::resource_not_found(
                format!(
                    "resource `{}` is not declared by this MCP server",
                    request.uri
                ),
                None,
            ));
        }
        proxy_read_resource(
            &self.upstream,
            &self.access_guard,
            &self.shutdown,
            request,
            context,
        )
        .await
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        self.ensure_access_is_current()?;
        let fallback_call_id = request_id_string(&context.id);
        let cancellation = context.ct.clone();
        let downstream = context.peer.clone();
        let state = Arc::clone(&self.state);
        let Some(upstream_name) = state.upstream_names.get(request.name.as_ref()) else {
            return Err(rmcp::ErrorData::invalid_params(
                format!("unknown tool `{}`", request.name),
                None,
            ));
        };
        let bridge = Arc::clone(&self.upstream.elicitation_bridge);
        let _elicitation_call = tokio::select! {
            call = bridge.begin_call(downstream.clone()) => call.map_err(proxy_error),
            _ = cancellation.cancelled() => return Err(proxy_cancelled("tools/call")),
            _ = self.shutdown.cancelled() => return Err(proxy_shutdown()),
        }?;
        self.ensure_access_is_current()?;
        let upstream = tokio::select! {
            result = self.upstream.client() => result.map_err(proxy_error),
            _ = cancellation.cancelled() => return Err(proxy_cancelled("tools/call")),
            _ = self.shutdown.cancelled() => return Err(proxy_shutdown()),
        }?;
        self.ensure_access_is_current()?;
        let mut meta = context.meta.0;
        if let Some(request_meta) = request.meta {
            meta.extend(request_meta.0);
        }
        let sandbox_state = meta
            .remove(MCP_SANDBOX_STATE_META_CAPABILITY)
            .map(serde_json::from_value::<SandboxState>)
            .transpose()
            .map_err(|error| {
                rmcp::ErrorData::invalid_params(
                    format!("invalid Codex sandbox state: {error}"),
                    None,
                )
            })?;
        let call_id = meta
            .remove(MCP_TOOL_CALL_ID_META_KEY)
            .and_then(|value| value.as_str().map(str::to_string))
            .filter(|value| !value.is_empty())
            .unwrap_or(fallback_call_id);
        let mut apps_meta = state
            .tools
            .iter()
            .find(|tool| tool.name == request.name)
            .and_then(|tool| tool.meta.as_deref())
            .and_then(|meta| meta.get(MCP_TOOL_CODEX_APPS_META_KEY))
            .and_then(JsonValue::as_object)
            .cloned()
            .unwrap_or_default();
        // The private Apps envelope is proxy-owned. Preserve ordinary request metadata, but never
        // forward fields supplied by the downstream MCP client under this key.
        meta.remove(MCP_TOOL_CODEX_APPS_META_KEY);
        apps_meta.insert("call_id".to_string(), JsonValue::String(call_id.clone()));
        meta.insert(
            MCP_TOOL_CODEX_APPS_META_KEY.to_string(),
            JsonValue::Object(apps_meta),
        );
        let original_arguments = request.arguments.map(serde_json::Value::Object);
        let arguments = match (
            self.file_support.as_deref(),
            state.file_input_params.get(request.name.as_ref()),
        ) {
            (Some(file_support), Some(file_params)) if !file_params.is_empty() => {
                tokio::select! {
                    result = rewrite_arguments_for_openai_files(
                        file_support,
                        sandbox_state.as_ref(),
                        original_arguments.clone(),
                        file_params,
                    ) => result.map_err(|error| rmcp::ErrorData::invalid_params(error, None))?,
                    _ = cancellation.cancelled() => return Err(proxy_cancelled("tools/call")),
                    _ = self.shutdown.cancelled() => return Err(proxy_shutdown()),
                }
            }
            _ => original_arguments.clone(),
        };
        let rewritten_tool_input = (arguments != original_arguments)
            .then(|| arguments.clone())
            .flatten();
        self.ensure_access_is_current()?;
        let call = upstream.call_tool(
            upstream_name.clone(),
            arguments,
            (!meta.is_empty()).then_some(serde_json::Value::Object(meta)),
            /*timeout*/ None,
        );
        let mut result = tokio::select! {
            result = call => result.map_err(proxy_error),
            _ = cancellation.cancelled() => Err(rmcp::ErrorData::internal_error(
                "Codex Apps MCP tool call was cancelled",
                None,
            )),
            _ = self.shutdown.cancelled() => Err(rmcp::ErrorData::internal_error(
                "Codex Apps MCP server is shutting down",
                None,
            )),
        }?;
        auth_elicitation::expose_auth_error_code_to_telemetry(&mut result);
        if let Some(meta) = result.meta.as_mut() {
            meta.0.remove(MCP_TOOL_INPUT_META_KEY);
        }
        if let Some(rewritten_tool_input) = rewritten_tool_input {
            result
                .meta
                .get_or_insert_with(Meta::new)
                .0
                .insert(MCP_TOOL_INPUT_META_KEY.to_string(), rewritten_tool_input);
        }
        let install_url = connector_install_url(&state.connector_name, &state.connector_id);
        let Some(plan) = auth_elicitation::build_auth_elicitation_plan_from_rmcp_result(
            &call_id,
            &result,
            Some(&state.connector_id),
            Some(&state.connector_name),
            Some(install_url),
        ) else {
            return Ok(result);
        };
        if !supports_url_elicitation(&downstream) {
            return Ok(result);
        }
        let elicitation_meta = plan.elicitation.meta.as_object().cloned().map(Meta);
        let elicitation =
            downstream.create_elicitation(CreateElicitationRequestParams::UrlElicitationParams {
                meta: elicitation_meta,
                message: plan.elicitation.message,
                url: plan.elicitation.url,
                elicitation_id: plan.elicitation.elicitation_id,
            });
        let response = tokio::select! {
            response = elicitation => response,
            _ = cancellation.cancelled() => return Err(rmcp::ErrorData::internal_error(
                "Codex Apps MCP auth elicitation was cancelled",
                None,
            )),
            _ = self.shutdown.cancelled() => return Err(rmcp::ErrorData::internal_error(
                "Codex Apps MCP server is shutting down",
                None,
            )),
        };
        match response {
            Ok(response) if response.action == ElicitationAction::Accept => {
                if let Some(refresh_coordinator) = self.refresh_coordinator.upgrade()
                    && let Err(error) = refresh_coordinator.refresh().await
                {
                    tracing::warn!(%error, "failed to refresh Codex Apps after authentication");
                }
                Ok(auth_elicitation::rmcp_auth_elicitation_completed_result(
                    &plan.auth_failure,
                    result,
                ))
            }
            Ok(_) => Ok(result),
            Err(error) => {
                tracing::warn!(%error, "Codex Apps auth elicitation was not available");
                Ok(result)
            }
        }
    }
}

impl ConnectorMcpServer {
    pub(super) fn for_http_session(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
            upstream: self.upstream.fork(),
            file_support: self.file_support.clone(),
            refresh_coordinator: self.refresh_coordinator.clone(),
            access_guard: self.access_guard.clone(),
            shutdown: self.shutdown.clone(),
        }
    }

    fn ensure_access_is_current(&self) -> Result<(), rmcp::ErrorData> {
        self.access_guard
            .is_current()
            .then_some(())
            .ok_or_else(access_expired)
    }
}

fn access_expired() -> rmcp::ErrorData {
    rmcp::ErrorData::internal_error("Codex Apps credentials are no longer current", None)
}

fn request_id_string(id: &rmcp::model::RequestId) -> String {
    match id {
        rmcp::model::NumberOrString::String(value) => value.to_string(),
        rmcp::model::NumberOrString::Number(value) => value.to_string(),
    }
}
