use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::McpResourceListing;
use codex_mcp::McpServerExposure;
use codex_mcp::McpServerKey;
use codex_protocol::items::McpToolCallError;
use codex_protocol::items::McpToolCallItem;
use codex_protocol::items::McpToolCallStatus;
use codex_protocol::items::TurnItem;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::models::function_call_output_content_items_to_text;
use codex_protocol::protocol::TruncationPolicy;
use codex_utils_output_truncation::truncate_text;
use codex_utils_output_truncation::with_serialization_allowance;
use rmcp::model::ListResourceTemplatesResult;
use rmcp::model::ListResourcesResult;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ReadResourceResult;
use rmcp::model::Resource;
use rmcp::model::ResourceTemplate;
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::function_tool::FunctionCallError;
use crate::session::session::Session;
use crate::session::step_context::StepContext;
use crate::session::turn_context::TurnContext;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolEnvironments;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use codex_protocol::protocol::McpInvocation;

mod list_mcp_resource_templates;
mod list_mcp_resources;
mod read_mcp_resource;

pub use list_mcp_resource_templates::ListMcpResourceTemplatesHandler;
pub use list_mcp_resources::ListMcpResourcesHandler;
pub use read_mcp_resource::ReadMcpResourceHandler;

fn model_can_access_mcp_server(turn: &TurnContext, server: &str) -> bool {
    turn.config.orchestrator_mcp_enabled || server != CODEX_APPS_MCP_SERVER_NAME
}

fn ensure_model_can_access_mcp_server(
    turn: &TurnContext,
    server: &str,
) -> Result<(), FunctionCallError> {
    if model_can_access_mcp_server(turn, server) {
        Ok(())
    } else {
        Err(FunctionCallError::RespondToModel(format!(
            "MCP server '{server}' is disabled by `orchestrator.mcp.enabled`"
        )))
    }
}

enum ResourceOperation {
    List,
    Read,
}

// Hooks and execution must choose the same server from this step's binding.
fn resolve_resource_server<'a>(
    step: &'a StepContext,
    server: &str,
    environment_id: Option<&str>,
) -> Result<&'a McpServerKey, FunctionCallError> {
    let catalog = &step.mcp.config().mcp_server_catalog;
    let multi_environment = catalog.exposure() == McpServerExposure::EnvironmentQualified;
    let environment_id = environment_id.filter(|_| multi_environment);
    let registration = match environment_id {
        Some(environment_id) => catalog
            .server(&McpServerKey::in_environment(environment_id, server))
            .or_else(|| catalog.server_by_name(server))
            .filter(|registration| registration.config().environment_id == environment_id),
        None => catalog.server_by_name(server),
    }
    .ok_or_else(|| {
        FunctionCallError::RespondToModel(format!(
            "unknown MCP server '{server}' for the selected environment"
        ))
    })?;
    let key = registration.key();
    ensure_model_can_access_mcp_server(&step.turn, key.name())?;
    if catalog.exposure().environment_id(key).is_some()
        && environment_id.is_none()
        && step.environments.environments.len() > 1
    {
        return Err(FunctionCallError::RespondToModel(format!(
            "environment_id is required for MCP server '{server}' when multiple environments are selected"
        )));
    }
    Ok(key)
}

fn resource_environments(
    invocation: &ToolInvocation,
    operation: ResourceOperation,
) -> Result<ToolEnvironments, FunctionCallError> {
    let arguments = resource_arguments(&invocation.payload)?;
    let (server, environment_id) = match operation {
        ResourceOperation::List => {
            let args = ListResourceArgs::parse(arguments)?;
            (args.server, args.environment_id)
        }
        ResourceOperation::Read => {
            let args = ReadResourceArgs::parse(arguments)?;
            (Some(args.server), args.environment_id)
        }
    };
    // Only an aggregate list has no owning server. A named but unknown server is an error.
    let Some(server) = server else {
        return Ok(ToolEnvironments::Mcp {
            server: None,
            owner: None,
        });
    };
    let step = &invocation.step_context;
    let server_key = resolve_resource_server(step, &server, environment_id.as_deref())?;
    let environment_id = step.mcp.server_environment_id(server_key).ok_or_else(|| {
        FunctionCallError::RespondToModel(format!("unknown MCP server '{server}'"))
    })?;
    Ok(ToolEnvironments::Mcp {
        server: Some(server),
        owner: Some(environment_id.to_owned()),
    })
}

fn resource_arguments(payload: &ToolPayload) -> Result<Option<Value>, FunctionCallError> {
    let ToolPayload::Function { arguments } = payload else {
        return Err(FunctionCallError::Fatal(
            "expected function arguments for MCP resource operation".to_string(),
        ));
    };
    parse_arguments(arguments)
}

#[derive(Debug, Deserialize, Default, PartialEq, Eq)]
struct ListResourceArgs {
    #[serde(default)]
    server: Option<String>,
    #[serde(default)]
    cursor: Option<String>,
    environment_id: Option<String>,
}

impl ListResourceArgs {
    fn parse(arguments: Option<Value>) -> Result<Self, FunctionCallError> {
        parse_args_with_default::<Self>(arguments).map(Self::normalized)
    }

    fn normalized(self) -> Self {
        Self {
            server: normalize_optional_string(self.server),
            cursor: normalize_optional_string(self.cursor),
            environment_id: normalize_optional_string(self.environment_id),
        }
    }

    fn target<'a>(
        &self,
        step: &'a StepContext,
    ) -> Result<Option<(&'a McpServerKey, Option<PaginatedRequestParams>)>, FunctionCallError> {
        match &self.server {
            Some(server) => {
                let server_key =
                    resolve_resource_server(step, server, self.environment_id.as_deref())?;
                let params = self
                    .cursor
                    .clone()
                    .map(|cursor| PaginatedRequestParams::default().with_cursor(Some(cursor)));
                Ok(Some((server_key, params)))
            }
            None if self.cursor.is_some() => Err(FunctionCallError::RespondToModel(
                "cursor can only be used when a server is specified".to_string(),
            )),
            None if self.environment_id.is_some()
                && step.mcp.config().mcp_server_catalog.exposure()
                    == McpServerExposure::EnvironmentQualified =>
            {
                Err(FunctionCallError::RespondToModel(
                    "environment_id can only be used when a server is specified".to_string(),
                ))
            }
            None => Ok(None),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ReadResourceArgs {
    server: String,
    uri: String,
    environment_id: Option<String>,
}

impl ReadResourceArgs {
    fn parse(arguments: Option<Value>) -> Result<Self, FunctionCallError> {
        let Self {
            server,
            uri,
            environment_id,
        } = parse_args(arguments)?;
        Ok(Self {
            server: normalize_required_string("server", server)?,
            uri: normalize_required_string("uri", uri)?,
            environment_id: normalize_optional_string(environment_id),
        })
    }
}

#[derive(Debug, Serialize)]
struct ResourceWithServer<T> {
    server: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    environment_id: Option<String>,
    #[serde(flatten)]
    resource: T,
}

impl<T> ResourceWithServer<T> {
    fn new(server: &McpServerKey, resource: T, exposure: McpServerExposure) -> Self {
        Self {
            server: server.name().to_owned(),
            environment_id: exposure.environment_id(server).map(str::to_owned),
            resource,
        }
    }

    fn from_server(
        server: &McpServerKey,
        resources: Vec<T>,
        exposure: McpServerExposure,
    ) -> Vec<Self> {
        resources
            .into_iter()
            .map(|resource| Self::new(server, resource, exposure))
            .collect()
    }

    fn from_all_servers(
        resources_by_server: HashMap<McpServerKey, Vec<T>>,
        exposure: McpServerExposure,
    ) -> Vec<Self> {
        let mut entries: Vec<_> = resources_by_server.into_iter().collect();
        entries.sort_by(|(left, _), (right, _)| left.cmp(right));
        entries
            .into_iter()
            .flat_map(|(server, resources)| Self::from_server(&server, resources, exposure))
            .collect()
    }
}

/// Identifies a server that can be started by a targeted resource request.
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum DeferredResourceServer {
    Unscoped(String),
    Scoped {
        server: String,
        environment_id: String,
    },
}

impl DeferredResourceServer {
    fn from_key(key: &McpServerKey, exposure: McpServerExposure) -> Self {
        match exposure.environment_id(key) {
            Some(environment_id) => Self::Scoped {
                server: key.name().to_owned(),
                environment_id: environment_id.to_owned(),
            },
            None => Self::Unscoped(key.name().to_owned()),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ListResourcesPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    server: Option<String>,
    #[serde(rename = "environment_id", skip_serializing_if = "Option::is_none")]
    environment_id: Option<String>,
    resources: Vec<ResourceWithServer<Resource>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_cursor: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    deferred_servers: Vec<DeferredResourceServer>,
}

impl ListResourcesPayload {
    fn from_single_server(
        server: &McpServerKey,
        result: ListResourcesResult,
        exposure: McpServerExposure,
    ) -> Self {
        Self {
            resources: ResourceWithServer::from_server(server, result.resources, exposure),
            server: Some(server.name().to_owned()),
            environment_id: exposure.environment_id(server).map(str::to_owned),
            next_cursor: result.next_cursor,
            deferred_servers: Vec::new(),
        }
    }

    fn from_all_servers(
        listing: McpResourceListing<Resource>,
        exposure: McpServerExposure,
    ) -> Self {
        Self {
            server: None,
            environment_id: None,
            resources: ResourceWithServer::from_all_servers(listing.by_server, exposure),
            next_cursor: None,
            deferred_servers: listing
                .deferred_servers
                .iter()
                .map(|key| DeferredResourceServer::from_key(key, exposure))
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ListResourceTemplatesPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    server: Option<String>,
    #[serde(rename = "environment_id", skip_serializing_if = "Option::is_none")]
    environment_id: Option<String>,
    resource_templates: Vec<ResourceWithServer<ResourceTemplate>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_cursor: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    deferred_servers: Vec<DeferredResourceServer>,
}

impl ListResourceTemplatesPayload {
    fn from_single_server(
        server: &McpServerKey,
        result: ListResourceTemplatesResult,
        exposure: McpServerExposure,
    ) -> Self {
        Self {
            resource_templates: ResourceWithServer::from_server(
                server,
                result.resource_templates,
                exposure,
            ),
            server: Some(server.name().to_owned()),
            environment_id: exposure.environment_id(server).map(str::to_owned),
            next_cursor: result.next_cursor,
            deferred_servers: Vec::new(),
        }
    }

    fn from_all_servers(
        listing: McpResourceListing<ResourceTemplate>,
        exposure: McpServerExposure,
    ) -> Self {
        Self {
            server: None,
            environment_id: None,
            resource_templates: ResourceWithServer::from_all_servers(listing.by_server, exposure),
            next_cursor: None,
            deferred_servers: listing
                .deferred_servers
                .iter()
                .map(|key| DeferredResourceServer::from_key(key, exposure))
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct ReadResourcePayload {
    server: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    environment_id: Option<String>,
    uri: String,
    #[serde(flatten)]
    result: ReadResourceResult,
}

fn call_tool_result_from_content(content: &str, success: Option<bool>) -> CallToolResult {
    CallToolResult {
        content: vec![serde_json::json!({"type": "text", "text": content})],
        structured_content: None,
        is_error: success.map(|value| !value),
        meta: None,
    }
}

async fn emit_tool_call_begin(
    session: &Arc<Session>,
    turn: &TurnContext,
    call_id: &str,
    invocation: McpInvocation,
) {
    let McpInvocation {
        server,
        environment_id,
        tool,
        arguments,
    } = invocation;
    let item = TurnItem::McpToolCall(McpToolCallItem {
        id: call_id.to_string(),
        server,
        environment_id,
        tool,
        arguments: arguments.unwrap_or(Value::Null),
        connector_id: None,
        mcp_app_resource_uri: None,
        mcp_app_ui: None,
        link_id: None,
        app_name: None,
        action_name: None,
        plugin_id: None,
        read_only_hint: None,
        status: McpToolCallStatus::InProgress,
        result: None,
        error: None,
        duration: None,
    });
    session.emit_turn_item_started(turn, &item).await;
}

async fn emit_tool_call_end(
    session: &Arc<Session>,
    turn: &TurnContext,
    call_id: &str,
    invocation: McpInvocation,
    duration: Duration,
    result: Result<CallToolResult, String>,
) {
    let (status, result, error) = match result {
        Ok(result) if result.is_error.unwrap_or(false) => {
            (McpToolCallStatus::Failed, Some(result), None)
        }
        Ok(result) => (McpToolCallStatus::Completed, Some(result), None),
        Err(message) => (
            McpToolCallStatus::Failed,
            None,
            Some(McpToolCallError { message }),
        ),
    };
    let McpInvocation {
        server,
        environment_id,
        tool,
        arguments,
    } = invocation;
    let item = TurnItem::McpToolCall(McpToolCallItem {
        id: call_id.to_string(),
        server,
        environment_id,
        tool,
        arguments: arguments.unwrap_or(Value::Null),
        connector_id: None,
        mcp_app_resource_uri: None,
        mcp_app_ui: None,
        link_id: None,
        app_name: None,
        action_name: None,
        plugin_id: None,
        read_only_hint: None,
        status,
        result,
        error,
        duration: Some(duration),
    });
    session.emit_turn_item_completed(turn, item).await;
}

async fn run_resource_operation<T>(
    session: &Arc<Session>,
    step_context: &StepContext,
    call_id: &str,
    invocation: McpInvocation,
    operation: impl Future<Output = Result<T, FunctionCallError>>,
) -> Result<Box<dyn ToolOutput>, FunctionCallError>
where
    T: Serialize,
{
    let turn = &step_context.turn;
    emit_tool_call_begin(session, turn, call_id, invocation.clone()).await;
    let start = Instant::now();
    let result = operation.await.and_then(|payload| {
        serialize_function_output(
            payload,
            step_context.settings.model_info.truncation_policy.into(),
        )
    });

    match result {
        Ok(output) => {
            let content =
                function_call_output_content_items_to_text(&output.body).unwrap_or_default();
            emit_tool_call_end(
                session,
                turn,
                call_id,
                invocation,
                start.elapsed(),
                Ok(call_tool_result_from_content(&content, output.success)),
            )
            .await;
            Ok(boxed_tool_output(output))
        }
        Err(error) => {
            emit_tool_call_end(
                session,
                turn,
                call_id,
                invocation,
                start.elapsed(),
                Err(error.to_string()),
            )
            .await;
            Err(error)
        }
    }
}

fn normalize_optional_string(input: Option<String>) -> Option<String> {
    input.and_then(|value| {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn normalize_required_string(field: &str, value: String) -> Result<String, FunctionCallError> {
    match normalize_optional_string(Some(value)) {
        Some(normalized) => Ok(normalized),
        None => Err(FunctionCallError::RespondToModel(format!(
            "{field} must be provided"
        ))),
    }
}

fn serialize_function_output<T>(
    payload: T,
    truncation_policy: TruncationPolicy,
) -> Result<FunctionToolOutput, FunctionCallError>
where
    T: Serialize,
{
    let content = serde_json::to_string(&payload).map_err(|err| {
        FunctionCallError::RespondToModel(format!(
            "failed to serialize MCP resource response: {err}"
        ))
    })?;
    // Match regular MCP tool outputs by bounding the copy persisted to the
    // rollout and injected into model context.
    let content = truncate_text(&content, with_serialization_allowance(truncation_policy));

    Ok(FunctionToolOutput::from_text(content, Some(true)))
}

fn parse_arguments(raw_args: &str) -> Result<Option<Value>, FunctionCallError> {
    if raw_args.trim().is_empty() {
        Ok(None)
    } else {
        let value: Value = serde_json::from_str(raw_args).map_err(|err| {
            FunctionCallError::RespondToModel(format!("failed to parse function arguments: {err}"))
        })?;
        if value.is_null() {
            Ok(None)
        } else {
            Ok(Some(value))
        }
    }
}

fn parse_args<T>(arguments: Option<Value>) -> Result<T, FunctionCallError>
where
    T: DeserializeOwned,
{
    match arguments {
        Some(value) => serde_json::from_value(value).map_err(|err| {
            FunctionCallError::RespondToModel(format!("failed to parse function arguments: {err}"))
        }),
        None => Err(FunctionCallError::RespondToModel(
            "failed to parse function arguments: expected value".to_string(),
        )),
    }
}

fn parse_args_with_default<T>(arguments: Option<Value>) -> Result<T, FunctionCallError>
where
    T: DeserializeOwned + Default,
{
    match arguments {
        Some(value) => parse_args(Some(value)),
        None => Ok(T::default()),
    }
}

#[cfg(test)]
#[path = "mcp_resource_tests.rs"]
mod tests;
