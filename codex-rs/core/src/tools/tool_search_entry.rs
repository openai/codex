use crate::tools::flat_tool_name;
use codex_mcp::ToolInfo;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_tool_api::ToolDefinition;
use codex_tools::LoadableToolSpec;
use codex_tools::tool_definition_to_loadable_tool_spec;

#[derive(Clone)]
pub(crate) struct ToolSearchEntry {
    pub(crate) search_text: String,
    pub(crate) output: LoadableToolSpec,
    pub(crate) limit_bucket: Option<String>,
}

#[cfg(test)]
pub(crate) fn build_tool_search_entries(
    mcp_tools: Option<&[ToolInfo]>,
    dynamic_tools: &[DynamicToolSpec],
) -> Vec<ToolSearchEntry> {
    let mut entries = Vec::new();

    let mut mcp_tools = mcp_tools
        .map(|tools| tools.iter().collect::<Vec<_>>())
        .unwrap_or_default();
    mcp_tools.sort_by_key(|info| info.canonical_tool_name());
    for info in mcp_tools {
        let definition =
            codex_tools::mcp_tool_definition(info.canonical_tool_name(), &info.tool).deferred();
        match mcp_tool_search_entry(info, &definition) {
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
    dynamic_tools.sort_by(|a, b| a.namespace.cmp(&b.namespace).then(a.name.cmp(&b.name)));
    for tool in dynamic_tools {
        let definition = codex_tools::parse_dynamic_tool(tool);
        match dynamic_tool_search_entry(tool, &definition) {
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

pub(crate) fn mcp_tool_search_entry<R>(
    info: &ToolInfo,
    definition: &ToolDefinition<R>,
) -> Result<ToolSearchEntry, serde_json::Error> {
    Ok(ToolSearchEntry {
        search_text: build_mcp_search_text(info),
        output: tool_definition_to_loadable_tool_spec(
            definition,
            mcp_tool_search_namespace_description(info),
        )?,
        limit_bucket: Some(info.server_name.clone()),
    })
}

pub(crate) fn dynamic_tool_search_entry<R>(
    tool: &DynamicToolSpec,
    definition: &ToolDefinition<R>,
) -> Result<ToolSearchEntry, serde_json::Error> {
    Ok(ToolSearchEntry {
        search_text: build_dynamic_search_text(tool),
        output: tool_definition_to_loadable_tool_spec(
            definition, /*namespace_description*/ None,
        )?,
        limit_bucket: None,
    })
}

fn mcp_tool_search_namespace_description(info: &ToolInfo) -> Option<String> {
    info.namespace_description
        .as_deref()
        .map(str::trim)
        .filter(|description| !description.is_empty())
        .map(str::to_string)
        .or_else(|| {
            info.connector_name
                .as_deref()
                .map(str::trim)
                .filter(|connector_name| !connector_name.is_empty())
                .map(|connector_name| format!("Tools for working with {connector_name}."))
        })
}

fn build_mcp_search_text(info: &ToolInfo) -> String {
    let tool_name = info.canonical_tool_name();
    let mut parts = vec![
        flat_tool_name(&tool_name).into_owned(),
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

    if let Some(description) = info.namespace_description.as_deref()
        && !description.trim().is_empty()
    {
        parts.push(description.to_string());
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

    if let Some(namespace) = &tool.namespace {
        parts.push(namespace.clone());
    }

    parts.extend(
        tool.input_schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .map(|map| map.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default(),
    );

    parts.join(" ")
}
