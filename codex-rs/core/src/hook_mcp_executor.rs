use std::sync::Arc;

use anyhow::Context;
use anyhow::bail;
use codex_hooks::HookMcpCall;
use codex_hooks::HookMcpExecutor;
use codex_mcp::McpRuntime;
use codex_protocol::ThreadId;
use futures::FutureExt;
use futures::future::BoxFuture;
use serde_json::Value;

pub(crate) struct CoreHookMcpExecutor {
    pub(crate) runtime: Arc<McpRuntime>,
    // Session-scoped MCP tools require the owning thread ID in request metadata.
    pub(crate) thread_id: ThreadId,
}

impl HookMcpExecutor for CoreHookMcpExecutor {
    fn execute(&self, call: HookMcpCall) -> BoxFuture<'_, anyhow::Result<String>> {
        async move {
            let prepared_call = self
                .runtime
                .prepare_call_if_connected(&call.server, &call.tool)
                .await
                .with_context(|| {
                    format!(
                        "MCP server `{}` or tool `{}` is not connected and available",
                        call.server, call.tool
                    )
                })?;

            let result = prepared_call
                .call(
                    Some(Value::Object(call.input)),
                    Some(serde_json::json!({ "threadId": self.thread_id.to_string() })),
                    Some(call.timeout),
                )
                .await?;
            let text = result
                .content
                .iter()
                .filter_map(|content| {
                    (content.get("type").and_then(Value::as_str) == Some("text"))
                        .then(|| content.get("text").and_then(Value::as_str))
                        .flatten()
                })
                .collect::<Vec<_>>()
                .join("\n");
            if result.is_error == Some(true) {
                bail!("MCP tool returned an error: {text}");
            }

            Ok(text)
        }
        .boxed()
    }
}
