use crate::FreeformTool;
use crate::FreeformToolFormat;
use crate::JsonSchema;
use crate::ResponsesApiTool;
use crate::ToolSpec;
use std::collections::BTreeMap;

pub fn create_js_repl_tool() -> ToolSpec {
    // Keep JS input freeform, but block the most common malformed payload shapes
    // (JSON wrappers, quoted strings, and markdown fences) before they reach the
    // runtime `reject_json_or_quoted_source` validation. The API's regex engine
    // does not support look-around, so this uses a "first significant token"
    // pattern rather than negative lookaheads.
    const JS_REPL_FREEFORM_GRAMMAR: &str = r#"
start: pragma_source | plain_source

pragma_source: PRAGMA_LINE NEWLINE js_source
plain_source: PLAIN_JS_SOURCE

js_source: JS_SOURCE

PRAGMA_LINE: /[ \t]*\/\/ codex-js-repl:[^\r\n]*/
NEWLINE: /\r?\n/
PLAIN_JS_SOURCE: /(?:\s*)(?:[^\s{\"`]|`[^`]|``[^`])[\s\S]*/
JS_SOURCE: /(?:\s*)(?:[^\s{\"`]|`[^`]|``[^`])[\s\S]*/
"#;

    ToolSpec::Freeform(FreeformTool {
        name: "js_repl".to_string(),
        description: "Runs JavaScript in a persistent Node kernel with top-level await. This is a freeform tool: send raw JavaScript source text, optionally with a first-line pragma like `// codex-js-repl: timeout_ms=15000`; do not send JSON/quotes/markdown fences."
            .to_string(),
        format: FreeformToolFormat {
            r#type: "grammar".to_string(),
            syntax: "lark".to_string(),
            definition: JS_REPL_FREEFORM_GRAMMAR.to_string(),
        },
    })
}

pub fn create_js_repl_reset_tool() -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: "js_repl_reset".to_string(),
        description:
            "Restarts the js_repl kernel for this run and clears persisted top-level bindings."
                .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(BTreeMap::new(), /*required*/ None, Some(false.into())),
        output_schema: None,
    })
}

#[cfg(test)]
#[path = "js_repl_tool_tests.rs"]
mod tests;
