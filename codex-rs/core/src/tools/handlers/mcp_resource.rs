use codex_protocol::models::FunctionCallOutputBody;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use async_trait::async_trait;
use codex_protocol::mcp::CallToolResult;
use rmcp::model::ListResourceTemplatesResult;
use rmcp::model::ListResourcesResult;
use rmcp::model::PaginatedRequestParam;
use rmcp::model::ReadResourceRequestParam;
use rmcp::model::ReadResourceResult;
use rmcp::model::Resource;
use rmcp::model::ResourceTemplate;
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::function_tool::FunctionCallError;
use crate::protocol::EventMsg;
use crate::protocol::McpInvocation;
use crate::protocol::McpToolCallBeginEvent;
use crate::protocol::McpToolCallEndEvent;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct McpResourceHandler;

#[derive(Debug, Deserialize, Default)]
struct ListResourcesArgs {
    /// Lists all resources from all servers if not specified.
    #[serde(default)]
    server: Option<String>,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ListResourceTemplatesArgs {
    /// Lists all resource templates from all servers if not specified.
    #[serde(default)]
    server: Option<String>,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReadResourceArgs {
    server: String,
    uri: String,
}

#[derive(Debug, Serialize)]
struct ResourceWithServer {
    server: String,
    #[serde(flatten)]
    resource: Resource,
}

impl ResourceWithServer {
    fn new(server: String, resource: Resource) -> Self {
        Self { server, resource }
    }
}

#[derive(Debug, Serialize)]
struct ResourceTemplateWithServer {
    server: String,
    #[serde(flatten)]
    template: ResourceTemplate,
}

impl ResourceTemplateWithServer {
    fn new(server: String, template: ResourceTemplate) -> Self {
        Self { server, template }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ListResourcesPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    server: Option<String>,
    resources: Vec<ResourceWithServer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_cursor: Option<String>,
}

impl ListResourcesPayload {
    fn from_single_server(server: String, result: ListResourcesResult) -> Self {
        let resources = result
            .resources
            .into_iter()
            .map(|resource| ResourceWithServer::new(server.clone(), resource))
            .collect();
        Self {
            server: Some(server),
            resources,
            next_cursor: result.next_cursor,
        }
    }

    fn from_all_servers(resources_by_server: HashMap<String, Vec<Resource>>) -> Self {
        let mut entries: Vec<(String, Vec<Resource>)> = resources_by_server.into_iter().collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));

        let mut resources = Vec::new();
        for (server, server_resources) in entries {
            for resource in server_resources {
                resources.push(ResourceWithServer::new(server.clone(), resource));
            }
        }

        Self {
            server: None,
            resources,
            next_cursor: None,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ListResourceTemplatesPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    server: Option<String>,
    resource_templates: Vec<ResourceTemplateWithServer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_cursor: Option<String>,
}

impl ListResourceTemplatesPayload {
    fn from_single_server(server: String, result: ListResourceTemplatesResult) -> Self {
        let resource_templates = result
            .resource_templates
            .into_iter()
            .map(|template| ResourceTemplateWithServer::new(server.clone(), template))
            .collect();
        Self {
            server: Some(server),
            resource_templates,
            next_cursor: result.next_cursor,
        }
    }

    fn from_all_servers(templates_by_server: HashMap<String, Vec<ResourceTemplate>>) -> Self {
        let mut entries: Vec<(String, Vec<ResourceTemplate>)> =
            templates_by_server.into_iter().collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));

        let mut resource_templates = Vec::new();
        for (server, server_templates) in entries {
            for template in server_templates {
                resource_templates.push(ResourceTemplateWithServer::new(server.clone(), template));
            }
        }

        Self {
            server: None,
            resource_templates,
            next_cursor: None,
        }
    }
}

#[derive(Debug, Serialize)]
struct ReadResourcePayload {
    server: String,
    uri: String,
    #[serde(flatten)]
    result: ReadResourceResult,
}

#[async_trait]
impl ToolHandler for McpResourceHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id,
            tool_name,
            payload,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "mcp_resource handler received unsupported payload".to_string(),
                ));
            }
        };

        let arguments_value = parse_arguments(arguments.as_str())?;

        match tool_name.as_str() {
            "list_mcp_resources" => {
                handle_list_resources(
                    Arc::clone(&session),
                    Arc::clone(&turn),
                    call_id.clone(),
                    arguments_value.clone(),
                )
                .await
            }
            "list_mcp_resource_templates" => {
                handle_list_resource_templates(
                    Arc::clone(&session),
                    Arc::clone(&turn),
                    call_id.clone(),
                    arguments_value.clone(),
                )
                .await
            }
            "read_mcp_resource" => {
                handle_read_resource(
                    Arc::clone(&session),
                    Arc::clone(&turn),
                    call_id,
                    arguments_value,
                )
                .await
            }
            other => Err(FunctionCallError::RespondToModel(format!(
                "unsupported MCP resource tool: {other}"
            ))),
        }
    }
}

async fn handle_list_resources(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    call_id: String,
    arguments: Option<Value>,
) -> Result<ToolOutput, FunctionCallError> {
    let args: ListResourcesArgs = parse_args_with_default(arguments.clone())?;
    let ListResourcesArgs { server, cursor } = args;
    let server = normalize_optional_string(server);
    let cursor = normalize_optional_string(cursor);

    let invocation = McpInvocation {
        server: server.clone().unwrap_or_else(|| "codex".to_string()),
        tool: "list_mcp_resources".to_string(),
        arguments: arguments.clone(),
    };

    emit_tool_call_begin(&session, turn.as_ref(), &call_id, invocation.clone()).await;
    let start = Instant::now();

    let payload_result: Result<ListResourcesPayload, FunctionCallError> = async {
        if let Some(server_name) = server.clone() {
            let params = cursor.clone().map(|value| PaginatedRequestParam {
                cursor: Some(value),
            });
            let result = session
                .list_resources(&server_name, params)
                .await
                .map_err(|err| {
                    FunctionCallError::RespondToModel(format!("resources/list failed: {err:#}"))
                })?;
            Ok(ListResourcesPayload::from_single_server(
                server_name,
                result,
            ))
        } else {
            if cursor.is_some() {
                return Err(FunctionCallError::RespondToModel(
                    "cursor can only be used when a server is specified".to_string(),
                ));
            }

            let resources = session
                .services
                .mcp_connection_manager
                .read()
                .await
                .list_all_resources()
                .await;
            Ok(ListResourcesPayload::from_all_servers(resources))
        }
    }
    .await;

    match payload_result {
        Ok(payload) => match serialize_function_output(payload) {
            Ok(output) => {
                let ToolOutput::Function { body, success } = &output else {
                    unreachable!("MCP resource handler should return function output");
                };
                let content = body.to_text().unwrap_or_default();
                let duration = start.elapsed();
                emit_tool_call_end(
                    &session,
                    turn.as_ref(),
                    &call_id,
                    invocation,
                    duration,
                    Ok(call_tool_result_from_content(&content, *success)),
                )
                .await;
                Ok(output)
            }
            Err(err) => {
                let duration = start.elapsed();
                let message = err.to_string();
                emit_tool_call_end(
                    &session,
                    turn.as_ref(),
                    &call_id,
                    invocation,
                    duration,
                    Err(message.clone()),
                )
                .await;
                Err(err)
            }
        },
        Err(err) => {
            let duration = start.elapsed();
            let message = err.to_string();
            emit_tool_call_end(
                &session,
                turn.as_ref(),
                &call_id,
                invocation,
                duration,
                Err(message.clone()),
            )
            .await;
            Err(err)
        }
    }
}

async fn handle_list_resource_templates(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    call_id: String,
    arguments: Option<Value>,
) -> Result<ToolOutput, FunctionCallError> {
    let args: ListResourceTemplatesArgs = parse_args_with_default(arguments.clone())?;
    let ListResourceTemplatesArgs { server, cursor } = args;
    let server = normalize_optional_string(server);
    let cursor = normalize_optional_string(cursor);

    let invocation = McpInvocation {
        server: server.clone().unwrap_or_else(|| "codex".to_string()),
        tool: "list_mcp_resource_templates".to_string(),
        arguments: arguments.clone(),
    };

    emit_tool_call_begin(&session, turn.as_ref(), &call_id, invocation.clone()).await;
    let start = Instant::now();

    let payload_result: Result<ListResourceTemplatesPayload, FunctionCallError> = async {
        if let Some(server_name) = server.clone() {
            let params = cursor.clone().map(|value| PaginatedRequestParam {
                cursor: Some(value),
            });
            let result = session
                .list_resource_templates(&server_name, params)
                .await
                .map_err(|err| {
                    FunctionCallError::RespondToModel(format!(
                        "resources/templates/list failed: {err:#}"
                    ))
                })?;
            Ok(ListResourceTemplatesPayload::from_single_server(
                server_name,
                result,
            ))
        } else {
            if cursor.is_some() {
                return Err(FunctionCallError::RespondToModel(
                    "cursor can only be used when a server is specified".to_string(),
                ));
            }

            let templates = session
                .services
                .mcp_connection_manager
                .read()
                .await
                .list_all_resource_templates()
                .await;
            Ok(ListResourceTemplatesPayload::from_all_servers(templates))
        }
    }
    .await;

    match payload_result {
        Ok(payload) => match serialize_function_output(payload) {
            Ok(output) => {
                let ToolOutput::Function { body, success } = &output else {
                    unreachable!("MCP resource handler should return function output");
                };
                let content = body.to_text().unwrap_or_default();
                let duration = start.elapsed();
                emit_tool_call_end(
                    &session,
                    turn.as_ref(),
                    &call_id,
                    invocation,
                    duration,
                    Ok(call_tool_result_from_content(&content, *success)),
                )
                .await;
                Ok(output)
            }
            Err(err) => {
                let duration = start.elapsed();
                let message = err.to_string();
                emit_tool_call_end(
                    &session,
                    turn.as_ref(),
                    &call_id,
                    invocation,
                    duration,
                    Err(message.clone()),
                )
                .await;
                Err(err)
            }
        },
        Err(err) => {
            let duration = start.elapsed();
            let message = err.to_string();
            emit_tool_call_end(
                &session,
                turn.as_ref(),
                &call_id,
                invocation,
                duration,
                Err(message.clone()),
            )
            .await;
            Err(err)
        }
    }
}

async fn handle_read_resource(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    call_id: String,
    arguments: Option<Value>,
) -> Result<ToolOutput, FunctionCallError> {
    let args: ReadResourceArgs = parse_args(arguments.clone())?;
    let ReadResourceArgs { server, uri } = args;
    let server = normalize_required_string("server", server)?;
    let uri = normalize_required_string("uri", uri)?;

    let invocation = McpInvocation {
        server: server.clone(),
        tool: "read_mcp_resource".to_string(),
        arguments: arguments.clone(),
    };

    emit_tool_call_begin(&session, turn.as_ref(), &call_id, invocation.clone()).await;
    let start = Instant::now();

    let payload_result: Result<ReadResourcePayload, FunctionCallError> = async {
        let result = session
            .read_resource(&server, ReadResourceRequestParam { uri: uri.clone() })
            .await
            .map_err(|err| {
                FunctionCallError::RespondToModel(format!("resources/read failed: {err:#}"))
            })?;

        Ok(ReadResourcePayload {
            server,
            uri,
            result,
        })
    }
    .await;

    match payload_result {
        Ok(payload) => match serialize_function_output(payload) {
            Ok(output) => {
                let ToolOutput::Function { body, success } = &output else {
                    unreachable!("MCP resource handler should return function output");
                };
                let content = body.to_text().unwrap_or_default();
                let duration = start.elapsed();
                emit_tool_call_end(
                    &session,
                    turn.as_ref(),
                    &call_id,
                    invocation,
                    duration,
                    Ok(call_tool_result_from_content(&content, *success)),
                )
                .await;
                Ok(output)
            }
            Err(err) => {
                let duration = start.elapsed();
                let message = err.to_string();
                emit_tool_call_end(
                    &session,
                    turn.as_ref(),
                    &call_id,
                    invocation,
                    duration,
                    Err(message.clone()),
                )
                .await;
                Err(err)
            }
        },
        Err(err) => {
            let duration = start.elapsed();
            let message = err.to_string();
            emit_tool_call_end(
                &session,
                turn.as_ref(),
                &call_id,
                invocation,
                duration,
                Err(message.clone()),
            )
            .await;
            Err(err)
        }
    }
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
    session
        .send_event(
            turn,
            EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
                call_id: call_id.to_string(),
                invocation,
            }),
        )
        .await;
}

async fn emit_tool_call_end(
    session: &Arc<Session>,
    turn: &TurnContext,
    call_id: &str,
    invocation: McpInvocation,
    duration: Duration,
    result: Result<CallToolResult, String>,
) {
    session
        .send_event(
            turn,
            EventMsg::McpToolCallEnd(McpToolCallEndEvent {
                call_id: call_id.to_string(),
                invocation,
                duration,
                result,
            }),
        )
        .await;
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

fn serialize_function_output<T>(payload: T) -> Result<ToolOutput, FunctionCallError>
where
    T: Serialize,
{
    let content = serde_json::to_string(&payload).map_err(|err| {
        FunctionCallError::RespondToModel(format!(
            "failed to serialize MCP resource response: {err}"
        ))
    })?;

    Ok(ToolOutput::Function {
        body: FunctionCallOutputBody::Text(content),
        success: Some(true),
    })
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
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use rmcp::model::AnnotateAble;
    use serde_json::json;

    fn resource(uri: &str, name: &str) -> Resource {
        rmcp::model::RawResource {
            uri: uri.to_string(),
            name: name.to_string(),
            title: None,
            description: None,
            mime_type: None,
            size: None,
            icons: None,
            meta: None,
        }
        .no_annotation()
    }

    fn template(uri_template: &str, name: &str) -> ResourceTemplate {
        rmcp::model::RawResourceTemplate {
            uri_template: uri_template.to_string(),
            name: name.to_string(),
            title: None,
            description: None,
            mime_type: None,
        }
        .no_annotation()
    }

    #[test]
    fn resource_with_server_serializes_server_field() {
        let entry = ResourceWithServer::new("test".to_string(), resource("memo://id", "memo"));
        let value = serde_json::to_value(&entry).expect("serialize resource");

        assert_eq!(value["server"], json!("test"));
        assert_eq!(value["uri"], json!("memo://id"));
        assert_eq!(value["name"], json!("memo"));
    }

    #[test]
    fn list_resources_payload_from_single_server_copies_next_cursor() {
        let result = ListResourcesResult {
            meta: None,
            next_cursor: Some("cursor-1".to_string()),
            resources: vec![resource("memo://id", "memo")],
        };
        let payload = ListResourcesPayload::from_single_server("srv".to_string(), result);
        let value = serde_json::to_value(&payload).expect("serialize payload");

        assert_eq!(value["server"], json!("srv"));
        assert_eq!(value["nextCursor"], json!("cursor-1"));
        let resources = value["resources"].as_array().expect("resources array");
        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0]["server"], json!("srv"));
    }

    #[test]
    fn list_resources_payload_from_all_servers_is_sorted() {
        let mut map = HashMap::new();
        map.insert("beta".to_string(), vec![resource("memo://b-1", "b-1")]);
        map.insert(
            "alpha".to_string(),
            vec![resource("memo://a-1", "a-1"), resource("memo://a-2", "a-2")],
        );

        let payload = ListResourcesPayload::from_all_servers(map);
        let value = serde_json::to_value(&payload).expect("serialize payload");
        let uris: Vec<String> = value["resources"]
            .as_array()
            .expect("resources array")
            .iter()
            .map(|entry| entry["uri"].as_str().unwrap().to_string())
            .collect();

        assert_eq!(
            uris,
            vec![
                "memo://a-1".to_string(),
                "memo://a-2".to_string(),
                "memo://b-1".to_string()
            ]
        );
    }

    #[test]
    fn call_tool_result_from_content_marks_success() {
        let result = call_tool_result_from_content("{}", Some(true));
        assert_eq!(result.is_error, Some(false));
        assert_eq!(result.content.len(), 1);
    }

    #[test]
    fn parse_arguments_handles_empty_and_json() {
        assert!(
            parse_arguments(" \n\t").unwrap().is_none(),
            "expected None for empty arguments"
        );

        assert!(
            parse_arguments("null").unwrap().is_none(),
            "expected None for null arguments"
        );

        let value = parse_arguments(r#"{"server":"figma"}"#)
            .expect("parse json")
            .expect("value present");
        assert_eq!(value["server"], json!("figma"));
    }

    #[test]
    fn template_with_server_serializes_server_field() {
        let entry =
            ResourceTemplateWithServer::new("srv".to_string(), template("memo://{id}", "memo"));
        let value = serde_json::to_value(&entry).expect("serialize template");

        assert_eq!(
            value,
            json!({
                "server": "srv",
                "uriTemplate": "memo://{id}",
                "name": "memo"
            })
        );
    }
}
