use super::*;
use crate::tools::tool_search_entry::ToolSearchEntry;
use codex_tools::JsonSchema;
use codex_tools::LoadableToolSpec;
use codex_tools::ResponsesApiTool;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::BTreeMap;

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

    assert_eq!(
        handler
            .search_info()
            .expect("dynamic search info")
            .entry,
        ToolSearchEntry {
            search_text: "automation_update automation update Create or update automations. codex_app mode timezone"
                .to_string(),
            output: LoadableToolSpec::Namespace(ResponsesApiNamespace {
                name: "codex_app".to_string(),
                description: "Tools in the codex_app namespace.".to_string(),
                tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                    name: "automation_update".to_string(),
                    description: "Create or update automations.".to_string(),
                    strict: false,
                    defer_loading: Some(true),
                    parameters: JsonSchema::object(
                        BTreeMap::from([
                            (
                                "mode".to_string(),
                                JsonSchema::string(/*description*/ None),
                            ),
                            (
                                "timezone".to_string(),
                                JsonSchema::string(/*description*/ None),
                            ),
                        ]),
                        /*required*/ None,
                        /*additional_properties*/ None,
                    ),
                    output_schema: None,
                })],
            }),
        }
    );
}
