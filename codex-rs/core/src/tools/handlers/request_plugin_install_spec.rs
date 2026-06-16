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

    let description = match presentation {
        ToolSuggestPresentation::ListTool => format!(
            "# Request plugin/connector install\n\nUse this tool only after `{LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME}` returns a plugin or connector that exactly matches the user's explicit request.\n\nDo not use it for adjacent capabilities, broad recommendations, or tools that merely seem useful. Pass the returned `tool_type` through directly, and pass the returned `id` as `tool_id`.\n\nIMPORTANT: DO NOT call this tool in parallel with other tools."
        ),
        ToolSuggestPresentation::RecommendationContext =>
            "# Suggest a recommended plugin installation\n\nSuggest installing a plugin from the `<recommended_plugins>` list when it would help with the user's current request. Briefly explain why in `suggest_reason`.".to_string(),
    };

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
            "Use this tool only after `list_available_plugins_to_install` returns a plugin or connector that exactly matches the user's explicit request.\n\n",
            "Do not use it for adjacent capabilities, broad recommendations, or tools that merely seem useful. Pass the returned `tool_type` through directly, and pass the returned `id` as `tool_id`.\n\n",
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

    #[test]
    fn recommendation_context_changes_only_the_description() {
        let mut expected = create_request_plugin_install_tool(ToolSuggestPresentation::ListTool);
        let recommendations =
            create_request_plugin_install_tool(ToolSuggestPresentation::RecommendationContext);

        let ToolSpec::Function(expected_function) = &mut expected else {
            panic!("expected function tool specs");
        };
        expected_function.description = "# Suggest a recommended plugin installation\n\nSuggest installing a plugin from the `<recommended_plugins>` list when it would help with the user's current request. Briefly explain why in `suggest_reason`.".to_string();

        assert_eq!(recommendations, expected);
    }
}
