use super::*;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

#[test]
fn default_search_text_uses_model_visible_namespace_metadata_once() {
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
        "codex_app Manage Codex automations. automation_update automation update Create or update automations. Automation options. mode Update mode. schedule Schedule settings. timezone IANA timezone."
    );
}
