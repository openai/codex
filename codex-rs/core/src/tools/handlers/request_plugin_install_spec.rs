use codex_tools::JsonSchema;
use codex_tools::LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME;
use codex_tools::REQUEST_PLUGIN_INSTALL_TOOL_NAME;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use std::collections::BTreeMap;

use crate::tools::router::ToolSuggestPresentation;

pub(crate) fn create_request_plugin_install_tool(
    presentation: ToolSuggestPresentation,
) -> ToolSpec {
    let (properties, required, description) = match presentation {
        ToolSuggestPresentation::ListTool => (
            BTreeMap::from([
                (
                    "tool_type".to_string(),
                    JsonSchema::string(Some(
                        "Type of discoverable tool to suggest. Use \"connector\" or \"plugin\"."
                            .to_string(),
                    )),
                ),
                (
                    "action_type".to_string(),
                    JsonSchema::string(Some(
                        "Suggested action for the tool. Use \"install\".".to_string(),
                    )),
                ),
                (
                    "tool_id".to_string(),
                    JsonSchema::string(Some(
                        "Connector or plugin id to suggest. For a Plugin Management dependency, use the exact canonical_plugin_id."
                            .to_string(),
                    )),
                ),
                (
                    "suggest_reason".to_string(),
                    JsonSchema::string(Some(
                        "Concise one-line user-facing reason why this plugin or connector can help with the current request."
                            .to_string(),
                    )),
                ),
            ]),
            vec![
                "tool_type".to_string(),
                "action_type".to_string(),
                "tool_id".to_string(),
                "suggest_reason".to_string(),
            ],
            format!(
                "# Request plugin/connector install\n\nUse this tool only after `{LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME}` returns a plugin or connector that exactly matches the user's explicit request, or after Plugin Management `get_plugin_dependencies` returns an exact installable `canonical_plugin_id`.\n\nFor a dependency, use this tool only when `canonical_plugin_status=\"enabled\"`, `canonical_plugin_installation_policy=\"available\"`, and `canonical_plugin_installed=false`; pass `tool_type=\"plugin\"`, `action_type=\"install\"`, and the exact `canonical_plugin_id` as `tool_id`. Never invent IDs or use bare or fuzzy plugin names.\n\nDo not use it for adjacent capabilities, broad recommendations, or tools that merely seem useful. Pass list-tool results through directly.\n\nIMPORTANT: DO NOT call this tool in parallel with other tools."
            ),
        ),
        ToolSuggestPresentation::RecommendationContext => (
            BTreeMap::from([
                (
                    "plugin_id".to_string(),
                    JsonSchema::string(Some(
                        "Plugin id from the `<recommended_plugins>` list, or exact canonical_plugin_id returned by Plugin Management `get_plugin_dependencies`."
                            .to_string(),
                    )),
                ),
                (
                    "suggest_reason".to_string(),
                    JsonSchema::string(Some(
                        "Concise one-line user-facing reason why this plugin can help with the current request."
                            .to_string(),
                    )),
                ),
            ]),
            vec!["plugin_id".to_string(), "suggest_reason".to_string()],
            "# Suggest a plugin installation\n\nSuggest installing a plugin from the `<recommended_plugins>` list when it would help with the user's current request. You may also use an exact `canonical_plugin_id` returned by Plugin Management `get_plugin_dependencies` when `canonical_plugin_status=\"enabled\"`, `canonical_plugin_installation_policy=\"available\"`, and `canonical_plugin_installed=false`. Never invent IDs or use bare or fuzzy plugin names. Briefly explain why in `suggest_reason`.".to_string(),
        ),
    };

    ToolSpec::Function(ResponsesApiTool {
        name: REQUEST_PLUGIN_INSTALL_TOOL_NAME.to_string(),
        description,
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(properties, Some(required), Some(false.into())),
        output_schema: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_tools::JsonSchema;
    use pretty_assertions::assert_eq;
    use std::collections::BTreeMap;

    #[test]
    fn create_request_plugin_install_tool_uses_expected_legacy_wire_shape() {
        let expected_description = concat!(
            "# Request plugin/connector install\n\n",
            "Use this tool only after `list_available_plugins_to_install` returns a plugin or connector that exactly matches the user's explicit request, or after Plugin Management `get_plugin_dependencies` returns an exact installable `canonical_plugin_id`.\n\n",
            "For a dependency, use this tool only when `canonical_plugin_status=\"enabled\"`, `canonical_plugin_installation_policy=\"available\"`, and `canonical_plugin_installed=false`; pass `tool_type=\"plugin\"`, `action_type=\"install\"`, and the exact `canonical_plugin_id` as `tool_id`. Never invent IDs or use bare or fuzzy plugin names.\n\n",
            "Do not use it for adjacent capabilities, broad recommendations, or tools that merely seem useful. Pass list-tool results through directly.\n\n",
            "IMPORTANT: DO NOT call this tool in parallel with other tools.",
        );

        assert_eq!(
            create_request_plugin_install_tool(ToolSuggestPresentation::ListTool),
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
                                    "Connector or plugin id to suggest. For a Plugin Management dependency, use the exact canonical_plugin_id."
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

    #[test]
    fn recommendation_context_uses_simplified_plugin_wire_shape() {
        assert_eq!(
            create_request_plugin_install_tool(ToolSuggestPresentation::RecommendationContext),
            ToolSpec::Function(ResponsesApiTool {
                name: "request_plugin_install".to_string(),
                description: "# Suggest a plugin installation\n\nSuggest installing a plugin from the `<recommended_plugins>` list when it would help with the user's current request. You may also use an exact `canonical_plugin_id` returned by Plugin Management `get_plugin_dependencies` when `canonical_plugin_status=\"enabled\"`, `canonical_plugin_installation_policy=\"available\"`, and `canonical_plugin_installed=false`. Never invent IDs or use bare or fuzzy plugin names. Briefly explain why in `suggest_reason`.".to_string(),
                strict: false,
                defer_loading: None,
                parameters: JsonSchema::object(
                    BTreeMap::from([
                        (
                            "plugin_id".to_string(),
                            JsonSchema::string(Some(
                                "Plugin id from the `<recommended_plugins>` list, or exact canonical_plugin_id returned by Plugin Management `get_plugin_dependencies`.".to_string(),
                            )),
                        ),
                        (
                            "suggest_reason".to_string(),
                            JsonSchema::string(Some(
                                "Concise one-line user-facing reason why this plugin can help with the current request."
                                    .to_string(),
                            )),
                        ),
                    ]),
                    Some(vec!["plugin_id".to_string(), "suggest_reason".to_string()]),
                    Some(false.into()),
                ),
                output_schema: None,
            })
        );
    }
}
