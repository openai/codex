use super::ToolDefinition;
use crate::JsonSchema;
use crate::ToolExecution;
use crate::ToolLoadingPolicy;
use crate::ToolName;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

fn tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: ToolName::plain("lookup_order"),
        description: "Look up an order".to_string(),
        input_schema: JsonSchema::object(
            BTreeMap::new(),
            /*required*/ None,
            /*additional_properties*/ None,
        ),
        output_schema: Some(serde_json::json!({
            "type": "object",
        })),
        loading: ToolLoadingPolicy::Eager,
        execution: ToolExecution::Dynamic,
        presentation: None,
        search: None,
        supports_parallel_tool_calls: false,
    }
}

#[test]
fn renamed_overrides_name_only() {
    assert_eq!(
        tool_definition().renamed(ToolName::namespaced("mcp__orders__", "lookup_order")),
        ToolDefinition {
            name: ToolName::namespaced("mcp__orders__", "lookup_order"),
            ..tool_definition()
        }
    );
}

#[test]
fn into_deferred_drops_output_schema_and_sets_defer_loading() {
    assert_eq!(
        tool_definition().into_deferred(),
        ToolDefinition {
            output_schema: None,
            loading: ToolLoadingPolicy::Deferred,
            ..tool_definition()
        }
    );
}
