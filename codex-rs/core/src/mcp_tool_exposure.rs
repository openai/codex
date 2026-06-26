use codex_mcp::ToolInfo as McpToolInfo;
use codex_mcp::tool_is_model_visible;
use tracing::instrument;

pub(crate) struct McpToolExposure {
    pub(crate) direct_tools: Vec<McpToolInfo>,
    pub(crate) deferred_tools: Option<Vec<McpToolInfo>>,
}

#[instrument(level = "trace", skip_all)]
pub(crate) fn build_mcp_tool_exposure(
    all_mcp_tools: &[McpToolInfo],
    search_tool_enabled: bool,
) -> McpToolExposure {
    let deferred_tools = all_mcp_tools
        .iter()
        .filter(|tool| tool_is_model_visible(tool))
        .cloned()
        .collect::<Vec<_>>();

    if !search_tool_enabled {
        return McpToolExposure {
            direct_tools: deferred_tools,
            deferred_tools: None,
        };
    }

    McpToolExposure {
        direct_tools: Vec::new(),
        deferred_tools: (!deferred_tools.is_empty()).then_some(deferred_tools),
    }
}

#[cfg(test)]
#[path = "mcp_tool_exposure_test.rs"]
mod tests;
