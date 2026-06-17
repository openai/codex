use codex_tools::JsonSchema;
use codex_tools::LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use serde_json::json;
use std::collections::BTreeMap;

use crate::tool::REQUEST_PLUGIN_INSTALLS_TOOL_NAME;
use crate::validation::MAX_REQUEST_PLUGIN_INSTALLS_ENTRIES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolSuggestPresentation {
    ListTool,
    RecommendationContext,
}

pub fn create_request_plugin_installs_tool(presentation: ToolSuggestPresentation) -> ToolSpec {
    let description = request_plugin_installs_description(
        presentation,
        RequestPluginInstallsSchema::MultipleEntries,
    );

    ToolSpec::Function(ResponsesApiTool {
        name: REQUEST_PLUGIN_INSTALLS_TOOL_NAME.to_string(),
        description,
        strict: false,
        defer_loading: None,
        parameters: picker_schema(),
        output_schema: None,
    })
}

pub fn create_request_plugin_installs_tool_for_tui(
    presentation: ToolSuggestPresentation,
) -> ToolSpec {
    let description =
        request_plugin_installs_description(presentation, RequestPluginInstallsSchema::SingleEntry);

    ToolSpec::Function(ResponsesApiTool {
        name: REQUEST_PLUGIN_INSTALLS_TOOL_NAME.to_string(),
        description,
        strict: false,
        defer_loading: None,
        parameters: single_entry_picker_schema(),
        output_schema: None,
    })
}

#[derive(Clone, Copy)]
enum RequestPluginInstallsSchema {
    SingleEntry,
    MultipleEntries,
}

fn request_plugin_installs_description(
    presentation: ToolSuggestPresentation,
    schema: RequestPluginInstallsSchema,
) -> String {
    match (presentation, schema) {
        (ToolSuggestPresentation::ListTool, RequestPluginInstallsSchema::SingleEntry) => format!(
            "# Request plugin/connector install\n\nUse this tool only after `{LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME}` returns a connector that exactly matches the user's explicit request.\n\nDo not use it for adjacent capabilities, broad recommendations, or tools that merely seem useful. Make one call with exactly one `entries` item. Pass only exact `tool_type` and `tool_id` values returned by `{LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME}`; Codex resolves picker labels and metadata from that known tool list.\n\nWhen this tool returns, the user-visible install picker has resolved and is no longer visible.\n\nIMPORTANT: DO NOT call this tool in parallel with other tools."
        ),
        (ToolSuggestPresentation::ListTool, RequestPluginInstallsSchema::MultipleEntries) => format!(
            "# Request plugin/connector install\n\nUse this tool only after `{LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME}` returns one or more plugins or connectors that exactly match the user's explicit request.\n\nDo not use it for adjacent capabilities, broad recommendations, or tools that merely seem useful. Make one call with `entries` for a flat list or `categories` when alternatives are organized by category; use one flat `entries` item for a single target, with at most {MAX_REQUEST_PLUGIN_INSTALLS_ENTRIES} entries total. Pass only exact `tool_type` and `tool_id` values returned by `{LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME}`; Codex resolves picker labels and metadata from that known tool list.\n\nWhen this tool returns, the user-visible install picker has resolved and is no longer visible.\n\nIMPORTANT: DO NOT call this tool in parallel with other tools."
        ),
        (
            ToolSuggestPresentation::RecommendationContext,
            RequestPluginInstallsSchema::SingleEntry,
        ) =>
            "# Suggest a recommended plugin installation\n\nSuggest installing exactly one connector from the `<recommended_plugins>` list when it would help with the user's current request.\n\nWhen this tool returns, the user-visible install picker has resolved and is no longer visible.\n\nIMPORTANT: DO NOT call this tool in parallel with other tools.".to_string(),
        (
            ToolSuggestPresentation::RecommendationContext,
            RequestPluginInstallsSchema::MultipleEntries,
        ) => format!(
            "# Suggest recommended plugin installations\n\nSuggest installing one or more plugins from the `<recommended_plugins>` list when they would help with the user's current request. Make one call with `entries` for a flat list or `categories` when alternatives are organized by category; use one flat `entries` item for a single target, with at most {MAX_REQUEST_PLUGIN_INSTALLS_ENTRIES} entries total.\n\nWhen this tool returns, the user-visible install picker has resolved and is no longer visible.\n\nIMPORTANT: DO NOT call this tool in parallel with other tools."
        ),
    }
}

fn picker_schema() -> JsonSchema {
    JsonSchema::object(
        BTreeMap::from([
            ("action_type".to_string(), install_action_schema()),
            (
                "entries".to_string(),
                JsonSchema::array(
                    picker_entry_schema(),
                    Some("Flat list of exact install candidates.".to_string()),
                ),
            ),
            (
                "categories".to_string(),
                JsonSchema::array(
                    picker_category_schema(),
                    Some("Grouped exact install candidates.".to_string()),
                ),
            ),
        ]),
        Some(vec!["action_type".to_string()]),
        Some(false.into()),
    )
}

fn single_entry_picker_schema() -> JsonSchema {
    JsonSchema::object(
        BTreeMap::from([
            ("action_type".to_string(), install_action_schema()),
            (
                "entries".to_string(),
                JsonSchema::array(
                    connector_picker_entry_schema(),
                    Some("Exactly one connector install candidate.".to_string()),
                ),
            ),
        ]),
        Some(vec!["action_type".to_string(), "entries".to_string()]),
        Some(false.into()),
    )
}

fn connector_picker_entry_schema() -> JsonSchema {
    JsonSchema::object(
        BTreeMap::from([
            (
                "tool_id".to_string(),
                JsonSchema::string(Some(
                    "Exact connector id returned by list_available_plugins_to_install.".to_string(),
                )),
            ),
            (
                "tool_type".to_string(),
                JsonSchema::string_enum(
                    vec![json!("connector")],
                    Some(
                        "Use the connector type returned by list_available_plugins_to_install."
                            .to_string(),
                    ),
                ),
            ),
        ]),
        Some(vec!["tool_id".to_string(), "tool_type".to_string()]),
        Some(false.into()),
    )
}

fn picker_entry_schema() -> JsonSchema {
    JsonSchema::object(
        BTreeMap::from([
            (
                "tool_id".to_string(),
                JsonSchema::string(Some(
                    "Exact connector or plugin id returned by list_available_plugins_to_install."
                        .to_string(),
                )),
            ),
            (
                "tool_type".to_string(),
                tool_type_schema("Type returned by list_available_plugins_to_install.".to_string()),
            ),
        ]),
        Some(vec!["tool_id".to_string(), "tool_type".to_string()]),
        Some(false.into()),
    )
}

fn picker_category_schema() -> JsonSchema {
    JsonSchema::object(
        BTreeMap::from([
            (
                "title".to_string(),
                JsonSchema::string(Some("User-facing category title.".to_string())),
            ),
            (
                "entries".to_string(),
                JsonSchema::array(
                    picker_entry_schema(),
                    Some("Install candidates in this category.".to_string()),
                ),
            ),
        ]),
        Some(vec!["title".to_string(), "entries".to_string()]),
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

#[cfg(test)]
#[path = "spec_tests.rs"]
mod tests;
