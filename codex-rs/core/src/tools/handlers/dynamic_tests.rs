use super::*;
use codex_tools::ToolSearchSourceInfo;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn search_info_uses_dynamic_tool_metadata_and_parameter_names() {
    let handler = DynamicToolHandler::new(&DynamicToolSpec {
        namespace: Some("codex_app".to_string()),
        name: "automation_update".to_string(),
        description: "Create or update automations.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "timezone": { "type": "string" },
                "mode": { "type": "string" }
            }
        }),
        defer_loading: true,
    })
    .expect("dynamic handler should be created");

    let search_info = handler.search_info().expect("dynamic search info");

    assert_eq!(
        search_info.entry.search_text,
        "automation_update automation update Create or update automations. codex_app mode timezone"
    );
    assert_eq!(
        search_info.source_info,
        Some(ToolSearchSourceInfo {
            name: "Dynamic tools".to_string(),
            description: Some("Tools provided by the current Codex thread.".to_string()),
        })
    );
}
