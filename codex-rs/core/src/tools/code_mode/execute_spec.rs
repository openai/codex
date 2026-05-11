use codex_code_mode::ToolDefinition as CodeModeToolDefinition;
use codex_tools::FreeformTool;
use codex_tools::FreeformToolFormat;
use codex_tools::ToolSpec;
use std::collections::BTreeMap;

pub(crate) fn create_code_mode_tool(
    enabled_tools: &[CodeModeToolDefinition],
    namespace_descriptions: &BTreeMap<String, codex_code_mode::ToolNamespaceDescription>,
    code_mode_only: bool,
    deferred_tools_available: bool,
) -> ToolSpec {
    const CODE_MODE_FREEFORM_GRAMMAR: &str = r#"
start: pragma_source | plain_source
pragma_source: PRAGMA_LINE NEWLINE SOURCE
plain_source: SOURCE

PRAGMA_LINE: /[ \t]*\/\/ @exec:[^\r\n]*/
NEWLINE: /\r?\n/
SOURCE: /[\s\S]+/
"#;

    ToolSpec::Freeform(FreeformTool {
        name: codex_code_mode::PUBLIC_TOOL_NAME.to_string(),
        description: codex_code_mode::build_exec_tool_description(
            enabled_tools,
            namespace_descriptions,
            code_mode_only,
            deferred_tools_available,
        ),
        format: FreeformToolFormat {
            r#type: "grammar".to_string(),
            syntax: "lark".to_string(),
            definition: CODE_MODE_FREEFORM_GRAMMAR.to_string(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_tools::ToolName;
    use pretty_assertions::assert_eq;

    #[test]
    fn create_code_mode_tool_matches_expected_spec() {
        let enabled_tools = vec![codex_code_mode::ToolDefinition {
            name: "update_plan".to_string(),
            tool_name: ToolName::plain("update_plan"),
            description: "Update the plan".to_string(),
            kind: codex_code_mode::CodeModeToolKind::Function,
            input_schema: None,
            output_schema: None,
        }];

        assert_eq!(
            create_code_mode_tool(
                &enabled_tools,
                &BTreeMap::new(),
                /*code_mode_only*/ true,
                /*deferred_tools_available*/ false,
            ),
            ToolSpec::Freeform(FreeformTool {
                name: codex_code_mode::PUBLIC_TOOL_NAME.to_string(),
                description: codex_code_mode::build_exec_tool_description(
                    &enabled_tools,
                    &BTreeMap::new(),
                    /*code_mode_only*/ true,
                    /*deferred_tools_available*/ false
                ),
                format: FreeformToolFormat {
                    r#type: "grammar".to_string(),
                    syntax: "lark".to_string(),
                    definition: r#"
start: pragma_source | plain_source
pragma_source: PRAGMA_LINE NEWLINE SOURCE
plain_source: SOURCE

PRAGMA_LINE: /[ \t]*\/\/ @exec:[^\r\n]*/
NEWLINE: /\r?\n/
SOURCE: /[\s\S]+/
"#
                    .to_string(),
                },
            })
        );
    }
}
