use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_tools::ToolDefinition;
use codex_tools::ToolSearchOutputTool;
use codex_tools::dynamic_tool_to_responses_api_tool;
use codex_tools::tool_definition_to_tool_search_output_tool;

#[derive(Clone)]
pub(crate) struct ToolSearchEntry {
    pub(crate) search_text: String,
    pub(crate) output: ToolSearchOutputTool,
    pub(crate) limit_bucket: Option<String>,
}

pub(crate) fn build_tool_search_entries(
    mcp_tools: Option<&[ToolDefinition]>,
    dynamic_tools: &[DynamicToolSpec],
) -> Vec<ToolSearchEntry> {
    let mut entries = Vec::new();

    let mut mcp_tools = mcp_tools
        .map(|tools| tools.iter().collect::<Vec<_>>())
        .unwrap_or_default();
    mcp_tools.sort_by_key(|tool| tool.name.display());
    for tool in mcp_tools {
        entries.push(tool_definition_search_entry(tool));
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

fn tool_definition_search_entry(tool: &ToolDefinition) -> ToolSearchEntry {
    ToolSearchEntry {
        search_text: build_tool_definition_search_text(tool),
        output: tool_definition_to_tool_search_output_tool(tool),
        limit_bucket: tool
            .search
            .as_ref()
            .and_then(|metadata| metadata.limit_bucket.clone()),
    }
}

fn dynamic_tool_search_entry(tool: &DynamicToolSpec) -> Result<ToolSearchEntry, serde_json::Error> {
    Ok(ToolSearchEntry {
        search_text: build_dynamic_search_text(tool),
        output: ToolSearchOutputTool::Function(dynamic_tool_to_responses_api_tool(tool)?),
        limit_bucket: None,
    })
}

fn build_tool_definition_search_text(tool: &ToolDefinition) -> String {
    let mut parts = vec![
        tool.name.display(),
        tool.name.name.clone(),
        tool.description.clone(),
    ];

    if let Some(search) = tool.search.as_ref() {
        parts.push(search.source_name.clone());
        if let Some(source_description) = search.source_description.as_ref() {
            parts.push(source_description.clone());
        }
        parts.extend(search.extra_terms.clone());
    }

    parts.extend(
        tool.input_schema
            .properties
            .as_ref()
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
