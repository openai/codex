use crate::ToolSpec;
use codex_code_mode::CodeModeToolKind;
use codex_code_mode::ToolDefinition as CodeModeToolDefinition;

/// Augment tool descriptions with code-mode-specific exec samples.
pub fn augment_tool_spec_for_code_mode(spec: ToolSpec) -> ToolSpec {
    let Some(description) = code_mode_tool_definition_for_spec(&spec)
        .map(codex_code_mode::augment_tool_definition)
        .map(|definition| definition.description)
    else {
        return spec;
    };

    match spec {
        ToolSpec::Function(mut tool) => {
            tool.description = description;
            ToolSpec::Function(tool)
        }
        ToolSpec::Freeform(mut tool) => {
            tool.description = description;
            ToolSpec::Freeform(tool)
        }
        other => other,
    }
}

/// Convert a supported nested tool spec into the code-mode runtime shape,
/// including the code-mode-specific description sample.
pub fn tool_spec_to_code_mode_tool_definition(spec: &ToolSpec) -> Option<CodeModeToolDefinition> {
    let definition = code_mode_tool_definition_for_spec(spec)?;
    codex_code_mode::is_code_mode_nested_tool(&definition.name)
        .then(|| codex_code_mode::augment_tool_definition(definition))
}

fn code_mode_tool_definition_for_spec(spec: &ToolSpec) -> Option<CodeModeToolDefinition> {
    match spec {
        ToolSpec::Function(tool) => Some(CodeModeToolDefinition {
            name: tool.name.clone(),
            description: tool.description.clone(),
            kind: CodeModeToolKind::Function,
            input_schema: serde_json::to_value(&tool.parameters).ok(),
            output_schema: tool.output_schema.clone(),
        }),
        ToolSpec::Freeform(tool) => Some(CodeModeToolDefinition {
            name: tool.name.clone(),
            description: tool.description.clone(),
            kind: CodeModeToolKind::Freeform,
            input_schema: None,
            output_schema: None,
        }),
        ToolSpec::Namespace(_)
        | ToolSpec::LocalShell {}
        | ToolSpec::ImageGeneration { .. }
        | ToolSpec::ToolSearch { .. }
        | ToolSpec::WebSearch { .. } => None,
    }
}

#[cfg(test)]
#[path = "code_mode_tests.rs"]
mod tests;
