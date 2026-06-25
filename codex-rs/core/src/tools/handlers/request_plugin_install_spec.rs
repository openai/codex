use std::collections::BTreeMap;

use codex_tools::JsonSchema;
use codex_tools::LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME;
use codex_tools::REQUEST_PLUGIN_INSTALL_TOOL_NAME;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;

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
                        "Type of discoverable tool to suggest. Use \"plugin\".".to_string(),
                    )),
                ),
                (
                    "action_type".to_string(),
                    JsonSchema::string(Some(
                        "Suggested action for the plugin. Use \"install\".".to_string(),
                    )),
                ),
                (
                    "tool_id".to_string(),
                    JsonSchema::string(Some("Plugin id to suggest.".to_string())),
                ),
                (
                    "suggest_reason".to_string(),
                    JsonSchema::string(Some(
                        "Concise one-line user-facing reason why this plugin can help with the current request."
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
                "# Request plugin install\n\nUse this tool only after `{LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME}` returns a plugin that exactly matches the user's explicit request.\n\nDo not use it for adjacent capabilities, broad recommendations, or plugins that merely seem useful. Pass the returned `tool_type` through directly, and pass the returned `id` as `tool_id`.\n\nIMPORTANT: DO NOT call this tool in parallel with other tools."
            ),
        ),
        ToolSuggestPresentation::RecommendationContext => (
            BTreeMap::from([
                (
                    "plugin_id".to_string(),
                    JsonSchema::string(Some(
                        "Plugin id from the `<recommended_plugins>` list.".to_string(),
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
            "# Suggest a recommended plugin installation\n\nSuggest installing a plugin from the `<recommended_plugins>` list when it would help with the user's current request. Briefly explain why in `suggest_reason`.".to_string(),
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
    use pretty_assertions::assert_eq;

    #[test]
    fn uses_recommended_plugin_wire_shape() {
        let ToolSpec::Function(spec) =
            create_request_plugin_install_tool(ToolSuggestPresentation::RecommendationContext)
        else {
            panic!("expected function tool");
        };
        assert_eq!(spec.name, REQUEST_PLUGIN_INSTALL_TOOL_NAME);
        let properties = spec.parameters.properties.expect("object properties");
        assert_eq!(
            properties.keys().cloned().collect::<Vec<_>>(),
            vec!["plugin_id".to_string(), "suggest_reason".to_string()]
        );
    }

    #[test]
    fn uses_legacy_plugin_wire_shape() {
        let ToolSpec::Function(spec) =
            create_request_plugin_install_tool(ToolSuggestPresentation::ListTool)
        else {
            panic!("expected function tool");
        };
        assert_eq!(spec.name, REQUEST_PLUGIN_INSTALL_TOOL_NAME);
        assert!(
            spec.description
                .contains(LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME)
        );
        let properties = spec.parameters.properties.expect("object properties");
        assert_eq!(
            properties.keys().cloned().collect::<Vec<_>>(),
            vec![
                "action_type".to_string(),
                "suggest_reason".to_string(),
                "tool_id".to_string(),
                "tool_type".to_string(),
            ]
        );
    }
}
