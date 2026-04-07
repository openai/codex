use crate::JsonSchema;
use crate::ResponsesApiTool;
use crate::ToolSpec;
use std::collections::BTreeMap;

pub fn create_update_plan_tool() -> ToolSpec {
    let plan_item_properties = BTreeMap::from([
        ("step".to_string(), JsonSchema::String { description: None }),
        (
            "status".to_string(),
            JsonSchema::String {
                description: Some("One of: pending, in_progress, completed".to_string()),
            },
        ),
    ]);

    let properties = BTreeMap::from([
        (
            "explanation".to_string(),
            JsonSchema::String { description: None },
        ),
        (
            "plan".to_string(),
            JsonSchema::Array {
                description: Some("The list of steps".to_string()),
                items: Box::new(JsonSchema::Object {
                    properties: plan_item_properties,
                    required: Some(vec!["step".to_string(), "status".to_string()]),
                    additional_properties: Some(false.into()),
                }),
            },
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: "update_plan".to_string(),
        description: r#"Updates the task plan.
Provide an optional explanation and a list of plan items, each with a step and status.
At most one step can be in_progress at a time.
"#
        .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["plan".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}
