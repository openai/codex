use codex_mcp::ToolInfo;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_tools::ToolSearchOutputTool;
use codex_tools::ToolSearchResultSource;
use codex_tools::dynamic_tool_to_responses_api_tool;
use codex_tools::tool_search_result_source_to_output_tool;
use std::collections::HashMap;

#[derive(Clone)]
pub(crate) struct ToolSearchEntry {
    pub(crate) search_text: String,
    pub(crate) output: ToolSearchOutputTool,
    pub(crate) limit_bucket: Option<String>,
}

pub(crate) fn build_tool_search_entries(
    mcp_tools: Option<&HashMap<String, ToolInfo>>,
    dynamic_tools: &[DynamicToolSpec],
) -> Vec<ToolSearchEntry> {
    let mut entries = Vec::new();

    let mut mcp_tools = mcp_tools
        .map(|tools| tools.values().collect::<Vec<_>>())
        .unwrap_or_default();
    mcp_tools.sort_by_key(|info| info.canonical_tool_name().display());
    for info in mcp_tools {
        match mcp_tool_search_entry(info) {
            Ok(entry) => entries.push(entry),
            Err(error) => {
                let tool_name = info.canonical_tool_name();
                tracing::error!(
                    "Failed to convert deferred MCP tool `{tool_name}` to OpenAI tool: {error:?}"
                );
            }
        }
    }

    let mut dynamic_tools = dynamic_tools.iter().collect::<Vec<_>>();
    dynamic_tools.sort_by(|a, b| a.name.cmp(&b.name));
    for tool in dynamic_tools {
        match dynamic_tool_search_entry(tool) {
            Ok(entry) => entries.push(entry),
            Err(error) => {
                tracing::error!(
                    "Failed to convert deferred dynamic tool {:?} to OpenAI tool: {error:?}",
                    tool.name
                );
            }
        }
    }

    entries
}

fn mcp_tool_search_entry(info: &ToolInfo) -> Result<ToolSearchEntry, serde_json::Error> {
    Ok(ToolSearchEntry {
        search_text: build_mcp_search_text(info),
        output: tool_search_result_source_to_output_tool(ToolSearchResultSource {
            server_name: info.server_name.as_str(),
            tool_namespace: info.callable_namespace.as_str(),
            tool_name: info.callable_name.as_str(),
            tool: &info.tool,
            connector_name: info.connector_name.as_deref(),
            connector_description: info.connector_description.as_deref(),
        })?,
        limit_bucket: Some(info.server_name.clone()),
    })
}

fn dynamic_tool_search_entry(tool: &DynamicToolSpec) -> Result<ToolSearchEntry, serde_json::Error> {
    Ok(ToolSearchEntry {
        search_text: build_dynamic_search_text(tool),
        output: ToolSearchOutputTool::Function(dynamic_tool_to_responses_api_tool(tool)?),
        limit_bucket: None,
    })
}

fn build_mcp_search_text(info: &ToolInfo) -> String {
    let mut parts = vec![
        info.canonical_tool_name().display(),
        info.callable_name.clone(),
        info.tool.name.to_string(),
        info.server_name.clone(),
    ];

    if let Some(title) = info.tool.title.as_deref()
        && !title.trim().is_empty()
    {
        parts.push(title.to_string());
    }

    if let Some(description) = info.tool.description.as_deref()
        && !description.trim().is_empty()
    {
        parts.push(description.to_string());
    }

    if let Some(connector_name) = info.connector_name.as_deref()
        && !connector_name.trim().is_empty()
    {
        parts.push(connector_name.to_string());
    }

    if let Some(connector_description) = info.connector_description.as_deref()
        && !connector_description.trim().is_empty()
    {
        parts.push(connector_description.to_string());
    }

    parts.extend(
        info.plugin_display_names
            .iter()
            .map(String::as_str)
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string),
    );

    parts.extend(
        info.tool
            .input_schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .map(|map| map.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default(),
    );

    parts.join(" ")
}

fn build_dynamic_search_text(tool: &DynamicToolSpec) -> String {
    let mut parts = vec![
        tool.name.clone(),
        tool.name.replace('_', " "),
        tool.description.clone(),
    ];

    parts.extend(
        tool.input_schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .map(|map| map.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default(),
    );

    parts.join(" ")
}
