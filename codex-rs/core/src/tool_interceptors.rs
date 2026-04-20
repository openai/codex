use std::time::Duration;
use std::time::Instant;

use codex_protocol::mcp::CallToolResult;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_tools::ToolSpec;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::function_tool::FunctionCallError;
use crate::original_image_detail::can_request_original_image_detail;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::McpToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::AnyToolResult;

const INTERCEPTOR_API_VERSION: u32 = 1;
const INTERCEPTOR_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const TOOL_CALL_PATH: &str = "tool-call";

#[derive(Serialize)]
struct ToolInterceptorRequest<'a> {
    protocol_version: u32,
    call: ToolInterceptorCall<'a>,
    context: ToolInterceptorContext<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_schema: Option<Value>,
}

#[derive(Serialize)]
struct ToolInterceptorCall<'a> {
    id: &'a str,
    tool_name: String,
    kind: &'static str,
    input: Value,
    raw_input: Option<&'a str>,
    mcp: Option<ToolInterceptorMcpCall<'a>>,
}

#[derive(Serialize)]
struct ToolInterceptorMcpCall<'a> {
    server: &'a str,
    tool: &'a str,
}

#[derive(Serialize)]
struct ToolInterceptorContext<'a> {
    cwd: &'a AbsolutePathBuf,
    thread_id: String,
    turn_id: &'a str,
    trace_id: Option<&'a str>,
    model: &'a str,
}

#[derive(Deserialize)]
struct ToolInterceptorResponse {
    protocol_version: u32,
    #[serde(flatten)]
    action: ToolInterceptorAction,
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum ToolInterceptorAction {
    Continue,
    Replace { result: ToolInterceptorResult },
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ToolInterceptorResult {
    Text {
        text: String,
        success: Option<bool>,
    },
    ContentItems {
        content_items: Vec<FunctionCallOutputContentItem>,
        success: Option<bool>,
    },
    Mcp {
        value: CallToolResult,
    },
}

pub(crate) async fn maybe_intercept(
    invocation: &ToolInvocation,
    tool_schema: Option<&ToolSpec>,
) -> Result<Option<AnyToolResult>, FunctionCallError> {
    let Some(interceptor_url) = invocation.turn.config.tool_interceptor.as_deref() else {
        return Ok(None);
    };

    let started = Instant::now();
    let response = call_interceptor(invocation, interceptor_url, tool_schema).await?;
    let ToolInterceptorAction::Replace {
        result: replacement,
    } = response.action
    else {
        return Ok(None);
    };

    let result = replacement_to_tool_result(invocation, replacement, started)?;
    Ok(Some(result))
}

async fn call_interceptor(
    invocation: &ToolInvocation,
    configured_url: &str,
    tool_schema: Option<&ToolSpec>,
) -> Result<ToolInterceptorResponse, FunctionCallError> {
    let tool_name = invocation.tool_name.display();
    let url = interceptor_endpoint(configured_url, &tool_name)?;
    let request = ToolInterceptorRequest {
        protocol_version: INTERCEPTOR_API_VERSION,
        call: ToolInterceptorCall {
            id: invocation.call_id.as_str(),
            tool_name,
            kind: payload_kind(&invocation.payload),
            input: payload_input_json(&invocation.payload),
            raw_input: raw_input(&invocation.payload),
            mcp: mcp_call(&invocation.payload),
        },
        context: ToolInterceptorContext {
            cwd: &invocation.turn.cwd,
            thread_id: invocation.session.conversation_id.to_string(),
            turn_id: invocation.turn.sub_id.as_str(),
            trace_id: invocation.turn.trace_id.as_deref(),
            model: invocation.turn.model_info.slug.as_str(),
        },
        tool_schema: tool_schema_json(tool_schema)?,
    };

    let client = reqwest::Client::builder()
        .timeout(INTERCEPTOR_REQUEST_TIMEOUT)
        .build()
        .map_err(|err| {
            FunctionCallError::Fatal(format!("failed to build tool interceptor client: {err}"))
        })?;

    let response = client
        .post(url.clone())
        .json(&request)
        .send()
        .await
        .map_err(|err| {
            FunctionCallError::Fatal(format!("tool interceptor request to `{url}` failed: {err}"))
        })?;

    let status = response.status();
    let body = response.bytes().await.map_err(|err| {
        FunctionCallError::Fatal(format!("failed to read tool interceptor response: {err}"))
    })?;

    if !status.is_success() {
        return Err(FunctionCallError::Fatal(format!(
            "tool interceptor `{url}` returned HTTP {status}: {}",
            String::from_utf8_lossy(&body).trim()
        )));
    }

    let response = serde_json::from_slice::<ToolInterceptorResponse>(&body).map_err(|err| {
        FunctionCallError::Fatal(format!(
            "tool interceptor `{url}` returned invalid JSON: {err}; body: {}",
            String::from_utf8_lossy(&body).trim()
        ))
    })?;

    if response.protocol_version != INTERCEPTOR_API_VERSION {
        return Err(FunctionCallError::Fatal(format!(
            "tool interceptor `{url}` returned protocol_version {}, expected {}",
            response.protocol_version, INTERCEPTOR_API_VERSION
        )));
    }

    Ok(response)
}

fn interceptor_endpoint(
    configured_url: &str,
    tool_name: &str,
) -> Result<reqwest::Url, FunctionCallError> {
    let mut url = reqwest::Url::parse(configured_url).map_err(|err| {
        FunctionCallError::Fatal(format!(
            "tool_interceptor must be an HTTP URL for `{tool_name}`: {err}"
        ))
    })?;

    match url.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(FunctionCallError::Fatal(format!(
                "tool_interceptor for `{tool_name}` must use http or https, got `{scheme}`"
            )));
        }
    }

    let Some(host) = url.host_str() else {
        return Err(FunctionCallError::Fatal(format!(
            "tool_interceptor for `{tool_name}` must include a host"
        )));
    };
    if !matches!(host, "127.0.0.1" | "localhost" | "::1") {
        return Err(FunctionCallError::Fatal(format!(
            "tool_interceptor for `{tool_name}` must point to localhost, got `{host}`"
        )));
    }

    if matches!(url.path(), "" | "/") {
        url.set_path(TOOL_CALL_PATH);
    }
    Ok(url)
}

fn replacement_to_tool_result(
    invocation: &ToolInvocation,
    replacement: ToolInterceptorResult,
    started: Instant,
) -> Result<AnyToolResult, FunctionCallError> {
    let result: Box<dyn ToolOutput> = match &invocation.payload {
        ToolPayload::Mcp { .. } => {
            Box::new(mcp_replacement_to_output(invocation, replacement, started)?)
        }
        _ => Box::new(function_replacement_to_output(replacement)?),
    };

    Ok(AnyToolResult {
        call_id: invocation.call_id.clone(),
        payload: invocation.payload.clone(),
        result,
    })
}

fn mcp_replacement_to_output(
    invocation: &ToolInvocation,
    replacement: ToolInterceptorResult,
    started: Instant,
) -> Result<McpToolOutput, FunctionCallError> {
    let result = match replacement {
        ToolInterceptorResult::Mcp { value } => value,
        ToolInterceptorResult::Text { text, success } => CallToolResult {
            content: vec![serde_json::json!({
                "type": "text",
                "text": text,
            })],
            structured_content: None,
            is_error: success.map(|success| !success),
            meta: None,
        },
        ToolInterceptorResult::ContentItems { .. } => {
            return Err(FunctionCallError::Fatal(
                "tool interceptor returned content_items for an MCP tool; use text or mcp"
                    .to_string(),
            ));
        }
    };

    Ok(McpToolOutput {
        result,
        wall_time: started.elapsed(),
        original_image_detail_supported: can_request_original_image_detail(
            &invocation.turn.model_info,
        ),
    })
}

fn function_replacement_to_output(
    replacement: ToolInterceptorResult,
) -> Result<FunctionToolOutput, FunctionCallError> {
    match replacement {
        ToolInterceptorResult::Text { text, success } => {
            Ok(FunctionToolOutput::from_text(text, success))
        }
        ToolInterceptorResult::ContentItems {
            content_items,
            success,
        } => Ok(FunctionToolOutput::from_content(content_items, success)),
        ToolInterceptorResult::Mcp { .. } => Err(FunctionCallError::Fatal(
            "tool interceptor returned mcp for a non-MCP tool; use text or content_items"
                .to_string(),
        )),
    }
}

fn payload_kind(payload: &ToolPayload) -> &'static str {
    match payload {
        ToolPayload::Function { .. } => "function",
        ToolPayload::ToolSearch { .. } => "tool_search",
        ToolPayload::Custom { .. } => "custom",
        ToolPayload::LocalShell { .. } => "local_shell",
        ToolPayload::Mcp { .. } => "mcp",
    }
}

fn payload_input_json(payload: &ToolPayload) -> Value {
    match payload {
        ToolPayload::Function { arguments } => serde_json::json!({
            "arguments_json": parse_json(arguments),
        }),
        ToolPayload::ToolSearch { arguments } => serde_json::json!({
            "query": &arguments.query,
            "limit": arguments.limit,
        }),
        ToolPayload::Custom { input } => serde_json::json!({
            "input": input,
        }),
        ToolPayload::LocalShell { params } => serde_json::json!({
            "command": &params.command,
            "workdir": &params.workdir,
            "timeout_ms": params.timeout_ms,
            "sandbox_permissions": params.sandbox_permissions,
            "prefix_rule": &params.prefix_rule,
            "additional_permissions": &params.additional_permissions,
            "justification": &params.justification,
        }),
        ToolPayload::Mcp { raw_arguments, .. } => serde_json::json!({
            "arguments_json": parse_json(raw_arguments),
        }),
    }
}

fn raw_input(payload: &ToolPayload) -> Option<&str> {
    match payload {
        ToolPayload::Function { arguments }
        | ToolPayload::Mcp {
            raw_arguments: arguments,
            ..
        } => Some(arguments.as_str()),
        ToolPayload::Custom { input } => Some(input.as_str()),
        ToolPayload::ToolSearch { .. } | ToolPayload::LocalShell { .. } => None,
    }
}

fn mcp_call(payload: &ToolPayload) -> Option<ToolInterceptorMcpCall<'_>> {
    match payload {
        ToolPayload::Mcp {
            server,
            tool,
            raw_arguments: _,
        } => Some(ToolInterceptorMcpCall {
            server: server.as_str(),
            tool: tool.as_str(),
        }),
        _ => None,
    }
}

fn tool_schema_json(tool_schema: Option<&ToolSpec>) -> Result<Option<Value>, FunctionCallError> {
    tool_schema
        .map(serde_json::to_value)
        .transpose()
        .map_err(|err| FunctionCallError::Fatal(format!("failed to serialize tool schema: {err}")))
}

fn parse_json(raw: &str) -> Option<Value> {
    serde_json::from_str(raw).ok()
}

#[cfg(test)]
#[path = "tool_interceptors_tests.rs"]
mod tests;
