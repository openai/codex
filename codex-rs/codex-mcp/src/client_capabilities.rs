use std::collections::HashMap;

use serde_json::Map;
use serde_json::Value;

const MCP_APP_UI_EXTENSION_ID: &str = "io.modelcontextprotocol/ui";
const MCP_APP_UI_WEBVIEW_MIME_TYPE: &str = "text/html;profile=mcp-app";
const MCP_CLIENT_CAPABILITIES_META_KEY: &str = "io.modelcontextprotocol/clientCapabilities";

/// Returns whether the app-server host can render MCP App WebViews.
///
/// App-server clients may declare unrelated extensions or additional UI MIME
/// types. Codex only forwards this one trusted capability because it is the
/// rendering contract downstream MCP servers need for MCP App widgets.
pub fn supports_mcp_app_ui_webview(extensions: Option<&HashMap<String, Value>>) -> bool {
    extensions
        .and_then(|extensions| extensions.get(MCP_APP_UI_EXTENSION_ID))
        .and_then(Value::as_object)
        .and_then(|settings| settings.get("mimeTypes"))
        .and_then(Value::as_array)
        .is_some_and(|mime_types| {
            mime_types
                .iter()
                .any(|mime_type| mime_type.as_str() == Some(MCP_APP_UI_WEBVIEW_MIME_TYPE))
        })
}

/// Adds a trusted host capability profile to a Codex Apps request and removes
/// any caller-supplied copy of the reserved metadata key.
///
/// An explicit empty profile is important for hosts without WebView support:
/// downstream connector MCP sessions preserve WebView as the legacy default
/// when client capabilities are absent.
pub fn with_mcp_app_ui_client_capabilities_meta(
    meta: Option<Value>,
    supports_mcp_app_ui_webview: bool,
) -> Option<Value> {
    let mut object = match meta {
        Some(Value::Object(mut object)) => {
            object.remove(MCP_CLIENT_CAPABILITIES_META_KEY);
            object
        }
        _ => Map::new(),
    };
    let extensions = if supports_mcp_app_ui_webview {
        serde_json::json!({
            MCP_APP_UI_EXTENSION_ID: {
                "mimeTypes": [MCP_APP_UI_WEBVIEW_MIME_TYPE],
            }
        })
    } else {
        serde_json::json!({})
    };
    object.insert(
        MCP_CLIENT_CAPABILITIES_META_KEY.to_string(),
        serde_json::json!({ "extensions": extensions }),
    );
    Some(Value::Object(object))
}

#[cfg(test)]
#[path = "client_capabilities_tests.rs"]
mod tests;
