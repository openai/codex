use super::*;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

#[test]
fn default_search_text_uses_descriptions_and_schema_property_names() {
    let mut schedule_schema = JsonSchema::object(
        BTreeMap::from([(
            "timezone".to_string(),
            JsonSchema::string(Some("IANA timezone.".to_string())),
        )]),
        /*required*/ None,
        /*additional_properties*/ None,
    );
    schedule_schema.description = Some("Schedule settings.".to_string());
    let mut parameters = JsonSchema::object(
        BTreeMap::from([
            (
                "mode".to_string(),
                JsonSchema::string(Some("Update mode.".to_string())),
            ),
            ("schedule".to_string(), schedule_schema),
        ]),
        /*required*/ None,
        /*additional_properties*/ None,
    );
    parameters.description = Some("Automation options.".to_string());
    let spec = ToolSpec::Namespace(crate::ResponsesApiNamespace {
        name: "codex_app".to_string(),
        description: "Manage Codex automations.".to_string(),
        tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
            name: "automation_update".to_string(),
            description: "Create or update automations.".to_string(),
            strict: false,
            defer_loading: None,
            parameters,
            output_schema: None,
        })],
    });

    let search_info = ToolSearchInfo::from_tool_spec(spec, /*source_info*/ None)
        .expect("namespace should be searchable");

    assert_eq!(
        search_info.entry.search_text,
        "Manage Codex automations. Create or update automations. mode schedule timezone"
    );
    assert_eq!(
        search_info.entry.identity,
        ToolSearchIdentity {
            canonical_aliases: vec![
                "codex_app.automation_update".to_string(),
                "codex_app__automation_update".to_string(),
                "codex_appautomation_update".to_string(),
            ],
            tool_aliases: vec!["automation_update".to_string()],
            source_aliases: vec!["codex_app".to_string()],
        }
    );
}

#[test]
fn mixed_namespaced_function_and_custom_tools_are_searchable() {
    let function_tool = ResponsesApiTool {
        name: "lookup_order".to_string(),
        description: "Look up an order".to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            BTreeMap::new(),
            /*required*/ None,
            /*additional_properties*/ None,
        ),
        output_schema: Some(serde_json::json!({"type": "object"})),
    };
    let custom_tool = crate::FreeformTool {
        name: "apply_patch".to_string(),
        description: "Apply a patch".to_string(),
        defer_loading: None,
        format: crate::FreeformToolFormat {
            r#type: "grammar".to_string(),
            syntax: "lark".to_string(),
            definition: "start: \"patch\"".to_string(),
        },
    };
    let spec = ToolSpec::Namespace(crate::ResponsesApiNamespace {
        name: "editor".to_string(),
        description: "Editing tools".to_string(),
        tools: vec![
            ResponsesApiNamespaceTool::Function(function_tool.clone()),
            ResponsesApiNamespaceTool::Custom(custom_tool.clone()),
        ],
    });

    let search_info = ToolSearchInfo::from_tool_spec(spec, /*source_info*/ None)
        .expect("mixed namespace should be searchable");

    assert_eq!(
        search_info.entry.search_text,
        "editor Editing tools lookup_order lookup order Look up an order apply_patch Apply a patch lark"
    );
    assert_eq!(
        search_info.entry.output,
        LoadableToolSpec::Namespace(crate::ResponsesApiNamespace {
            name: "editor".to_string(),
            description: "Editing tools".to_string(),
            tools: vec![
                ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                    defer_loading: Some(true),
                    output_schema: None,
                    ..function_tool
                }),
                ResponsesApiNamespaceTool::Custom(crate::FreeformTool {
                    defer_loading: Some(true),
                    ..custom_tool
                }),
            ],
        })
    );
}
