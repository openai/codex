use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use regex::Regex;
use serde_json::Map;
use serde_json::Value;

use super::ConfiguredHandler;
use super::HandlerRunResult;
use super::HandlerSourcePath;
use crate::mcp::HookMcpCall;
use crate::mcp::HookMcpExecutor;

/// Expands an MCP argument template against this hook event and invokes the configured tool.
#[tracing::instrument(
    name = "codex.hooks.mcp_tool",
    level = "trace",
    skip_all,
    fields(
        hook.server = server,
        hook.tool = tool,
        hook.timeout_sec = handler.timeout_sec,
    )
)]
pub(crate) async fn run_mcp_tool(
    executor: &dyn HookMcpExecutor,
    handler: &ConfiguredHandler,
    server: &str,
    tool: &str,
    argument_template: &Map<String, Value>,
    hook_event_json: &str,
    metadata: Option<&Map<String, Value>>,
) -> HandlerRunResult {
    let started_at = chrono::Utc::now().timestamp();
    let started = Instant::now();
    let result = async {
        let hook_event: Value =
            serde_json::from_str(hook_event_json).context("failed to parse hook event input")?;
        let input = expand_mcp_argument_template(argument_template, &hook_event)?;
        executor
            .execute(HookMcpCall {
                server: server.to_string(),
                tool: tool.to_string(),
                environment_id: match &handler.source_path {
                    HandlerSourcePath::Local(_) => None,
                    HandlerSourcePath::ExecutorScoped { environment_id, .. } => {
                        Some(environment_id.clone())
                    }
                },
                metadata: metadata.cloned(),
                input,
                timeout: Duration::from_secs(handler.timeout_sec),
            })
            .await
    }
    .await;

    let (exit_code, stdout, error) = match result {
        Ok(output) => (Some(0), output, None),
        Err(error) => (None, String::new(), Some(error.to_string())),
    };
    HandlerRunResult {
        started_at,
        completed_at: chrono::Utc::now().timestamp(),
        duration_ms: started.elapsed().as_millis().try_into().unwrap_or(i64::MAX),
        exit_code,
        stdout,
        stderr: String::new(),
        error,
    }
}

/// Recursively substitutes `${field.nested}` placeholders using values from a hook event.
///
/// A complete placeholder preserves its JSON type; a placeholder embedded in surrounding
/// text is rendered as a string. Missing fields fail the hook instead of passing unresolved
/// arguments to the server. For example, the template `{"count":"${tool_input.count}"}`
/// becomes `{"count":3}` when the event contains `{"tool_input":{"count":3}}`.
pub(crate) fn expand_mcp_argument_template(
    argument_template: &Map<String, Value>,
    hook_event: &Value,
) -> Result<Map<String, Value>> {
    argument_template
        .iter()
        .map(|(key, value)| Ok((key.clone(), resolve_value(value, hook_event)?)))
        .collect()
}

fn resolve_value(value: &Value, hook_event: &Value) -> Result<Value> {
    match value {
        Value::Object(input) => Ok(Value::Object(expand_mcp_argument_template(
            input, hook_event,
        )?)),
        Value::Array(items) => items
            .iter()
            .map(|item| resolve_value(item, hook_event))
            .collect::<Result<Vec<_>>>()
            .map(Value::Array),
        Value::String(text) => resolve_string(text, hook_event),
        _ => Ok(value.clone()),
    }
}

fn resolve_string(text: &str, hook_event: &Value) -> Result<Value> {
    let pattern = Regex::new(r"\$\{([^{}]+)\}")?;
    let captures = pattern.captures_iter(text).collect::<Vec<_>>();
    if captures.is_empty() {
        return Ok(Value::String(text.to_string()));
    }

    if let [capture] = captures.as_slice()
        && let Some(placeholder) = capture.get(0)
        && placeholder.start() == 0
        && placeholder.end() == text.len()
    {
        return resolve_path(hook_event, &capture[1]).cloned();
    }

    let mut resolved = String::new();
    let mut previous_end = 0;
    for capture in captures {
        let Some(placeholder) = capture.get(0) else {
            continue;
        };
        resolved.push_str(&text[previous_end..placeholder.start()]);
        let value = resolve_path(hook_event, &capture[1])?;
        match value {
            Value::String(value) => resolved.push_str(value),
            _ => resolved.push_str(&serde_json::to_string(value)?),
        }
        previous_end = placeholder.end();
    }
    resolved.push_str(&text[previous_end..]);
    Ok(Value::String(resolved))
}

fn resolve_path<'a>(hook_event: &'a Value, path: &str) -> Result<&'a Value> {
    path.split('.').try_fold(hook_event, |value, field| {
        value
            .get(field)
            .with_context(|| format!("hook input placeholder `${{{path}}}` was not found"))
    })
}

#[cfg(test)]
#[path = "mcp_runner_tests.rs"]
mod tests;
