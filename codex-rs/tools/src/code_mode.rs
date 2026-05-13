use crate::ResponsesApiNamespaceTool;
use crate::ToolName;
use crate::ToolSpec;
use codex_code_mode::CodeModeToolKind;
use codex_code_mode::ToolDefinition as CodeModeToolDefinition;

/// Augment tool descriptions with code-mode-specific exec samples.
pub fn augment_tool_spec_for_code_mode(spec: ToolSpec) -> ToolSpec {
    match spec {
        ToolSpec::Function(mut tool) => {
            let Some(description) =
                augmented_description_for_spec(&ToolSpec::Function(tool.clone()))
            else {
                return ToolSpec::Function(tool);
            };
            tool.description = description;
            ToolSpec::Function(tool)
        }
        ToolSpec::Freeform(mut tool) => {
            let Some(description) =
                augmented_description_for_spec(&ToolSpec::Freeform(tool.clone()))
            else {
                return ToolSpec::Freeform(tool);
            };
            tool.description = description;
            ToolSpec::Freeform(tool)
        }
        ToolSpec::Namespace(mut namespace) => {
            for tool in &mut namespace.tools {
                match tool {
                    ResponsesApiNamespaceTool::Function(tool) => {
                        let tool_name =
                            ToolName::namespaced(namespace.name.clone(), tool.name.clone());
                        let definition = CodeModeToolDefinition {
                            name: code_mode_name_for_tool_name(&tool_name),
                            tool_name,
                            description: tool.description.clone(),
                            kind: CodeModeToolKind::Function,
                            input_schema: serde_json::to_value(&tool.parameters).ok(),
                            output_schema: tool.output_schema.clone(),
                        };
                        tool.description =
                            codex_code_mode::augment_tool_definition(definition).description;
                    }
                }
            }
            ToolSpec::Namespace(namespace)
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

pub fn collect_code_mode_tool_definitions<'a>(
    specs: impl IntoIterator<Item = &'a ToolSpec>,
) -> Vec<CodeModeToolDefinition> {
    let mut tool_definitions = specs
        .into_iter()
        .flat_map(code_mode_tool_definitions_for_spec)
        .filter(|definition| codex_code_mode::is_code_mode_nested_tool(&definition.name))
        .map(codex_code_mode::augment_tool_definition)
        .collect::<Vec<_>>();
    tool_definitions.sort_by(|left, right| left.name.cmp(&right.name));
    tool_definitions.dedup_by(|left, right| left.name == right.name);
    tool_definitions
}

pub fn collect_code_mode_exec_prompt_tool_definitions<'a>(
    specs: impl IntoIterator<Item = &'a ToolSpec>,
) -> Vec<CodeModeToolDefinition> {
    let mut tool_definitions = specs
        .into_iter()
        .flat_map(code_mode_tool_definitions_for_spec)
        .filter(|definition| codex_code_mode::is_code_mode_nested_tool(&definition.name))
        .collect::<Vec<_>>();
    tool_definitions.sort_by(|left, right| left.name.cmp(&right.name));
    tool_definitions.dedup_by(|left, right| left.name == right.name);
    tool_definitions
}

fn augmented_description_for_spec(spec: &ToolSpec) -> Option<String> {
    code_mode_tool_definition_for_spec(spec)
        .map(codex_code_mode::augment_tool_definition)
        .map(|definition| definition.description)
}

fn code_mode_tool_definition_for_spec(spec: &ToolSpec) -> Option<CodeModeToolDefinition> {
    code_mode_tool_definitions_for_spec(spec).into_iter().next()
}

fn code_mode_tool_definitions_for_spec(spec: &ToolSpec) -> Vec<CodeModeToolDefinition> {
    match spec {
        ToolSpec::Function(tool) => {
            let name = tool.name.clone();
            vec![CodeModeToolDefinition {
                tool_name: ToolName::plain(name.clone()),
                name,
                description: tool.description.clone(),
                kind: CodeModeToolKind::Function,
                input_schema: serde_json::to_value(&tool.parameters).ok(),
                output_schema: tool.output_schema.clone(),
            }]
        }
        ToolSpec::Freeform(tool) => {
            let name = tool.name.clone();
            vec![CodeModeToolDefinition {
                tool_name: ToolName::plain(name.clone()),
                name,
                description: tool.description.clone(),
                kind: CodeModeToolKind::Freeform,
                input_schema: None,
                output_schema: None,
            }]
        }
        ToolSpec::Namespace(namespace) => namespace
            .tools
            .iter()
            .map(|tool| match tool {
                ResponsesApiNamespaceTool::Function(tool) => {
                    let tool_name = ToolName::namespaced(namespace.name.clone(), tool.name.clone());
                    CodeModeToolDefinition {
                        name: code_mode_name_for_tool_name(&tool_name),
                        tool_name,
                        description: tool.description.clone(),
                        kind: CodeModeToolKind::Function,
                        input_schema: serde_json::to_value(&tool.parameters).ok(),
                        output_schema: tool.output_schema.clone(),
                    }
                }
            })
            .collect(),
        ToolSpec::LocalShell {}
        | ToolSpec::ImageGeneration { .. }
        | ToolSpec::ToolSearch { .. }
        | ToolSpec::WebSearch { .. } => Vec::new(),
    }
}

pub fn code_mode_name_for_tool_name(tool_name: &ToolName) -> String {
    match tool_name.namespace.as_deref() {
        Some(namespace) if namespace.ends_with('_') || tool_name.name.starts_with('_') => {
            format!("{namespace}{}", tool_name.name)
        }
        Some(namespace) => format!("{namespace}_{}", tool_name.name),
        None => tool_name.name.clone(),
    }
}

#[cfg(test)]
#[path = "code_mode_tests.rs"]
mod tests;
