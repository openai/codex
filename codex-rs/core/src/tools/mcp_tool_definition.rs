use codex_mcp::ToolInfo;
use codex_tools::ToolDefinition;
use codex_tools::ToolLoadingPolicy;
use codex_tools::ToolPresentation;
use codex_tools::ToolSearchMetadata;
use codex_tools::mcp_tool_to_tool_definition;

pub(crate) fn mcp_tool_info_to_tool_definition(
    info: &ToolInfo,
    loading: ToolLoadingPolicy,
) -> Result<ToolDefinition, serde_json::Error> {
    let mut definition = mcp_tool_to_tool_definition(&info.canonical_tool_name(), &info.tool)?;
    if loading.is_deferred() {
        definition = definition.into_deferred();
        definition.search = Some(mcp_tool_search_metadata(info));
    }
    definition.presentation = mcp_tool_presentation(info, loading);
    Ok(definition)
}

fn mcp_tool_presentation(info: &ToolInfo, loading: ToolLoadingPolicy) -> Option<ToolPresentation> {
    let namespace_description = match loading {
        ToolLoadingPolicy::Eager => non_empty(info.connector_description.as_deref())
            .or_else(|| non_empty(info.server_instructions.as_deref())),
        ToolLoadingPolicy::Deferred => {
            non_empty(info.connector_description.as_deref()).or_else(|| {
                non_empty(info.connector_name.as_deref())
                    .map(|name| format!("Tools for working with {name}."))
            })
        }
    };

    namespace_description.map(|namespace_description| ToolPresentation {
        namespace_display_name: None,
        namespace_description: Some(namespace_description),
    })
}

fn mcp_tool_search_metadata(info: &ToolInfo) -> ToolSearchMetadata {
    let source_name = non_empty(info.connector_name.as_deref())
        .unwrap_or_else(|| info.server_name.trim().to_string());
    let source_description = non_empty(info.connector_description.as_deref());
    let mut extra_terms = Vec::new();

    push_non_empty(&mut extra_terms, &info.callable_name);
    push_non_empty(&mut extra_terms, info.tool.name.as_ref());
    push_non_empty(&mut extra_terms, &info.server_name);
    if let Some(title) = info.tool.title.as_deref() {
        push_non_empty(&mut extra_terms, title);
    }
    if let Some(connector_name) = info.connector_name.as_deref() {
        push_non_empty(&mut extra_terms, connector_name);
    }
    if let Some(connector_description) = info.connector_description.as_deref() {
        push_non_empty(&mut extra_terms, connector_description);
    }
    for plugin_display_name in &info.plugin_display_names {
        push_non_empty(&mut extra_terms, plugin_display_name);
    }

    ToolSearchMetadata {
        source_name,
        source_description,
        extra_terms,
        limit_bucket: Some(info.server_name.clone()),
    }
}

fn push_non_empty(parts: &mut Vec<String>, value: &str) {
    if let Some(value) = non_empty(Some(value)) {
        parts.push(value);
    }
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
#[path = "mcp_tool_definition_tests.rs"]
mod tests;
