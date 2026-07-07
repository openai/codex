use codex_app_server_protocol::DynamicToolCallOutputContentItem;
use codex_app_server_protocol::DynamicToolCallResponse;
use codex_app_server_protocol::DynamicToolFunctionSpec;
use codex_app_server_protocol::DynamicToolNamespaceSpec;
use codex_app_server_protocol::DynamicToolNamespaceTool;
use codex_app_server_protocol::DynamicToolSpec;
use codex_terminal_browser::BrowserToolOutput;
use codex_terminal_browser::classify_browser_error;
use serde_json::json;

pub(crate) const TERMINAL_BROWSER_NAMESPACE: &str = "terminal_browser";

pub(crate) fn dynamic_tool_specs() -> Vec<DynamicToolSpec> {
    vec![DynamicToolSpec::Namespace(DynamicToolNamespaceSpec {
        name: TERMINAL_BROWSER_NAMESPACE.to_string(),
        description: "Control the browser rendered inside the terminal. Use these tools when the user asks for the terminal browser; do not substitute shell text browsers."
            .to_string(),
        tools: vec![
            eager_tool(
                "open",
                "Open a URL in the browser panel rendered inside the terminal. Use this tool whenever the user asks for the terminal browser or browser tab; do not substitute web search. An ephemeral profile is active by default, and the user can click the panel to take control.",
                json!({
                    "type": "object",
                    "properties": {
                        "url": {
                            "type": "string",
                            "maxLength": 4096,
                            "description": "HTTP or HTTPS URL to open."
                        },
                        "visible": { "type": "boolean", "description": "Whether to show the browser panel." },
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
                "navigate",
                "Navigate the current tab with a URL, browser history, or reload and wait for a bounded load state.",
                json!({
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["goto", "back", "forward", "reload"] },
                        "url": {
                            "type": "string",
                            "maxLength": 4096,
                            "description": "Required only for goto; must use HTTP or HTTPS."
                        },
                        "waitUntil": {
                            "type": "string",
                            "enum": ["domContentLoaded", "load"],
                            "default": "load"
                        },
                        "timeoutMs": { "type": "integer", "minimum": 1, "maximum": 30000 }
                    },
                    "required": ["action"],
                    "additionalProperties": false
                }),
            ),
            tool(
                "wait",
                "Wait up to 30 seconds for a URL, load state, page text, or snapshot node condition.",
                json!({
                    "oneOf": [
                        {
                            "type": "object",
                            "properties": {
                                "type": { "const": "url" },
                                "value": { "type": "string", "maxLength": 4096 },
                                "match": { "type": "string", "enum": ["exact", "contains"], "default": "exact" },
                                "timeoutMs": { "type": "integer", "minimum": 1, "maximum": 30000 }
                            },
                            "required": ["type", "value"],
                            "additionalProperties": false
                        },
                        {
                            "type": "object",
                            "properties": {
                                "type": { "const": "loadState" },
                                "state": { "type": "string", "enum": ["domContentLoaded", "load"] },
                                "timeoutMs": { "type": "integer", "minimum": 1, "maximum": 30000 }
                            },
                            "required": ["type", "state"],
                            "additionalProperties": false
                        },
                        {
                            "type": "object",
                            "properties": {
                                "type": { "const": "text" },
                                "value": { "type": "string", "maxLength": 4096 },
                                "state": { "type": "string", "enum": ["present", "absent"] },
                                "timeoutMs": { "type": "integer", "minimum": 1, "maximum": 30000 }
                            },
                            "required": ["type", "value", "state"],
                            "additionalProperties": false
                        },
                        {
                            "type": "object",
                            "properties": {
                                "type": { "const": "node" },
                                "nodeId": { "type": "string", "pattern": "^d[0-9a-f]{16}n[0-9]{1,20}$" },
                                "state": { "type": "string", "enum": ["visible", "hidden", "attached", "detached"] },
                                "timeoutMs": { "type": "integer", "minimum": 1, "maximum": 30000 }
                            },
                            "required": ["type", "nodeId", "state"],
                            "additionalProperties": false
                        }
                    ]
                }),
            ),
            tool(
                "profile",
                "List profiles or request a user-approved create, select, ephemeral, or forget operation. Profile changes are not required before opening a URL; an ephemeral profile is active by default.",
                json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["list", "requestCreate", "requestSelect", "requestEphemeral", "requestForget"]
                        },
                        "name": {
                            "type": "string",
                            "minLength": 1,
                            "maxLength": 64,
                            "pattern": "^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$"
                        }
                    },
                    "required": ["action"],
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
                "Show or hide the terminal browser panel without stopping browser automation.",
                json!({
                    "type": "object",
                    "properties": { "visible": { "type": "boolean" } },
                    "required": ["visible"],
                    "additionalProperties": false
                }),
            ),
            tool(
                "close",
                "Close the terminal browser. Ephemeral profile data is discarded; named profiles are retained.",
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
                text: serde_json::to_string(&json!({
                    "error": classify_browser_error(&err),
                }))
                .unwrap_or_else(|_| {
                    "{\"error\":{\"code\":\"internal\",\"message\":\"the terminal-browser action failed\",\"retryable\":false}}".to_string()
                }),
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
    dynamic_tool(name, description, input_schema, /*defer_loading*/ true)
}

fn eager_tool(
    name: &str,
    description: &str,
    input_schema: serde_json::Value,
) -> DynamicToolNamespaceTool {
    dynamic_tool(
        name,
        description,
        input_schema,
        /*defer_loading*/ false,
    )
}

fn dynamic_tool(
    name: &str,
    description: &str,
    input_schema: serde_json::Value,
    defer_loading: bool,
) -> DynamicToolNamespaceTool {
    DynamicToolNamespaceTool::Function(DynamicToolFunctionSpec {
        name: name.to_string(),
        description: description.to_string(),
        input_schema,
        defer_loading,
    })
}

fn empty_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false
    })
}
