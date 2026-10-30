//! Host-provided metadata for a specific tool on a specific MCP registration.

use std::sync::Arc;

use crate::McpServerSource;
use crate::PreparedMcpCall;
use anyhow::Context;
use codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID;
use codex_config::McpServerTransportConfig;
use codex_protocol::ThreadId;
use futures::future::BoxFuture;
use serde_json::Value;

/// Supplies one MCP request metadata value for a live tool invocation. An error
/// fails the tool call before dispatch and is reported to the tool caller.
pub trait McpToolRequestMetaProvider: Send + Sync {
    fn value<'a>(
        &'a self,
        thread_id: ThreadId,
        turn_id: &'a str,
        call_id: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Value>>;
}

/// Install in thread extension data to add a private `_meta` field to one remote
/// tool on the selected local MCP registration. Bind the source and full transport
/// supplied by the trusted host, including routing headers for a proxied endpoint.
pub struct McpToolRequestMetaHook {
    pub server_name: String,
    pub source: McpServerSource,
    pub transport: McpServerTransportConfig,
    pub tool_name: String,
    /// A metadata key owned by the provider. Existing metadata cannot be overwritten.
    pub key: String,
    pub provider: Arc<dyn McpToolRequestMetaProvider>,
}

impl McpToolRequestMetaHook {
    /// Adds the provider’s value only when the immutable call matches the selected registration.
    pub async fn apply(
        &self,
        call: &PreparedMcpCall,
        thread_id: ThreadId,
        turn_id: &str,
        call_id: &str,
        meta: &mut Option<Value>,
    ) -> anyhow::Result<()> {
        if call.server_name() == self.server_name
            && call.tool_info().tool.name.as_ref() == self.tool_name
            && call.server_environment_id() == DEFAULT_MCP_SERVER_ENVIRONMENT_ID
            && let Some(server) = call.config().mcp_server_catalog.server(call.server_name())
            && server.config().is_local_environment()
            && server.source() == &self.source
            && server.config().transport == self.transport
        {
            let meta = meta
                .as_mut()
                .and_then(Value::as_object_mut)
                .context("missing MCP request metadata object")?;
            anyhow::ensure!(
                !meta.contains_key(&self.key),
                "MCP request metadata already contains {}",
                self.key
            );
            let value = self.provider.value(thread_id, turn_id, call_id).await?;
            meta.insert(self.key.clone(), value);
        }
        Ok(())
    }
}
