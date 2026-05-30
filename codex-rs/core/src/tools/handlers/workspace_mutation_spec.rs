use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use std::collections::BTreeMap;

pub(crate) fn create_set_working_directory_tool() -> ToolSpec {
    create_workspace_mutation_tool(
        "set_working_directory",
        "Changes the active working directory and adds it as a workspace root when needed.",
    )
}

pub(crate) fn create_add_workspace_root_tool() -> ToolSpec {
    create_workspace_mutation_tool(
        "add_workspace_root",
        "Adds a workspace root without changing the active working directory.",
    )
}

fn create_workspace_mutation_tool(name: &str, summary: &str) -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: name.to_string(),
        description: format!(
            "{summary}\n\nRelative paths resolve from the active working directory. Later tool calls in the same batch start after this mutation succeeds and are cancelled if it fails."
        ),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            BTreeMap::from([(
                "path".to_string(),
                JsonSchema::string(Some("Existing directory path.".to_string())),
            )]),
            Some(vec!["path".to_string()]),
            /*additional_properties*/ Some(false.into()),
        ),
        output_schema: None,
    })
}
