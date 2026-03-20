use crate::client_common::tools::ToolSpec;

#[allow(unused_imports)]
#[cfg(test)]
pub(crate) use codex_code_mode::append_code_mode_sample;
#[allow(unused_imports)]
#[cfg(test)]
pub(crate) use codex_code_mode::render_json_schema_to_typescript;

pub(crate) fn augment_tool_spec_for_code_mode(spec: ToolSpec, code_mode_enabled: bool) -> ToolSpec {
    if !code_mode_enabled {
        return spec;
    }

    match spec {
        ToolSpec::Function(tool) => {
            let definition = codex_code_mode::ToolDefinition {
                name: tool.name.clone(),
                description: tool.description,
                kind: codex_code_mode::CodeModeToolKind::Function,
                input_schema: serde_json::to_value(&tool.parameters).ok(),
                output_schema: tool.output_schema,
            };
            let definition = codex_code_mode::augment_tool_definition(definition);
            ToolSpec::Function(crate::client_common::tools::ResponsesApiTool {
                name: tool.name,
                description: definition.description,
                strict: tool.strict,
                defer_loading: tool.defer_loading,
                parameters: tool.parameters,
                output_schema: definition.output_schema,
            })
        }
        ToolSpec::Freeform(tool) => {
            let definition = codex_code_mode::ToolDefinition {
                name: tool.name.clone(),
                description: tool.description,
                kind: codex_code_mode::CodeModeToolKind::Freeform,
                input_schema: None,
                output_schema: None,
            };
            let definition = codex_code_mode::augment_tool_definition(definition);
            ToolSpec::Freeform(crate::client_common::tools::FreeformTool {
                name: tool.name,
                description: definition.description,
                format: tool.format,
            })
        }
        other => other,
    }
}
