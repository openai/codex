use super::augment_tool_spec_for_code_mode;
use super::create_code_mode_tool;
use super::create_wait_tool;
use super::tool_spec_to_code_mode_tool_definition;
use crate::AdditionalProperties;
use crate::FreeformTool;
use crate::FreeformToolFormat;
use crate::JsonSchema;
use crate::ResponsesApiTool;
use crate::ToolSpec;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn augment_tool_spec_for_code_mode_augments_function_tools() {
    assert_eq!(
        augment_tool_spec_for_code_mode(ToolSpec::Function(ResponsesApiTool {
            name: "lookup_order".to_string(),
            description: "Look up an order".to_string(),
            strict: false,
            defer_loading: Some(true),
            parameters: JsonSchema::object(
                BTreeMap::from([(
                    "order_id".to_string(),
                    JsonSchema::string(/*description*/ None),
                )]),
                Some(vec!["order_id".to_string()]),
                Some(AdditionalProperties::Boolean(false))
            ),
            output_schema: Some(json!({
                "type": "object",
                "properties": {
                    "ok": {"type": "boolean"}
                },
                "required": ["ok"],
            })),
        })),
        ToolSpec::Function(ResponsesApiTool {
            name: "lookup_order".to_string(),
            description: r#"Look up an order

exec tool declaration:
```ts
declare const tools: { lookup_order(args: { order_id: string; }): Promise<{ ok: boolean; }>; };
```"#
                .to_string(),
            strict: false,
            defer_loading: Some(true),
            parameters: JsonSchema::object(
                BTreeMap::from([(
                    "order_id".to_string(),
                    JsonSchema::string(/*description*/ None),
                )]),
                Some(vec!["order_id".to_string()]),
                Some(AdditionalProperties::Boolean(false))
            ),
            output_schema: Some(json!({
                "type": "object",
                "properties": {
                    "ok": {"type": "boolean"}
                },
                "required": ["ok"],
            })),
        })
    );
}

#[test]
fn augment_tool_spec_for_code_mode_preserves_exec_tool_description() {
    assert_eq!(
        augment_tool_spec_for_code_mode(ToolSpec::Freeform(FreeformTool {
            name: codex_code_mode::PUBLIC_TOOL_NAME.to_string(),
            description: "Run code".to_string(),
            format: FreeformToolFormat {
                r#type: "grammar".to_string(),
                syntax: "lark".to_string(),
                definition: "start: \"exec\"".to_string(),
            },
        })),
        ToolSpec::Freeform(FreeformTool {
            name: codex_code_mode::PUBLIC_TOOL_NAME.to_string(),
            description: "Run code".to_string(),
            format: FreeformToolFormat {
                r#type: "grammar".to_string(),
                syntax: "lark".to_string(),
                definition: "start: \"exec\"".to_string(),
            },
        })
    );
}

#[test]
fn tool_spec_to_code_mode_tool_definition_returns_augmented_nested_tools() {
    let spec = ToolSpec::Freeform(FreeformTool {
        name: "apply_patch".to_string(),
        description: "Apply a patch".to_string(),
        format: FreeformToolFormat {
            r#type: "grammar".to_string(),
            syntax: "lark".to_string(),
            definition: "start: \"patch\"".to_string(),
        },
    });

    assert_eq!(
        tool_spec_to_code_mode_tool_definition(&spec),
        Some(codex_code_mode::ToolDefinition {
            name: "apply_patch".to_string(),
            description: r#"Apply a patch

exec tool declaration:
```ts
declare const tools: { apply_patch(input: string): Promise<unknown>; };
```"#
                .to_string(),
            kind: codex_code_mode::CodeModeToolKind::Freeform,
            input_schema: None,
            output_schema: None,
        })
    );
}

#[test]
fn tool_spec_to_code_mode_tool_definition_skips_unsupported_variants() {
    assert_eq!(
        tool_spec_to_code_mode_tool_definition(&ToolSpec::ToolSearch {
            execution: "sync".to_string(),
            description: "Search".to_string(),
            parameters: JsonSchema::object(
                BTreeMap::new(),
                /*required*/ None,
                /*additional_properties*/ None
            ),
        }),
        None
    );
}

#[test]
fn create_wait_tool_matches_expected_spec() {
    assert_eq!(
        create_wait_tool(),
        ToolSpec::Function(ResponsesApiTool {
            name: codex_code_mode::WAIT_TOOL_NAME.to_string(),
            description: format!(
                "Waits on a yielded `{}` cell and returns new output or completion.\n{}",
                codex_code_mode::PUBLIC_TOOL_NAME,
                codex_code_mode::build_wait_tool_description().trim()
            ),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(BTreeMap::from([
                    (
                        "cell_id".to_string(),
                        JsonSchema::string(Some("Identifier of the running exec cell.".to_string()),),
                    ),
                    (
                        "max_tokens".to_string(),
                        JsonSchema::number(Some(
                                "Maximum number of output tokens to return for this wait call."
                                    .to_string(),
                            ),),
                    ),
                    (
                        "terminate".to_string(),
                        JsonSchema::boolean(Some(
                                "Whether to terminate the running exec cell.".to_string(),
                            ),),
                    ),
                    (
                        "yield_time_ms".to_string(),
                        JsonSchema::number(Some(
                                "How long to wait (in milliseconds) for more output before yielding again."
                                    .to_string(),
                            ),),
                    ),
                ]), Some(vec!["cell_id".to_string()]), Some(false.into())),
            output_schema: None,
        })
    );
}

#[test]
fn create_code_mode_tool_matches_expected_spec() {
    let enabled_tools = vec![("update_plan".to_string(), "Update the plan".to_string())];

    assert_eq!(
        create_code_mode_tool(
            &enabled_tools,
            &BTreeMap::new(),
            /*code_mode_only_enabled*/ true,
        ),
        ToolSpec::Freeform(FreeformTool {
            name: codex_code_mode::PUBLIC_TOOL_NAME.to_string(),
            description: codex_code_mode::build_exec_tool_description(
                &enabled_tools,
                &BTreeMap::new(),
                /*code_mode_only*/ true
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
