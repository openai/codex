use crate::JsonSchema;
use crate::ResponsesApiTool;
use crate::ToolSpec;
use std::collections::BTreeMap;

pub const REFLECTIONS_NEW_CONTEXT_WINDOW_TOOL_NAME: &str = "reflections_new_context_window";
pub const REFLECTIONS_GET_CONTEXT_REMAINING_TOOL_NAME: &str = "reflections_get_context_remaining";

pub fn create_reflections_new_context_window_tool(usage_hint: Option<&str>) -> ToolSpec {
    let mut description = "Starts a fresh context window for the same task. Use this after you have saved concise recovery notes under the Reflections notes directory when the current context is large or the next steps should continue from durable logs.".to_string();
    if let Some(usage_hint) = usage_hint {
        description.push_str("\n\n");
        description.push_str(usage_hint);
    }

    ToolSpec::Function(ResponsesApiTool {
        name: REFLECTIONS_NEW_CONTEXT_WINDOW_TOOL_NAME.to_string(),
        description,
        strict: false,
        defer_loading: None,
        parameters: empty_parameters(),
        output_schema: None,
    })
}

pub fn create_reflections_get_context_remaining_tool() -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: REFLECTIONS_GET_CONTEXT_REMAINING_TOOL_NAME.to_string(),
        description: "Returns the estimated context window size, used tokens, and remaining tokens for the current thread."
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: empty_parameters(),
        output_schema: None,
    })
}

fn empty_parameters() -> JsonSchema {
    JsonSchema::object(BTreeMap::new(), Some(Vec::new()), Some(false.into()))
}
