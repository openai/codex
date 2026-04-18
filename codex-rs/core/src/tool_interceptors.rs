use std::process::Stdio;
use std::time::Instant;

use codex_config::types::ToolInterceptorHandlerToml;
use codex_config::types::ToolInterceptorRuleToml;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::function_tool::FunctionCallError;
use crate::original_image_detail::can_request_original_image_detail;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::McpToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::AnyToolResult;

#[derive(Serialize)]
struct ToolInterceptorRequest<'a> {
    tool_name: String,
    call_id: &'a str,
    cwd: &'a AbsolutePathBuf,
    kind: &'static str,
    operation: Option<&'a str>,
    payload: Value,
}

#[derive(Deserialize)]
struct ToolInterceptorResponse {
    output: Option<String>,
    content_items: Option<Vec<FunctionCallOutputContentItem>>,
    mcp_result: Option<CallToolResult>,
    success: Option<bool>,
}

pub(crate) async fn maybe_intercept(
    invocation: &ToolInvocation,
) -> Result<Option<AnyToolResult>, FunctionCallError> {
    let Some(tool_interceptors) = invocation.turn.config.tool_interceptors.as_ref() else {
        return Ok(None);
    };

    let tool_name = invocation.tool_name.display();
    let Some(rule) = tool_interceptors
        .rules
        .iter()
        .find(|rule| rule.tool == tool_name)
    else {
        return Ok(None);
    };

    let Some(handler) = tool_interceptors.handlers.get(&rule.handler) else {
        return Err(FunctionCallError::Fatal(format!(
            "tool interceptor `{}` references missing handler `{}`",
            rule.tool, rule.handler
        )));
    };

    let started = Instant::now();
    let response = invoke_handler(invocation, &tool_name, rule, handler).await?;
    let result = interceptor_response_to_tool_result(invocation, response, started);
    Ok(Some(result))
}

async fn invoke_handler(
    invocation: &ToolInvocation,
    tool_name: &str,
    rule: &ToolInterceptorRuleToml,
    handler: &ToolInterceptorHandlerToml,
) -> Result<ToolInterceptorResponse, FunctionCallError> {
    if handler.command.trim().is_empty() {
        return Err(FunctionCallError::Fatal(format!(
            "tool interceptor handler for `{tool_name}` has an empty command"
        )));
    }

    let request = ToolInterceptorRequest {
        tool_name: tool_name.to_string(),
        call_id: invocation.call_id.as_str(),
        cwd: &invocation.turn.cwd,
        kind: payload_kind(&invocation.payload),
        operation: rule.operation.as_deref(),
        payload: payload_json(&invocation.payload),
    };
    let request_bytes = serde_json::to_vec(&request).map_err(|err| {
        FunctionCallError::Fatal(format!("failed to serialize interceptor request: {err}"))
    })?;

    let mut command = Command::new(&handler.command);
    command.args(&handler.args);
    if let Some(cwd) = handler.cwd.as_ref() {
        command.current_dir(cwd);
    }
    command.envs(&handler.env);
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command.spawn().map_err(|err| {
        FunctionCallError::Fatal(format!(
            "failed to spawn interceptor for `{tool_name}`: {err}"
        ))
    })?;

    let Some(mut stdin) = child.stdin.take() else {
        return Err(FunctionCallError::Fatal(format!(
            "failed to open interceptor stdin for `{tool_name}`"
        )));
    };
    stdin.write_all(&request_bytes).await.map_err(|err| {
        FunctionCallError::Fatal(format!(
            "failed to write interceptor request for `{tool_name}`: {err}"
        ))
    })?;
    drop(stdin);

    let output = child.wait_with_output().await.map_err(|err| {
        FunctionCallError::Fatal(format!(
            "failed to read interceptor output for `{tool_name}`: {err}"
        ))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(FunctionCallError::RespondToModel(format!(
            "tool interceptor for `{tool_name}` exited with {}: {}",
            output.status,
            stderr.trim()
        )));
    }

    serde_json::from_slice::<ToolInterceptorResponse>(&output.stdout).map_err(|err| {
        let stdout = String::from_utf8_lossy(&output.stdout);
        FunctionCallError::RespondToModel(format!(
            "tool interceptor for `{tool_name}` returned invalid JSON: {err}; stdout: {}",
            stdout.trim()
        ))
    })
}

fn interceptor_response_to_tool_result(
    invocation: &ToolInvocation,
    response: ToolInterceptorResponse,
    started: Instant,
) -> AnyToolResult {
    let ToolInterceptorResponse {
        output,
        content_items,
        mcp_result,
        success,
    } = response;

    let result: Box<dyn ToolOutput> = match &invocation.payload {
        ToolPayload::Mcp { .. } => Box::new(McpToolOutput {
            result: mcp_result.unwrap_or_else(|| CallToolResult {
                content: vec![serde_json::json!({
                    "type": "text",
                    "text": output.unwrap_or_default(),
                })],
                structured_content: None,
                is_error: success.map(|success| !success),
                meta: None,
            }),
            wall_time: started.elapsed(),
            original_image_detail_supported: can_request_original_image_detail(
                &invocation.turn.model_info,
            ),
        }),
        _ => {
            if let Some(content_items) = content_items {
                Box::new(FunctionToolOutput::from_content(content_items, success))
            } else {
                Box::new(FunctionToolOutput::from_text(
                    output.unwrap_or_default(),
                    success,
                ))
            }
        }
    };

    AnyToolResult {
        call_id: invocation.call_id.clone(),
        payload: invocation.payload.clone(),
        result,
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

fn payload_json(payload: &ToolPayload) -> Value {
    match payload {
        ToolPayload::Function { arguments } => serde_json::json!({
            "arguments": arguments,
            "arguments_json": parse_json(arguments),
        }),
        ToolPayload::ToolSearch { arguments } => serde_json::json!({
            "arguments": {
                "query": &arguments.query,
                "limit": arguments.limit,
            },
        }),
        ToolPayload::Custom { input } => serde_json::json!({
            "input": input,
        }),
        ToolPayload::LocalShell { params } => serde_json::json!({
            "arguments": {
                "command": &params.command,
                "workdir": &params.workdir,
                "timeout_ms": params.timeout_ms,
                "sandbox_permissions": params.sandbox_permissions,
                "prefix_rule": &params.prefix_rule,
                "additional_permissions": &params.additional_permissions,
                "justification": &params.justification,
            },
        }),
        ToolPayload::Mcp {
            server,
            tool,
            raw_arguments,
        } => serde_json::json!({
            "server": server,
            "tool": tool,
            "arguments": raw_arguments,
            "arguments_json": parse_json(raw_arguments),
        }),
    }
}

fn parse_json(raw: &str) -> Option<Value> {
    serde_json::from_str(raw).ok()
}

#[cfg(test)]
#[path = "tool_interceptors_tests.rs"]
mod tests;
