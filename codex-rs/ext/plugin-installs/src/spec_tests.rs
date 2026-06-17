use super::*;
use pretty_assertions::assert_eq;

#[test]
fn create_request_plugin_installs_tool_uses_expected_wire_shape() {
    let expected_description = concat!(
        "# Request plugin/connector install\n\n",
        "Use this tool only after `list_available_plugins_to_install` returns one or more plugins or connectors that exactly match the user's explicit request.\n\n",
        "Do not use it for adjacent capabilities, broad recommendations, or tools that merely seem useful. Make one call with `entries` for a flat list or `categories` when alternatives are organized by category; use one flat `entries` item for a single target, with at most 16 entries total. Pass only exact `tool_type` and `tool_id` values returned by `list_available_plugins_to_install`; Codex resolves picker labels and metadata from that known tool list.\n\n",
        "When this tool returns, the user-visible install picker has resolved and is no longer visible.\n\n",
        "IMPORTANT: DO NOT call this tool in parallel with other tools.",
    );

    assert_eq!(
        create_request_plugin_installs_tool(ToolSuggestPresentation::ListTool),
        ToolSpec::Function(ResponsesApiTool {
            name: "request_plugin_installs".to_string(),
            description: expected_description.to_string(),
            strict: false,
            defer_loading: None,
            parameters: picker_schema(),
            output_schema: None,
        })
    );
}

#[test]
fn create_request_plugin_installs_tool_for_tui_uses_single_entry_shape() {
    let expected_description = concat!(
        "# Request plugin/connector install\n\n",
        "Use this tool only after `list_available_plugins_to_install` returns a connector that exactly matches the user's explicit request.\n\n",
        "Do not use it for adjacent capabilities, broad recommendations, or tools that merely seem useful. Make one call with exactly one `entries` item. Pass only exact `tool_type` and `tool_id` values returned by `list_available_plugins_to_install`; Codex resolves picker labels and metadata from that known tool list.\n\n",
        "When this tool returns, the user-visible install picker has resolved and is no longer visible.\n\n",
        "IMPORTANT: DO NOT call this tool in parallel with other tools.",
    );

    assert_eq!(
        create_request_plugin_installs_tool_for_tui(ToolSuggestPresentation::ListTool),
        ToolSpec::Function(ResponsesApiTool {
            name: "request_plugin_installs".to_string(),
            description: expected_description.to_string(),
            strict: false,
            defer_loading: None,
            parameters: single_entry_picker_schema(),
            output_schema: None,
        })
    );
}

#[test]
fn plural_developer_recommendations_change_only_the_description() {
    let mut expected = create_request_plugin_installs_tool(ToolSuggestPresentation::ListTool);
    let recommendations =
        create_request_plugin_installs_tool(ToolSuggestPresentation::RecommendationContext);

    let ToolSpec::Function(expected_function) = &mut expected else {
        panic!("expected function tool specs");
    };
    expected_function.description = format!(
        "# Suggest recommended plugin installations\n\nSuggest installing one or more plugins from the `<recommended_plugins>` list when they would help with the user's current request. Make one call with `entries` for a flat list or `categories` when alternatives are organized by category; use one flat `entries` item for a single target, with at most {MAX_REQUEST_PLUGIN_INSTALLS_ENTRIES} entries total.\n\nWhen this tool returns, the user-visible install picker has resolved and is no longer visible.\n\nIMPORTANT: DO NOT call this tool in parallel with other tools."
    );

    assert_eq!(recommendations, expected);
}
