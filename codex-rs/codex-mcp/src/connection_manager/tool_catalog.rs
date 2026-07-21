use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use codex_connectors::ConnectorRuntimeFetchSource;
use tracing::Instrument;
use tracing::instrument;
use tracing::trace;
use tracing::trace_span;

use super::McpConnectionManager;
use crate::codex_apps::prepare_openai_file_params_for_model;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::rmcp_client::CODEX_APPS_REFRESH_DURATION_METRIC;
use crate::rmcp_client::MCP_TOOLS_LIST_DURATION_METRIC;
use crate::rmcp_client::list_tools_for_client_uncached;
use crate::runtime::emit_duration;
use crate::tools::ToolInfo;
use crate::tools::filter_tools;
use crate::tools::normalize_tools_for_model_with_prefix;

impl McpConnectionManager {
    /// Returns all tools with model-visible names normalized.
    #[instrument(level = "trace", skip_all, fields(mcp_server_count = self.clients.len()))]
    pub async fn list_all_tools(&self) -> Vec<ToolInfo> {
        let mut tools = Vec::new();
        let mut available_server_count = 0;
        let mut unavailable_server_count = 0;
        for (server_name, managed_client) in &self.clients {
            managed_client.reconnect_failed_startup().await;
            let has_cached_tools = managed_client.has_cached_tools();
            let startup_complete = managed_client
                .startup_complete
                .load(std::sync::atomic::Ordering::Acquire);
            let Some(server_tools) = managed_client
                .listed_tools()
                .instrument(trace_span!(
                    "list_tools_for_server",
                    server_name = %server_name,
                    has_cached_tools,
                    startup_complete
                ))
                .await
            else {
                unavailable_server_count += 1;
                trace!(
                    server_name = %server_name,
                    has_cached_tools,
                    startup_complete,
                    "MCP server tools unavailable while building tool list"
                );
                continue;
            };
            available_server_count += 1;
            tools.extend(
                server_tools
                    .into_iter()
                    .map(|tool| self.with_server_metadata(tool)),
            );
        }
        let tools = normalize_tools_for_model_with_prefix(tools, self.prefix_mcp_tool_names);
        trace!(
            available_server_count,
            unavailable_server_count,
            tool_count = tools.len(),
            "built MCP tool list"
        );
        tools
    }

    /// Returns one tool from the current live connection.
    pub async fn tool_info(&self, server: &str, tool: &str) -> Option<ToolInfo> {
        let client = self.clients.get(server)?;
        let managed_client = client.client().await.ok()?;
        let tool = client
            .prepare_tools(managed_client.listed_tools())
            .into_iter()
            .find(|tool_info| tool_info.tool.name == tool)?;
        Some(self.with_server_metadata(tool))
    }

    /// Force-refresh codex apps tools by bypassing the in-process cache.
    ///
    /// On success, the refreshed tools replace shared cache contents when the
    /// cache is enabled and the latest filtered tools are returned directly to
    /// the caller. On failure, existing shared cache contents remain unchanged.
    pub async fn hard_refresh_codex_apps_tools_cache(&self) -> Result<Vec<ToolInfo>> {
        let refresh_start = Instant::now();
        let managed_client = self
            .clients
            .get(CODEX_APPS_MCP_SERVER_NAME)
            .ok_or_else(|| anyhow!("unknown MCP server '{CODEX_APPS_MCP_SERVER_NAME}'"))?
            .client()
            .await
            .context("failed to get client")?;

        let list_start = Instant::now();
        let fetch_ticket =
            managed_client
                .codex_apps_tools_cache_context
                .as_ref()
                .map(|cache_context| {
                    cache_context.begin_fetch(ConnectorRuntimeFetchSource::HardRefresh)
                });
        let tools = list_tools_for_client_uncached(
            CODEX_APPS_MCP_SERVER_NAME,
            /*is_codex_apps_mcp_server*/ true,
            /*codex_apps_refresh_trigger*/ "explicit",
            &managed_client.client,
            managed_client.tool_timeout,
            managed_client.server_instructions.as_deref(),
        )
        .await
        .with_context(|| {
            format!("failed to refresh tools for MCP server '{CODEX_APPS_MCP_SERVER_NAME}'")
        })?;

        let tools =
            match (
                managed_client.codex_apps_tools_cache_context.as_ref(),
                fetch_ticket,
            ) {
                (Some(cache_context), Some(fetch_ticket)) => cache_context
                    .publish_if_newest_accepted(fetch_ticket, &managed_client.server_info, tools),
                (None, None) => tools,
                _ => unreachable!("Codex Apps fetch ticket requires cache context"),
            };
        emit_duration(
            MCP_TOOLS_LIST_DURATION_METRIC,
            list_start.elapsed(),
            &[("cache", "miss")],
        );
        let tools = filter_tools(tools, &managed_client.tool_filter)
            .into_iter()
            .map(|mut tool| {
                prepare_openai_file_params_for_model(&mut tool);
                self.with_server_metadata(tool)
            });
        let tools = normalize_tools_for_model_with_prefix(tools, self.prefix_mcp_tool_names);
        emit_duration(
            CODEX_APPS_REFRESH_DURATION_METRIC,
            refresh_start.elapsed(),
            &[("path", "legacy"), ("trigger", "explicit")],
        );
        Ok(tools)
    }

    fn with_server_metadata(&self, mut tool: ToolInfo) -> ToolInfo {
        let Some(metadata) = self.server_metadata.get(&tool.server_name) else {
            tool.supports_parallel_tool_calls = false;
            tool.server_origin = None;
            return tool;
        };

        tool.supports_parallel_tool_calls = metadata.supports_parallel_tool_calls;
        tool.server_origin = metadata
            .origin
            .as_ref()
            .map(|origin| origin.as_str().to_string());
        tool
    }
}
