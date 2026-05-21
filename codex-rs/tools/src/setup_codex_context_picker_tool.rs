use crate::JsonSchema;
use crate::ResponsesApiTool;
use crate::ToolSpec;
use std::collections::BTreeMap;

pub const SETUP_CODEX_CONTEXT_PICKER_TOOL_NAME: &str = "setup_codex_context_picker";

pub fn create_setup_codex_context_picker_tool() -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: SETUP_CODEX_CONTEXT_PICKER_TOOL_NAME.to_string(),
        description: "Open Codex's onboarding context source picker so the user can connect work apps or add a folder before the assistant continues setup.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(BTreeMap::new(), Some(Vec::new()), Some(false.into())),
        output_schema: None,
    })
}
