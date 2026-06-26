use codex_app_server_protocol::DynamicToolCallOutputContentItem;
use codex_app_server_protocol::DynamicToolCallResponse;
use codex_app_server_protocol::DynamicToolFunctionSpec;
use codex_app_server_protocol::DynamicToolNamespaceSpec;
use codex_app_server_protocol::DynamicToolNamespaceTool;
use codex_app_server_protocol::DynamicToolSpec;
use codex_terminal_browser::BrowserToolOutput;
use codex_terminal_browser::TerminalBrowser;
use serde_json::json;

pub(crate) const TERMINAL_BROWSER_NAMESPACE: &str = "terminal_browser";

pub(crate) fn terminal_browser_available() -> bool {
    TerminalBrowser::discover().is_available()
}

pub(crate) fn dynamic_tool_specs() -> Vec<DynamicToolSpec> {
    vec![DynamicToolSpec::Namespace(DynamicToolNamespaceSpec {
        name: TERMINAL_BROWSER_NAMESPACE.to_string(),
        description: "Control a browser rendered inside the terminal.".to_string(),
        tools: vec![
            tool(
                "open",
                "Open a URL in the terminal browser. The floating view is watch-only; Codex controls the page.",
                json!({
                    "type": "object",
                    "properties": {
                        "url": {
                            "type": "string",
                            "maxLength": 4096,
                            "description": "HTTP or HTTPS URL to open."
                        },
                        "visible": { "type": "boolean", "description": "Whether to show the floating browser view." },
                        "renderMode": {
                            "type": "string",
                            "enum": ["nativeText", "bitmap"],
                            "description": "Use native terminal text by default, or bitmap mode for page screenshots."
                        }
                    },
                    "required": ["url"],
                    "additionalProperties": false
                }),
            ),
            tool(
                "snapshot",
                "Return a bounded accessibility snapshot of the current page with document-scoped node IDs.",
                empty_schema(),
            ),
            tool(
                "click",
                "Click a node from the latest terminal browser snapshot.",
                json!({
                    "type": "object",
                    "properties": {
                        "nodeId": { "type": "string", "pattern": "^d[0-9a-f]{16}n[0-9]{1,20}$" }
                    },
                    "required": ["nodeId"],
                    "additionalProperties": false
                }),
            ),
            tool(
                "fill",
                "Replace the value of an editable node from the latest terminal browser snapshot.",
                json!({
                    "type": "object",
                    "properties": {
                        "nodeId": { "type": "string", "pattern": "^d[0-9a-f]{16}n[0-9]{1,20}$" },
                        "text": { "type": "string", "maxLength": 65536 }
                    },
                    "required": ["nodeId", "text"],
                    "additionalProperties": false
                }),
            ),
            tool(
                "press",
                "Send a key press to the active page, for example Enter, Tab, or Escape.",
                json!({
                    "type": "object",
                    "properties": {
                        "key": { "type": "string", "minLength": 1, "maxLength": 32 }
                    },
                    "required": ["key"],
                    "additionalProperties": false
                }),
            ),
            tool(
                "scroll",
                "Scroll the active page by CSS pixel deltas.",
                json!({
                    "type": "object",
                    "properties": {
                        "deltaX": { "type": "integer" },
                        "deltaY": { "type": "integer" }
                    },
                    "required": ["deltaY"],
                    "additionalProperties": false
                }),
            ),
            tool(
                "screenshot",
                "Capture the current page as an image.",
                empty_schema(),
            ),
            tool(
                "set_visibility",
                "Show or hide the floating terminal browser without stopping browser automation.",
                json!({
                    "type": "object",
                    "properties": { "visible": { "type": "boolean" } },
                    "required": ["visible"],
                    "additionalProperties": false
                }),
            ),
            tool(
                "close",
                "Close the terminal browser and discard its temporary browsing profile.",
                empty_schema(),
            ),
        ],
    })]
}

pub(crate) fn dynamic_tool_response(
    result: anyhow::Result<BrowserToolOutput>,
) -> DynamicToolCallResponse {
    match result {
        Ok(BrowserToolOutput::Text(text)) => DynamicToolCallResponse {
            content_items: vec![DynamicToolCallOutputContentItem::InputText { text }],
            success: true,
        },
        Ok(BrowserToolOutput::ImageDataUrl(image_url)) => DynamicToolCallResponse {
            content_items: vec![DynamicToolCallOutputContentItem::InputImage { image_url }],
            success: true,
        },
        Err(err) => DynamicToolCallResponse {
            content_items: vec![DynamicToolCallOutputContentItem::InputText {
                text: format!("Terminal browser action failed: {err:#}")
                    .chars()
                    .take(/*n*/ 4_000)
                    .collect(),
            }],
            success: false,
        },
    }
}

fn tool(
    name: &str,
    description: &str,
    input_schema: serde_json::Value,
) -> DynamicToolNamespaceTool {
    DynamicToolNamespaceTool::Function(DynamicToolFunctionSpec {
        name: name.to_string(),
        description: description.to_string(),
        input_schema,
        defer_loading: true,
    })
}

fn empty_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false
    })
}
