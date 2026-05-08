use codex_tools::DiscoverableToolType;
use codex_tools::JsonSchema;
use codex_tools::REQUEST_PLUGIN_INSTALL_TOOL_NAME;
use codex_tools::RequestPluginInstallEntry;
use codex_tools::ResponsesApiTool;
use codex_tools::TOOL_SEARCH_TOOL_NAME;
use codex_tools::ToolSpec;
use std::collections::BTreeMap;

pub(crate) fn create_request_plugin_install_tool(
    discoverable_tools: &[RequestPluginInstallEntry],
) -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "tool_type".to_string(),
            JsonSchema::string(Some(
                "Type of discoverable tool to suggest. Use \"connector\" or \"plugin\"."
                    .to_string(),
            )),
        ),
        (
            "action_type".to_string(),
            JsonSchema::string(Some("Suggested action for the tool. Use \"install\".".to_string())),
        ),
        (
            "tool_id".to_string(),
            JsonSchema::string(Some("Connector or plugin id to suggest.".to_string())),
        ),
        (
            "suggest_reason".to_string(),
            JsonSchema::string(Some(
                "Concise one-line user-facing reason why this plugin or connector can help with the current request."
                    .to_string(),
            )),
        ),
    ]);

    let discoverable_tools = format_discoverable_tools(discoverable_tools);
    let description = format!(
        "# Request plugin/connector install\n\nUse this tool only to ask the user to install one known plugin or connector from the list below. The list contains known candidates that are not currently installed.\n\nUse this ONLY when all of the following are true:\n- The user explicitly asks to use a specific plugin or connector that is not already available in the current context or active `tools` list.\n- `{TOOL_SEARCH_TOOL_NAME}` is not available, or it has already been called and did not find or make the requested tool callable.\n- The plugin or connector is one of the known installable plugins or connectors listed below. Only ask to install plugins or connectors from this list.\n\nDo not use this tool for adjacent capabilities, broad recommendations, or tools that merely seem useful. Only use when the user explicitly asks to use that exact listed plugin or connector.\n\nKnown plugins/connectors available to install:\n{discoverable_tools}\n\nWorkflow:\n\n1. Check the current context and active `tools` list first. If current active tools aren't relevant and `{TOOL_SEARCH_TOOL_NAME}` is available, only call this tool after `{TOOL_SEARCH_TOOL_NAME}` has already been tried and found no relevant tool.\n2. Match the user's explicit request against the known plugin/connector list above. Only proceed when one listed plugin or connector exactly fits.\n3. If we found both connectors and plugins to install, use plugins first, only use connectors if the corresponding plugin is installed but the connector is not.\n4. If one plugin or connector clearly fits, call `{REQUEST_PLUGIN_INSTALL_TOOL_NAME}` with:\n   - `tool_type`: `connector` or `plugin`\n   - `action_type`: `install`\n   - `tool_id`: exact id from the known plugin/connector list above\n   - `suggest_reason`: concise one-line user-facing reason this plugin or connector can help with the current request\n5. After the request flow completes:\n   - if the user finished the install flow, continue by searching again or using the newly available plugin or connector\n   - if the user did not finish, continue without that plugin or connector, and don't request it again unless the user explicitly asks for it.\n\nIMPORTANT: DO NOT call this tool in parallel with other tools."
    );

    ToolSpec::Function(ResponsesApiTool {
        name: REQUEST_PLUGIN_INSTALL_TOOL_NAME.to_string(),
        description,
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            Some(vec![
                "tool_type".to_string(),
                "action_type".to_string(),
                "tool_id".to_string(),
                "suggest_reason".to_string(),
            ]),
            Some(false.into()),
        ),
        output_schema: None,
    })
}

fn format_discoverable_tools(discoverable_tools: &[RequestPluginInstallEntry]) -> String {
    let mut discoverable_tools = discoverable_tools.to_vec();
    discoverable_tools.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.id.cmp(&right.id))
    });

    discoverable_tools
        .into_iter()
        .map(|tool| {
            let description = tool_description_or_fallback(&tool);
            format!(
                "- {} (id: `{}`, type: {}, action: install): {}",
                tool.name,
                tool.id,
                discoverable_tool_type_str(tool.tool_type),
                description
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn tool_description_or_fallback(tool: &RequestPluginInstallEntry) -> String {
    if let Some(description) = tool
        .description
        .as_deref()
        .map(str::trim)
        .filter(|description| !description.is_empty())
    {
        return description.to_string();
    }

    match tool.tool_type {
        DiscoverableToolType::Connector => "No description provided.".to_string(),
        DiscoverableToolType::Plugin => plugin_summary(tool),
    }
}

fn plugin_summary(tool: &RequestPluginInstallEntry) -> String {
    let mut capabilities = Vec::new();
    if tool.has_skills {
        capabilities.push("skills".to_string());
    }
    if !tool.mcp_server_names.is_empty() {
        capabilities.push(format!("MCP servers: {}", tool.mcp_server_names.join(", ")));
    }
    if !tool.app_connector_ids.is_empty() {
        capabilities.push(format!(
            "app connectors: {}",
            tool.app_connector_ids.join(", ")
        ));
    }
    if capabilities.is_empty() {
        "No description provided.".to_string()
    } else {
        capabilities.join("; ")
    }
}

fn discoverable_tool_type_str(tool_type: DiscoverableToolType) -> &'static str {
    match tool_type {
        DiscoverableToolType::Connector => "connector",
        DiscoverableToolType::Plugin => "plugin",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_tools::JsonSchema;
    use pretty_assertions::assert_eq;
    use std::collections::BTreeMap;

    #[test]
    fn create_request_plugin_install_tool_uses_plugin_summary_fallback() {
        let expected_description = concat!(
            "# Request plugin/connector install\n\n",
            "Use this tool only to ask the user to install one known plugin or connector from the list below. The list contains known candidates that are not currently installed.\n\n",
            "Use this ONLY when all of the following are true:\n",
            "- The user explicitly asks to use a specific plugin or connector that is not already available in the current context or active `tools` list.\n",
            "- `tool_search` is not available, or it has already been called and did not find or make the requested tool callable.\n",
            "- The plugin or connector is one of the known installable plugins or connectors listed below. Only ask to install plugins or connectors from this list.\n\n",
            "Do not use this tool for adjacent capabilities, broad recommendations, or tools that merely seem useful. Only use when the user explicitly asks to use that exact listed plugin or connector.\n\n",
            "Known plugins/connectors available to install:\n",
            "- GitHub (id: `github`, type: plugin, action: install): skills; MCP servers: github-mcp; app connectors: github-app\n",
            "- Slack (id: `slack@openai-curated`, type: connector, action: install): No description provided.\n\n",
            "Workflow:\n\n",
            "1. Check the current context and active `tools` list first. If current active tools aren't relevant and `tool_search` is available, only call this tool after `tool_search` has already been tried and found no relevant tool.\n",
            "2. Match the user's explicit request against the known plugin/connector list above. Only proceed when one listed plugin or connector exactly fits.\n",
            "3. If we found both connectors and plugins to install, use plugins first, only use connectors if the corresponding plugin is installed but the connector is not.\n",
            "4. If one plugin or connector clearly fits, call `request_plugin_install` with:\n",
            "   - `tool_type`: `connector` or `plugin`\n",
            "   - `action_type`: `install`\n",
            "   - `tool_id`: exact id from the known plugin/connector list above\n",
            "   - `suggest_reason`: concise one-line user-facing reason this plugin or connector can help with the current request\n",
            "5. After the request flow completes:\n",
            "   - if the user finished the install flow, continue by searching again or using the newly available plugin or connector\n",
            "   - if the user did not finish, continue without that plugin or connector, and don't request it again unless the user explicitly asks for it.\n\n",
            "IMPORTANT: DO NOT call this tool in parallel with other tools.",
        );

        assert_eq!(
            create_request_plugin_install_tool(&[
                RequestPluginInstallEntry {
                    id: "slack@openai-curated".to_string(),
                    name: "Slack".to_string(),
                    description: None,
                    tool_type: DiscoverableToolType::Connector,
                    has_skills: false,
                    mcp_server_names: Vec::new(),
                    app_connector_ids: Vec::new(),
                },
                RequestPluginInstallEntry {
                    id: "github".to_string(),
                    name: "GitHub".to_string(),
                    description: None,
                    tool_type: DiscoverableToolType::Plugin,
                    has_skills: true,
                    mcp_server_names: vec!["github-mcp".to_string()],
                    app_connector_ids: vec!["github-app".to_string()],
                },
            ]),
            ToolSpec::Function(ResponsesApiTool {
                name: "request_plugin_install".to_string(),
                description: expected_description.to_string(),
                strict: false,
                defer_loading: None,
                parameters: JsonSchema::object(BTreeMap::from([
                        (
                            "action_type".to_string(),
                            JsonSchema::string(Some(
                                    "Suggested action for the tool. Use \"install\"."
                                        .to_string(),
                                ),),
                        ),
                        (
                            "suggest_reason".to_string(),
                            JsonSchema::string(Some(
                                    "Concise one-line user-facing reason why this plugin or connector can help with the current request."
                                        .to_string(),
                                ),),
                        ),
                        (
                            "tool_id".to_string(),
                            JsonSchema::string(Some(
                                    "Connector or plugin id to suggest."
                                        .to_string(),
                                ),),
                        ),
                        (
                            "tool_type".to_string(),
                            JsonSchema::string(Some(
                                    "Type of discoverable tool to suggest. Use \"connector\" or \"plugin\"."
                                        .to_string(),
                                ),),
                        ),
                    ]), Some(vec![
                        "tool_type".to_string(),
                        "action_type".to_string(),
                        "tool_id".to_string(),
                        "suggest_reason".to_string(),
                    ]), Some(false.into())),
                output_schema: None,
            })
        );
    }
}
