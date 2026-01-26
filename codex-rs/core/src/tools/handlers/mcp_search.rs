use async_trait::async_trait;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::cmp::Ordering;
use std::collections::HashSet;
use url::Url;
use uuid::Uuid;

use super::mcp_resource::handle_list_resource_templates;
use super::mcp_resource::handle_list_resources;
use super::mcp_resource::handle_read_resource;
use crate::content_items_to_text;
use crate::function_tool::FunctionCallError;
use crate::mcp_tool_call::handle_mcp_tool_call;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use crate::tools::spec::sanitize_json_schema;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;

pub struct McpSearchHandler;

const DEFAULT_LIMIT: usize = 10;
const MAX_LIMIT: usize = 50;

fn default_limit() -> usize {
    DEFAULT_LIMIT
}

#[derive(Deserialize)]
struct McpSearchArgs {
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    server: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    include_schema: bool,
    #[serde(default)]
    route: bool,
    #[serde(default)]
    call: Option<McpSearchCall>,
    #[serde(default)]
    resources: Option<McpSearchResources>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
struct McpSearchCall {
    #[serde(default)]
    qualified_name: Option<String>,
    #[serde(default)]
    server: Option<String>,
    #[serde(default)]
    tool: Option<String>,
    #[serde(default)]
    arguments: Option<JsonValue>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
struct McpSearchResources {
    action: String,
    #[serde(default)]
    server: Option<String>,
    #[serde(default)]
    cursor: Option<String>,
    #[serde(default)]
    uri: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "snake_case")]
struct McpToolSchema {
    input: JsonValue,
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<JsonValue>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct McpSearchResult {
    qualified_name: String,
    server: String,
    tool: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    schema: Option<McpToolSchema>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct McpSearchResponse {
    query: String,
    total_matches: usize,
    results: Vec<McpSearchResult>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "snake_case")]
struct McpSearchRouteScore {
    required_filled: usize,
    required_total: usize,
    typed_matches: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "snake_case")]
struct McpSearchRouteCandidate {
    qualified_name: String,
    server: String,
    tool: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    schema: Option<McpToolSchema>,
    score: McpSearchRouteScore,
    #[serde(skip_serializing_if = "Option::is_none")]
    arguments: Option<JsonValue>,
    missing_required: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct McpSearchRouteDecision {
    action: String,
    reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    call: Option<McpSearchCallDescriptor>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct McpSearchCallDescriptor {
    qualified_name: String,
    server: String,
    tool: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    arguments: Option<JsonValue>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct McpSearchRouteResult {
    kind: String,
    value: JsonValue,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct McpSearchRouteResponse {
    query: String,
    total_candidates: usize,
    decision: McpSearchRouteDecision,
    candidates: Vec<McpSearchRouteCandidate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<McpSearchRouteResult>,
}

fn serialize_input_schema(
    mut input_schema: mcp_types::ToolInputSchema,
) -> Result<JsonValue, FunctionCallError> {
    if input_schema.properties.is_none() {
        input_schema.properties = Some(JsonValue::Object(serde_json::Map::new()));
    }
    let mut value = serde_json::to_value(input_schema).map_err(|err| {
        FunctionCallError::RespondToModel(format!("failed to serialize input schema: {err}"))
    })?;
    sanitize_json_schema(&mut value);
    Ok(value)
}

fn serialize_output_schema(
    output_schema: mcp_types::ToolOutputSchema,
) -> Result<JsonValue, FunctionCallError> {
    let mut value = serde_json::to_value(output_schema).map_err(|err| {
        FunctionCallError::RespondToModel(format!("failed to serialize output schema: {err}"))
    })?;
    sanitize_json_schema(&mut value);
    Ok(value)
}

fn resolve_call_target(
    call: &McpSearchCall,
    tools: &std::collections::HashMap<String, crate::mcp_connection_manager::ToolInfo>,
) -> Result<(String, String), FunctionCallError> {
    if let Some(qualified_name) = call
        .qualified_name
        .as_deref()
        .map(str::trim)
        .filter(|val| !val.is_empty())
    {
        let tool_info = tools.get(qualified_name).ok_or_else(|| {
            FunctionCallError::RespondToModel(format!(
                "unknown MCP tool qualified_name: {qualified_name}"
            ))
        })?;
        return Ok((tool_info.server_name.clone(), tool_info.tool_name.clone()));
    }

    let server = call
        .server
        .as_deref()
        .map(str::trim)
        .filter(|val| !val.is_empty())
        .ok_or_else(|| {
            FunctionCallError::RespondToModel(
                "call requires qualified_name or server + tool".to_string(),
            )
        })?;
    let tool = call
        .tool
        .as_deref()
        .map(str::trim)
        .filter(|val| !val.is_empty())
        .ok_or_else(|| {
            FunctionCallError::RespondToModel(
                "call requires qualified_name or server + tool".to_string(),
            )
        })?;

    Ok((server.to_string(), tool.to_string()))
}

#[derive(Clone)]
struct ArgumentInference {
    arguments: Option<JsonValue>,
    missing_required: Vec<String>,
    required_total: usize,
    required_filled: usize,
    typed_matches: usize,
}

impl McpSearchRouteScore {
    fn coverage_cmp(&self, other: &Self) -> Ordering {
        match (self.required_total, other.required_total) {
            (0, 0) => Ordering::Equal,
            (0, _) => Ordering::Less,
            (_, 0) => Ordering::Greater,
            _ => {
                let left = self.required_filled * other.required_total;
                let right = other.required_filled * self.required_total;
                left.cmp(&right)
                    .then_with(|| self.required_filled.cmp(&other.required_filled))
            }
        }
    }

    fn cmp(&self, other: &Self) -> Ordering {
        self.coverage_cmp(other)
            .then_with(|| self.typed_matches.cmp(&other.typed_matches))
    }
}

fn extract_urls(query: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut urls = Vec::new();
    for raw in query.split_whitespace() {
        if let Some(url) = normalize_url_candidate(raw)
            && seen.insert(url.clone())
        {
            urls.push(url);
        }
    }
    urls
}

fn normalize_url_candidate(raw: &str) -> Option<String> {
    let trimmed = raw.trim_matches(|c: char| {
        matches!(
            c,
            '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | '"' | '\'' | ',' | '.' | ';' | ':'
        )
    });
    if trimmed.is_empty() {
        return None;
    }
    if Url::parse(trimmed).is_ok() {
        return Some(trimmed.to_string());
    }
    if trimmed.contains('.') && !trimmed.contains(' ') {
        let candidate = format!("https://{trimmed}");
        if Url::parse(&candidate).is_ok() {
            return Some(candidate);
        }
    }
    None
}

#[derive(Clone, Copy)]
struct SchemaInfo<'a> {
    type_name: Option<&'a str>,
    format: Option<&'a str>,
    item_type: Option<&'a str>,
    item_format: Option<&'a str>,
}

fn schema_type(schema: &JsonValue) -> Option<&str> {
    match schema.get("type") {
        Some(JsonValue::String(value)) => Some(value),
        Some(JsonValue::Array(values)) => values
            .iter()
            .filter_map(JsonValue::as_str)
            .find(|value| *value == "string" || *value == "array")
            .or_else(|| values.iter().filter_map(JsonValue::as_str).next()),
        _ => None,
    }
}

fn schema_format(schema: &JsonValue) -> Option<&str> {
    schema.get("format").and_then(JsonValue::as_str)
}

fn schema_info(schema: &JsonValue) -> SchemaInfo<'_> {
    let type_name = schema_type(schema);
    let format = schema_format(schema);
    let (item_type, item_format) = if type_name == Some("array") {
        match schema.get("items") {
            Some(items) => (schema_type(items), schema_format(items)),
            None => (None, None),
        }
    } else {
        (None, None)
    };

    SchemaInfo {
        type_name,
        format,
        item_type,
        item_format,
    }
}

fn is_url_format(format: Option<&str>) -> bool {
    matches!(format, Some("uri") | Some("url") | Some("uri-reference"))
}

fn infer_value_from_schema(
    schema: &JsonValue,
    query: &str,
    urls: &[String],
    allow_query_fallback: bool,
) -> Option<JsonValue> {
    let info = schema_info(schema);
    match info.type_name {
        Some("string") => {
            if is_url_format(info.format) {
                urls.first().map(|url| JsonValue::String(url.clone()))
            } else if allow_query_fallback {
                Some(JsonValue::String(query.to_string()))
            } else {
                None
            }
        }
        Some("array") if info.item_type == Some("string") => {
            if is_url_format(info.item_format) {
                if urls.is_empty() {
                    None
                } else {
                    Some(JsonValue::Array(
                        urls.iter().map(|u| JsonValue::String(u.clone())).collect(),
                    ))
                }
            } else if allow_query_fallback {
                Some(JsonValue::Array(vec![JsonValue::String(query.to_string())]))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn infer_arguments(
    query: &str,
    urls: &[String],
    input_schema: &mcp_types::ToolInputSchema,
) -> ArgumentInference {
    let mut arguments = serde_json::Map::new();
    let required = input_schema.required.clone().unwrap_or_default();
    let required_total = required.len();
    let mut typed_matches = 0;

    let properties = input_schema
        .properties
        .as_ref()
        .and_then(JsonValue::as_object);
    let allow_query_fallback = required_total == 1;

    if required_total == 0 {
        if let Some((name, schema)) = properties.and_then(|props| {
            if props.len() == 1 {
                props.iter().next()
            } else {
                None
            }
        }) && let Some(value) = infer_value_from_schema(schema, query, urls, true)
        {
            arguments.insert(name.clone(), value);
            typed_matches += 1;
        }
    } else if let Some(properties) = properties {
        for required_name in &required {
            if arguments.contains_key(required_name) {
                continue;
            }
            if let Some(schema) = properties.get(required_name)
                && let Some(value) =
                    infer_value_from_schema(schema, query, urls, allow_query_fallback)
            {
                arguments.insert(required_name.clone(), value);
                typed_matches += 1;
            }
        }
    }

    let missing_required = required
        .iter()
        .filter(|name| !arguments.contains_key(*name))
        .cloned()
        .collect::<Vec<String>>();
    let required_filled = required_total.saturating_sub(missing_required.len());

    let arguments = if arguments.is_empty() {
        None
    } else {
        Some(JsonValue::Object(arguments))
    };

    ArgumentInference {
        arguments,
        missing_required,
        required_total,
        required_filled,
        typed_matches,
    }
}

struct RouteOutput {
    response: McpSearchRouteResponse,
    success: Option<bool>,
}

fn has_arguments(arguments: &Option<JsonValue>) -> bool {
    match arguments {
        Some(JsonValue::Object(map)) => !map.is_empty(),
        Some(_) => true,
        None => false,
    }
}

fn normalize_query(query: Option<&str>) -> Option<String> {
    query
        .map(str::trim)
        .filter(|val| !val.is_empty())
        .map(str::to_string)
}

async fn resolve_query(
    session: &crate::codex::Session,
    query: Option<&str>,
) -> Result<String, FunctionCallError> {
    if let Some(query) = normalize_query(query) {
        return Ok(query);
    }

    let history = session.clone_history().await;
    let last_user_message = history.raw_items().iter().rev().find_map(|item| {
        if let ResponseItem::Message { role, content, .. } = item
            && role == "user"
        {
            content_items_to_text(content)
        } else {
            None
        }
    });

    normalize_query(last_user_message.as_deref()).ok_or_else(|| {
        FunctionCallError::RespondToModel(
            "query must not be empty; provide query or ensure a user message is available"
                .to_string(),
        )
    })
}

async fn route_query(
    session: &crate::codex::Session,
    turn: &crate::codex::TurnContext,
    call_id: &str,
    query: &str,
    server_filter: Option<&str>,
    limit: usize,
    include_schema: bool,
    tools: std::collections::HashMap<String, crate::mcp_connection_manager::ToolInfo>,
) -> Result<RouteOutput, FunctionCallError> {
    let urls = extract_urls(query);
    let mut candidates = Vec::new();

    for (qualified_name, tool_info) in tools {
        if let Some(server) = server_filter
            && tool_info.server_name != server
        {
            continue;
        }

        let description = tool_info.tool.description.clone();
        let inference = infer_arguments(query, &urls, &tool_info.tool.input_schema);

        if inference.required_filled == 0 && inference.typed_matches == 0 {
            continue;
        }

        let schema = if include_schema {
            let input = serialize_input_schema(tool_info.tool.input_schema.clone())?;
            let output = tool_info
                .tool
                .output_schema
                .clone()
                .map(serialize_output_schema)
                .transpose()?;
            Some(McpToolSchema { input, output })
        } else {
            None
        };

        candidates.push(McpSearchRouteCandidate {
            qualified_name,
            server: tool_info.server_name,
            tool: tool_info.tool_name,
            description,
            schema,
            score: McpSearchRouteScore {
                required_filled: inference.required_filled,
                required_total: inference.required_total,
                typed_matches: inference.typed_matches,
            },
            arguments: inference.arguments,
            missing_required: inference.missing_required,
        });
    }

    candidates.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.qualified_name.cmp(&b.qualified_name))
    });

    let total_candidates = candidates.len();
    let limit = limit.min(MAX_LIMIT);
    let callable_candidates: Vec<&McpSearchRouteCandidate> = candidates
        .iter()
        .filter(|candidate| {
            candidate.missing_required.is_empty()
                && (candidate.score.required_total == 0 || has_arguments(&candidate.arguments))
        })
        .collect();

    let (decision, result, success) = match callable_candidates.as_slice() {
        [best] => {
            let arguments_str = match best.arguments.as_ref() {
                Some(arguments) => serde_json::to_string(arguments).map_err(|err| {
                    FunctionCallError::RespondToModel(format!(
                        "failed to serialize route arguments: {err}"
                    ))
                })?,
                None => String::new(),
            };
            let routed_call_id = format!("{call_id}:route:{}", Uuid::new_v4());
            let response = handle_mcp_tool_call(
                session,
                turn,
                routed_call_id,
                best.server.clone(),
                best.tool.clone(),
                arguments_str,
            )
            .await;
            let (result_kind, result_value, call_success) = match response {
                ResponseInputItem::McpToolCallOutput { result, .. } => {
                    let call_success = result.is_ok();
                    let value = serde_json::to_value(result).map_err(|err| {
                        FunctionCallError::RespondToModel(format!(
                            "failed to serialize MCP call result: {err}"
                        ))
                    })?;
                    ("mcp".to_string(), value, call_success)
                }
                ResponseInputItem::FunctionCallOutput { output, .. } => {
                    let call_success = output.success.unwrap_or(true);
                    let value = serde_json::to_value(output).map_err(|err| {
                        FunctionCallError::RespondToModel(format!(
                            "failed to serialize routed output: {err}"
                        ))
                    })?;
                    ("function".to_string(), value, call_success)
                }
                _ => {
                    return Err(FunctionCallError::RespondToModel(
                        "mcp_search route received unexpected response variant".to_string(),
                    ));
                }
            };

            let decision = McpSearchRouteDecision {
                action: "call".to_string(),
                reason: "selected tool has complete required arguments".to_string(),
                call: Some(McpSearchCallDescriptor {
                    qualified_name: best.qualified_name.clone(),
                    server: best.server.clone(),
                    tool: best.tool.clone(),
                    arguments: best.arguments.clone(),
                }),
            };
            let result = Some(McpSearchRouteResult {
                kind: result_kind,
                value: result_value,
            });
            (decision, result, Some(call_success))
        }
        [] => {
            let decision = McpSearchRouteDecision {
                action: "recommend".to_string(),
                reason: if total_candidates == 0 {
                    "no matching tools found".to_string()
                } else {
                    "no tool has complete required arguments".to_string()
                },
                call: None,
            };
            (decision, None, Some(true))
        }
        _ => {
            let decision = McpSearchRouteDecision {
                action: "recommend".to_string(),
                reason: "multiple tools have complete required arguments".to_string(),
                call: None,
            };
            (decision, None, Some(true))
        }
    };

    let candidates = candidates.into_iter().take(limit).collect();

    Ok(RouteOutput {
        response: McpSearchRouteResponse {
            query: query.to_string(),
            total_candidates,
            decision,
            candidates,
            result,
        },
        success,
    })
}

#[async_trait]
impl ToolHandler for McpSearchHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id,
            payload,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "mcp_search handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: McpSearchArgs = parse_arguments(&arguments)?;
        if args.limit == 0 {
            return Err(FunctionCallError::RespondToModel(
                "limit must be greater than zero".to_string(),
            ));
        }

        if let Some(resources) = args.resources {
            if args.route {
                return Err(FunctionCallError::RespondToModel(
                    "route cannot be used with resources".to_string(),
                ));
            }
            if args.call.is_some() {
                return Err(FunctionCallError::RespondToModel(
                    "resources cannot be used with call".to_string(),
                ));
            }
            if let Some(query) = args.query.as_deref()
                && !query.trim().is_empty()
            {
                return Err(FunctionCallError::RespondToModel(
                    "resources cannot be used with query".to_string(),
                ));
            }
            if let Some(server) = args.server.as_deref()
                && !server.trim().is_empty()
            {
                return Err(FunctionCallError::RespondToModel(
                    "resources cannot be used with server filter".to_string(),
                ));
            }
            if args.include_schema {
                return Err(FunctionCallError::RespondToModel(
                    "include_schema cannot be used with resources".to_string(),
                ));
            }

            let action = resources.action.trim();
            if action.is_empty() {
                return Err(FunctionCallError::RespondToModel(
                    "resources.action must not be empty".to_string(),
                ));
            }

            let server = resources
                .server
                .as_deref()
                .map(str::trim)
                .filter(|val| !val.is_empty());
            let cursor = resources
                .cursor
                .as_deref()
                .map(str::trim)
                .filter(|val| !val.is_empty());
            let uri = resources
                .uri
                .as_deref()
                .map(str::trim)
                .filter(|val| !val.is_empty());

            if action == "read" && (server.is_none() || uri.is_none()) {
                return Err(FunctionCallError::RespondToModel(
                    "resources.read requires server and uri".to_string(),
                ));
            }

            let mut args_map = serde_json::Map::new();
            if let Some(server) = server {
                args_map.insert("server".to_string(), JsonValue::String(server.to_string()));
            }
            if let Some(cursor) = cursor {
                args_map.insert("cursor".to_string(), JsonValue::String(cursor.to_string()));
            }
            if let Some(uri) = uri {
                args_map.insert("uri".to_string(), JsonValue::String(uri.to_string()));
            }
            let arguments_value = if args_map.is_empty() {
                None
            } else {
                Some(JsonValue::Object(args_map))
            };

            return match action {
                "list" | "list_resources" => {
                    handle_list_resources(
                        session.clone(),
                        turn.clone(),
                        call_id.clone(),
                        arguments_value,
                    )
                    .await
                }
                "list_templates" | "list_resource_templates" => {
                    handle_list_resource_templates(
                        session.clone(),
                        turn.clone(),
                        call_id.clone(),
                        arguments_value,
                    )
                    .await
                }
                "read" => {
                    handle_read_resource(
                        session.clone(),
                        turn.clone(),
                        call_id.clone(),
                        arguments_value,
                    )
                    .await
                }
                other => Err(FunctionCallError::RespondToModel(format!(
                    "unknown resources action: {other}"
                ))),
            };
        }

        let tools = session
            .services
            .mcp_connection_manager
            .read()
            .await
            .list_all_tools()
            .await;

        if args.route {
            if args.call.is_some() {
                return Err(FunctionCallError::RespondToModel(
                    "route cannot be used with call".to_string(),
                ));
            }

            let query = resolve_query(session.as_ref(), args.query.as_deref()).await?;

            let server_filter = args
                .server
                .as_deref()
                .map(str::trim)
                .filter(|val| !val.is_empty());
            let limit = args.limit.min(MAX_LIMIT);
            let RouteOutput { response, success } = route_query(
                session.as_ref(),
                turn.as_ref(),
                &call_id,
                &query,
                server_filter,
                limit,
                args.include_schema,
                tools,
            )
            .await?;
            let content = serde_json::to_string(&response).map_err(|err| {
                FunctionCallError::RespondToModel(format!(
                    "failed to serialize route response: {err}"
                ))
            })?;
            return Ok(ToolOutput::Function {
                content,
                content_items: None,
                success,
            });
        }

        if let Some(call) = args.call {
            if let Some(query) = args.query.as_deref()
                && !query.trim().is_empty()
            {
                return Err(FunctionCallError::RespondToModel(
                    "query cannot be used with call".to_string(),
                ));
            }
            if let Some(server) = args.server.as_deref()
                && !server.trim().is_empty()
            {
                return Err(FunctionCallError::RespondToModel(
                    "server filter cannot be used with call".to_string(),
                ));
            }
            if args.include_schema {
                return Err(FunctionCallError::RespondToModel(
                    "include_schema cannot be used with call".to_string(),
                ));
            }

            let (server, tool) = resolve_call_target(&call, &tools)?;
            let arguments_str = match call.arguments {
                Some(arguments) if !arguments.is_null() => serde_json::to_string(&arguments)
                    .map_err(|err| {
                        FunctionCallError::RespondToModel(format!(
                            "failed to serialize call arguments: {err}"
                        ))
                    })?,
                _ => String::new(),
            };

            let response = handle_mcp_tool_call(
                session.as_ref(),
                turn.as_ref(),
                call_id.clone(),
                server,
                tool,
                arguments_str,
            )
            .await;

            return match response {
                ResponseInputItem::McpToolCallOutput { result, .. } => {
                    Ok(ToolOutput::Mcp { result })
                }
                ResponseInputItem::FunctionCallOutput { output, .. } => {
                    let codex_protocol::models::FunctionCallOutputPayload {
                        content,
                        content_items,
                        success,
                    } = output;
                    Ok(ToolOutput::Function {
                        content,
                        content_items,
                        success,
                    })
                }
                _ => Err(FunctionCallError::RespondToModel(
                    "mcp_search call received unexpected response variant".to_string(),
                )),
            };
        }

        let query = resolve_query(session.as_ref(), args.query.as_deref()).await?;

        let limit = args.limit.min(MAX_LIMIT);
        let query_lc = query.to_lowercase();
        let server_filter = args
            .server
            .as_deref()
            .map(str::trim)
            .filter(|val| !val.is_empty());

        let mut matches = Vec::new();
        for (qualified_name, tool_info) in tools {
            if let Some(server) = server_filter
                && tool_info.server_name != server
            {
                continue;
            }

            let tool_name_lc = tool_info.tool_name.to_lowercase();
            let qualified_name_lc = qualified_name.to_lowercase();
            let description = tool_info.tool.description.as_deref().unwrap_or("");
            let description_lc = description.to_lowercase();
            let mut score = 0;
            if tool_name_lc.contains(&query_lc) || qualified_name_lc.contains(&query_lc) {
                score += 2;
            }
            if description_lc.contains(&query_lc) {
                score += 1;
            }

            if score == 0 {
                continue;
            }

            let schema = if args.include_schema {
                let input = serialize_input_schema(tool_info.tool.input_schema.clone())?;
                let output = tool_info
                    .tool
                    .output_schema
                    .clone()
                    .map(serialize_output_schema)
                    .transpose()?;
                Some(McpToolSchema { input, output })
            } else {
                None
            };

            matches.push((
                score,
                qualified_name,
                tool_info.server_name,
                tool_info.tool_name,
                tool_info.tool.description,
                schema,
            ));
        }

        matches.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        let total_matches = matches.len();
        let results = matches
            .into_iter()
            .take(limit)
            .map(
                |(_, qualified_name, server, tool, description, schema)| McpSearchResult {
                    qualified_name,
                    server,
                    tool,
                    description,
                    schema,
                },
            )
            .collect();

        let response = McpSearchResponse {
            query,
            total_matches,
            results,
        };
        let content = serde_json::to_string(&response).map_err(|err| {
            FunctionCallError::RespondToModel(format!("failed to serialize response: {err}"))
        })?;

        Ok(ToolOutput::Function {
            content,
            content_items: None,
            success: Some(true),
        })
    }
}
