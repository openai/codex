use std::sync::LazyLock;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use codex_protocol::mcp::is_node_repl_backed_server;
use codex_protocol::models::ContentItemMetadata;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookExecutionMode;
use regex::Regex;
use serde_json::Map;
use serde_json::Value;

use super::ConfiguredHandler;
use super::ConfiguredHandlerKind;
use super::HandlerRunResult;
use super::HandlerSourcePath;
use crate::mcp::HookMcpCall;
use crate::mcp::HookMcpExecutor;

// The memory producer bounds the decrypted tokens. The runner can only bound the ciphertext.
const MAX_ENCRYPTED_CONTEXT_BYTES: usize = 40_000;

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
    hook_event: &Value,
    metadata: Option<&Map<String, Value>>,
) -> HandlerRunResult {
    let started_at = chrono::Utc::now().timestamp();
    let started = Instant::now();
    let timeout_started = tokio::time::Instant::now();
    let duration = Duration::from_secs(handler.timeout_sec);
    let result = async {
        if let Some(trust) = &handler.plugin_trust {
            tokio::time::timeout(duration, trust.verify())
                .await
                .context("hook trust verification timed out")??;
        }
        let input = expand_mcp_argument_template(argument_template, hook_event)?;
        let (environment_id, mut call_metadata) = match &handler.source_path {
            HandlerSourcePath::Local(_) => (None, None),
            HandlerSourcePath::Environment { environment_id, .. } => {
                (Some(environment_id.clone()), None)
            }
            HandlerSourcePath::ExecutorScoped {
                environment_id,
                mcp_environment_id,
                mcp_metadata,
                ..
            } => (
                Some(
                    mcp_environment_id
                        .as_ref()
                        .unwrap_or(environment_id)
                        .clone(),
                ),
                mcp_metadata.as_deref().cloned(),
            ),
        };
        if let Some(metadata) = metadata {
            call_metadata
                .get_or_insert_with(Map::new)
                .extend(metadata.clone());
        }
        let output = executor
            .execute(HookMcpCall {
                skip_dormant_repl_cleanup: handler.builtin
                    && is_node_repl_backed_server(server)
                    && tool == "turn_ended",
                expected_app_connector_id: match &handler.kind {
                    ConfiguredHandlerKind::McpTool {
                        expected_app_connector_id,
                        ..
                    } => *expected_app_connector_id,
                    ConfiguredHandlerKind::Command { .. } => None,
                },
                server: server.to_string(),
                tool: tool.to_string(),
                environment_id,
                metadata: call_metadata,
                input,
                timeout: if handler.plugin_trust.is_some() {
                    duration.saturating_sub(timeout_started.elapsed())
                } else {
                    Duration::from_secs(handler.timeout_sec)
                },
            })
            .await?;
        let mut plaintext = Vec::new();
        let mut encrypted_context = Vec::new();
        let mut encrypted_bytes = 0;
        for block in &output.result.content {
            if block.get("type").and_then(Value::as_str) != Some("text") {
                continue;
            }
            let Some(text) = block.get("text").and_then(Value::as_str) else {
                continue;
            };
            if handler.event_name == HookEventName::SessionStart
                && let Ok(part @ FunctionCallOutputContentItem::EncryptedContent { .. }) =
                    FunctionCallOutputContentItem::from_mcp_content_block(block)
            {
                anyhow::ensure!(
                    handler.execution_mode() == HookExecutionMode::Sync,
                    "encrypted SessionStart context requires a synchronous MCP hook"
                );
                anyhow::ensure!(
                    !text.is_empty() && text.len() <= MAX_ENCRYPTED_CONTEXT_BYTES,
                    "invalid encrypted SessionStart context size"
                );
                encrypted_bytes += serde_json::to_vec(&part)?.len();
                anyhow::ensure!(
                    encrypted_bytes <= MAX_ENCRYPTED_CONTEXT_BYTES,
                    "encrypted SessionStart context exceeds 40,000 bytes"
                );
                encrypted_context.push(crate::HookContextPayload::EncryptedToolOutput {
                    name: tool.to_owned(),
                    namespace: output.source_tool_namespace.clone(),
                    encrypted_content: text.to_owned(),
                });
            } else {
                plaintext.push(text);
            }
        }
        let stdout = plaintext.join("\n");
        anyhow::ensure!(
            output.result.is_error != Some(true),
            "MCP tool returned an error: {stdout}"
        );
        Ok((
            stdout,
            encrypted_context,
            output.source_tool_namespace,
            output.result.structured_content,
        ))
    }
    .await;

    let (exit_code, stdout, encrypted_context, error, source_tool_namespace, structured_content) =
        match result {
            Ok((stdout, context, namespace, structured_content)) => (
                Some(0),
                stdout,
                context,
                None,
                namespace,
                structured_content,
            ),
            Err(error) => (
                None,
                String::new(),
                Vec::new(),
                Some(error.to_string()),
                None,
                None,
            ),
        };
    HandlerRunResult {
        encrypted_context,
        structured_content,
        context_metadata: ContentItemMetadata::tool(source_tool_namespace.map(Into::into)),
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
    static PLACEHOLDER_PATTERN: LazyLock<Result<Regex, regex::Error>> =
        LazyLock::new(|| Regex::new(r"\$\{([^{}]+)\}"));
    let pattern = PLACEHOLDER_PATTERN.as_ref().map_err(Clone::clone)?;
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
