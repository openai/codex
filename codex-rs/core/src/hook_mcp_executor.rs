//! Executes hook MCP calls; bundled Apps require current connector and approval authority.

use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::Weak;

use anyhow::Context;
use anyhow::bail;
use codex_hooks::HookMcpCall;
use codex_hooks::HookMcpExecutor;
use codex_hooks::HookMcpOutput;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::McpRuntime;
use codex_mcp::McpToolCallResult;
use codex_mcp::McpToolStartup;
use codex_protocol::ThreadId;
use futures::FutureExt;
use futures::future::BoxFuture;
use serde_json::Value;

use crate::mcp_tool_call::bundled_app_hook_connector;
use crate::session::session::Session;

#[cfg(test)]
#[path = "hook_mcp_executor_tests.rs"]
mod tests;

pub(crate) struct CoreHookMcpExecutor {
    pub(crate) session: OnceLock<Weak<Session>>,
    pub(crate) runtime: Arc<McpRuntime>,
    // Session-scoped MCP tools require the owning thread ID in request metadata.
    pub(crate) thread_id: ThreadId,
}

impl HookMcpExecutor for CoreHookMcpExecutor {
    fn execute(&self, call: HookMcpCall) -> BoxFuture<'_, anyhow::Result<HookMcpOutput>> {
        async move {
            let mut metadata = call.metadata.unwrap_or_default();
            metadata.insert(
                "threadId".to_string(),
                Value::String(self.thread_id.to_string()),
            );

            let call_result = tokio::time::timeout(
                call.timeout,
                async {
                    if let Some(expected_connector) = call.expected_app_connector_id {
                        anyhow::ensure!(
                            call.server == CODEX_APPS_MCP_SERVER_NAME && call.environment_id.is_none(),
                            "bundled App hook must use the host Apps server"
                        );
                        let session = self
                            .session
                            .get()
                            .and_then(Weak::upgrade)
                            .context("App hook session is unavailable")?;
                        loop {
                            // Invalidation happens before asynchronous publication. Capture first
                            // so refresh cannot pair new authority with an old runtime.
                            let admit = self.runtime.capture_app_hook_admission();
                            session.refresh_mcp_if_dirty().await;
                            session.wait_for_apps_tool_refresh().await;
                            if admit().is_err() {
                                continue;
                            }
                            let prepared = self
                                .runtime
                                .prepare_app_hook_call(&call.tool)
                                .await;
                            if admit().is_err() {
                                continue;
                            }
                            let prepared = prepared?;
                            let result = prepared.call_with_admission(
                                Some(call.timeout),
                                || {
                                    admit()?;
                                    anyhow::ensure!(
                                        bundled_app_hook_connector(&prepared) == Some(expected_connector),
                                        "App hook connector or approval policy does not permit automatic execution"
                                    );
                                    Ok(())
                                },
                                || async {
                                    Ok((Some(Value::Object(call.input)), Some(Value::Object(metadata))))
                                },
                            ).await?;
                            // Never retry a dispatched call: it may already have had side effects.
                            break Ok(McpToolCallResult {
                                result,
                                source_tool_namespace: Some(prepared.tool_info().callable_namespace.clone()),
                            });
                        }
                    } else {
                        self.runtime.latest_call_tool(
                            &call.server,
                            &call.tool,
                            call.environment_id.as_deref(),
                            Some(Value::Object(call.input)),
                            Some(Value::Object(metadata)),
                            Some(call.timeout),
                            if call.skip_dormant_repl_cleanup {
                                McpToolStartup::SkipDormantReplCleanup
                            } else {
                                McpToolStartup::Wait
                            },
                        ).await
                    }
                },
            )
            .await
            .context("MCP hook timed out")??;
            let result = call_result.result;
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

            Ok(HookMcpOutput {
                structured_content: result.structured_content,
                text,
                source_tool_namespace: call_result.source_tool_namespace,
            })
        }
        .boxed()
    }
}
