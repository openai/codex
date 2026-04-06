use super::*;
use crate::ToolSpec;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

#[test]
fn js_repl_tool_uses_expected_freeform_grammar() {
    let ToolSpec::Freeform(FreeformTool { format, .. }) = create_js_repl_tool() else {
        panic!("js_repl should use a freeform tool spec");
    };

    assert_eq!(format.syntax, "lark");
    assert!(format.definition.contains("PRAGMA_LINE"));
    assert!(format.definition.contains("`[^`]"));
    assert!(format.definition.contains("``[^`]"));
    assert!(format.definition.contains("PLAIN_JS_SOURCE"));
    assert!(format.definition.contains("codex-js-repl:"));
    assert!(!format.definition.contains("(?!"));
}

#[test]
fn js_repl_reset_tool_matches_expected_spec() {
    assert_eq!(
        create_js_repl_reset_tool(),
        ToolSpec::Function(ResponsesApiTool {
            name: "js_repl_reset".to_string(),
            description:
                "Restarts the js_repl kernel for this run and clears persisted top-level bindings."
                    .to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::Object {
                properties: BTreeMap::new(),
                required: None,
                additional_properties: Some(false.into()),
            },
            output_schema: None,
        })
    );
}
