use codex_tools::JsonSchema;
use codex_tools::LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME;
use codex_tools::REQUEST_PLUGIN_INSTALL_TOOL_NAME;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use serde_json::json;
use std::collections::BTreeMap;

use crate::tools::router::ToolSuggestPresentation;

pub(crate) fn create_request_plugin_install_tool(
    presentation: ToolSuggestPresentation,
) -> ToolSpec {
    let description = match presentation {
        ToolSuggestPresentation::ListTool => format!(
            "# Request plugin/connector install\n\nUse this tool only after `{LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME}` returns one or more plugins or connectors that exactly match the user's explicit request.\n\nDo not use it for adjacent capabilities, broad recommendations, or tools that merely seem useful. For a single target, pass the returned `tool_type` through directly and pass the returned `id` as `tool_id`. For multiple exact targets, make one call with `entries` for a flat list or `categories` when alternatives are organized by category; every entry's `tool_id` must be an exact `id` returned by `{LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME}`.\n\nIMPORTANT: DO NOT call this tool in parallel with other tools."
        ),
        ToolSuggestPresentation::RecommendationContext =>
            "# Suggest a recommended plugin installation\n\nSuggest installing a plugin from the `<recommended_plugins>` list when it would help with the user's current request. Briefly explain why in `suggest_reason`.".to_string(),
    };

    ToolSpec::Function(ResponsesApiTool {
        name: REQUEST_PLUGIN_INSTALL_TOOL_NAME.to_string(),
        description,
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::one_of(
            vec![
                single_target_schema(),
                flat_picker_schema(),
                categorized_picker_schema(),
            ],
            Some(
                "Use the single-target shape for one install card, the flat picker shape for a list, or the categorized picker shape for grouped exact install candidates."
                    .to_string(),
            ),
        ),
        output_schema: None,
    })
}

fn single_target_schema() -> JsonSchema {
    JsonSchema::object(
        BTreeMap::from([
            (
                "tool_type".to_string(),
                tool_type_schema("Type of discoverable tool to suggest.".to_string()),
            ),
            ("action_type".to_string(), install_action_schema()),
            (
                "tool_id".to_string(),
                JsonSchema::string(Some("Connector or plugin id to suggest.".to_string())),
            ),
            ("suggest_reason".to_string(), suggest_reason_schema()),
        ]),
        Some(vec![
            "tool_type".to_string(),
            "action_type".to_string(),
            "tool_id".to_string(),
            "suggest_reason".to_string(),
        ]),
        Some(false.into()),
    )
}

fn flat_picker_schema() -> JsonSchema {
    JsonSchema::object(
        BTreeMap::from([
            ("action_type".to_string(), install_action_schema()),
            ("suggest_reason".to_string(), suggest_reason_schema()),
            (
                "title".to_string(),
                JsonSchema::string(Some(
                    "Optional title for the flat multi-tool install picker.".to_string(),
                )),
            ),
            (
                "entries".to_string(),
                JsonSchema::array(
                    picker_entry_schema(),
                    Some("Flat list of exact install candidates.".to_string()),
                ),
            ),
        ]),
        Some(vec![
            "action_type".to_string(),
            "suggest_reason".to_string(),
            "entries".to_string(),
        ]),
        Some(false.into()),
    )
}

fn categorized_picker_schema() -> JsonSchema {
    JsonSchema::object(
        BTreeMap::from([
            ("action_type".to_string(), install_action_schema()),
            ("suggest_reason".to_string(), suggest_reason_schema()),
            (
                "title".to_string(),
                JsonSchema::string(Some(
                    "Optional title for the categorized multi-tool install picker.".to_string(),
                )),
            ),
            (
                "categories".to_string(),
                JsonSchema::array(
                    picker_category_schema(),
                    Some("Grouped exact install candidates.".to_string()),
                ),
            ),
        ]),
        Some(vec![
            "action_type".to_string(),
            "suggest_reason".to_string(),
            "categories".to_string(),
        ]),
        Some(false.into()),
    )
}

fn picker_entry_schema() -> JsonSchema {
    JsonSchema::object(
        BTreeMap::from([
            (
                "id".to_string(),
                JsonSchema::string(Some(
                    "Stable entry id for this picker row. Use a concise unique id.".to_string(),
                )),
            ),
            (
                "tool_id".to_string(),
                JsonSchema::string(Some(
                    "Exact connector or plugin id returned by list_available_plugins_to_install."
                        .to_string(),
                )),
            ),
            (
                "tool_name".to_string(),
                JsonSchema::string(Some(
                    "Display name returned by list_available_plugins_to_install.".to_string(),
                )),
            ),
            (
                "tool_type".to_string(),
                tool_type_schema("Type returned by list_available_plugins_to_install.".to_string()),
            ),
            (
                "description".to_string(),
                JsonSchema::string(Some(
                    "Optional short picker-row description for this exact candidate.".to_string(),
                )),
            ),
        ]),
        Some(vec![
            "id".to_string(),
            "tool_id".to_string(),
            "tool_name".to_string(),
            "tool_type".to_string(),
        ]),
        Some(false.into()),
    )
}

fn picker_category_schema() -> JsonSchema {
    JsonSchema::object(
        BTreeMap::from([
            (
                "id".to_string(),
                JsonSchema::string(Some(
                    "Stable category id for matching picker responses.".to_string(),
                )),
            ),
            (
                "title".to_string(),
                JsonSchema::string(Some("User-facing category title.".to_string())),
            ),
            (
                "required".to_string(),
                JsonSchema::boolean(Some(
                    "Whether the user must install enough entries in this category before continuing."
                        .to_string(),
                )),
            ),
            (
                "min_installed".to_string(),
                JsonSchema::integer(Some(
                    "Minimum ready entries required when this category is required.".to_string(),
                )),
            ),
            (
                "entries".to_string(),
                JsonSchema::array(
                    picker_entry_schema(),
                    Some("Install candidates in this category.".to_string()),
                ),
            ),
        ]),
        Some(vec![
            "id".to_string(),
            "title".to_string(),
            "entries".to_string(),
        ]),
        Some(false.into()),
    )
}

fn tool_type_schema(description: String) -> JsonSchema {
    JsonSchema::string_enum(vec![json!("connector"), json!("plugin")], Some(description))
}

fn install_action_schema() -> JsonSchema {
    JsonSchema::string_enum(
        vec![json!("install")],
        Some("Suggested action for the tool. Use \"install\".".to_string()),
    )
}

fn suggest_reason_schema() -> JsonSchema {
    JsonSchema::string(Some(
        "Concise one-line user-facing reason why this plugin or connector can help with the current request."
            .to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_tools::JsonSchema;
    use pretty_assertions::assert_eq;

    #[test]
    fn create_request_plugin_install_tool_uses_expected_legacy_wire_shape() {
        let expected_description = concat!(
            "# Request plugin/connector install\n\n",
            "Use this tool only after `list_available_plugins_to_install` returns one or more plugins or connectors that exactly match the user's explicit request.\n\n",
            "Do not use it for adjacent capabilities, broad recommendations, or tools that merely seem useful. For a single target, pass the returned `tool_type` through directly and pass the returned `id` as `tool_id`. For multiple exact targets, make one call with `entries` for a flat list or `categories` when alternatives are organized by category; every entry's `tool_id` must be an exact `id` returned by `list_available_plugins_to_install`.\n\n",
            "IMPORTANT: DO NOT call this tool in parallel with other tools.",
        );

        assert_eq!(
            create_request_plugin_install_tool(ToolSuggestPresentation::ListTool),
            ToolSpec::Function(ResponsesApiTool {
                name: "request_plugin_install".to_string(),
                description: expected_description.to_string(),
                strict: false,
                defer_loading: None,
                parameters: JsonSchema::one_of(
                    vec![
                        single_target_schema(),
                        flat_picker_schema(),
                        categorized_picker_schema(),
                    ],
                    Some(
                        "Use the single-target shape for one install card, the flat picker shape for a list, or the categorized picker shape for grouped exact install candidates."
                            .to_string(),
                    ),
                ),
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
