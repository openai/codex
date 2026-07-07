use std::collections::HashMap;

use rmcp::model::ExtensionCapabilities;
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

pub(crate) fn mcp_app_ui_extensions() -> ExtensionCapabilities {
    let mut settings = Map::new();
    settings.insert(
        "mimeTypes".to_string(),
        serde_json::json!([MCP_APP_UI_WEBVIEW_MIME_TYPE]),
    );
    [(MCP_APP_UI_EXTENSION_ID.to_string(), settings)]
        .into_iter()
        .collect()
}

/// Adds trusted host capabilities to a Codex Apps request and removes any
/// caller-supplied copy of the reserved metadata key.
pub(crate) fn with_client_capabilities_meta(
    meta: Option<Value>,
    supports_mcp_app_ui_webview: bool,
) -> Option<Value> {
    let meta = match meta {
        Some(Value::Object(mut object)) => {
            object.remove(MCP_CLIENT_CAPABILITIES_META_KEY);
            Some(Value::Object(object))
        }
        other => other,
    };
    if !supports_mcp_app_ui_webview {
        return meta;
    }
    let extensions = mcp_app_ui_extensions();
    let capabilities = serde_json::json!({ "extensions": extensions });
    let mut object = match meta {
        Some(Value::Object(object)) => object,
        _ => Map::new(),
    };
    object.insert(MCP_CLIENT_CAPABILITIES_META_KEY.to_string(), capabilities);
    Some(Value::Object(object))
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::*;

    #[test]
    fn recognizes_only_the_webview_mime_type() {
        let extensions = HashMap::from([
            (
                MCP_APP_UI_EXTENSION_ID.to_string(),
                json!({
                    "mimeTypes": [
                        MCP_APP_UI_WEBVIEW_MIME_TYPE,
                        "text/x-dil;profile=mcp-app",
                    ],
                }),
            ),
            ("example/other".to_string(), json!({"enabled": true})),
        ]);

        assert!(supports_mcp_app_ui_webview(Some(&extensions)));
        assert!(!supports_mcp_app_ui_webview(Some(&HashMap::from([(
            MCP_APP_UI_EXTENSION_ID.to_string(),
            json!({"mimeTypes": ["text/x-dil;profile=mcp-app"]}),
        ),]))));
        assert!(!supports_mcp_app_ui_webview(Some(&HashMap::from([(
            MCP_APP_UI_EXTENSION_ID.to_string(),
            json!("invalid")
        ),]))));
    }

    #[test]
    fn trusted_capabilities_replace_spoofed_request_metadata() {
        let meta = with_client_capabilities_meta(
            Some(json!({
                MCP_CLIENT_CAPABILITIES_META_KEY: {"spoofed": true},
                "caller": "preserved",
            })),
            /*supports_mcp_app_ui_webview*/ true,
        );

        assert_eq!(
            meta,
            Some(json!({
                "caller": "preserved",
                MCP_CLIENT_CAPABILITIES_META_KEY: {
                    "extensions": {
                        MCP_APP_UI_EXTENSION_ID: {
                            "mimeTypes": [
                                MCP_APP_UI_WEBVIEW_MIME_TYPE,
                            ]
                        }
                    }
                }
            }))
        );
    }

    #[test]
    fn absent_host_capabilities_remove_spoofed_request_metadata() {
        assert_eq!(
            with_client_capabilities_meta(
                Some(json!({
                    MCP_CLIENT_CAPABILITIES_META_KEY: {"spoofed": true},
                    "caller": "preserved",
                })),
                /*supports_mcp_app_ui_webview*/ false,
            ),
            Some(json!({"caller": "preserved"}))
        );
    }

    #[test]
    fn invalid_caller_metadata_does_not_suppress_trusted_capabilities() {
        assert_eq!(
            with_client_capabilities_meta(
                Some(json!("invalid")),
                /*supports_mcp_app_ui_webview*/ true,
            ),
            Some(json!({
                MCP_CLIENT_CAPABILITIES_META_KEY: {
                    "extensions": {
                        MCP_APP_UI_EXTENSION_ID: {
                            "mimeTypes": [MCP_APP_UI_WEBVIEW_MIME_TYPE],
                        }
                    }
                }
            }))
        );
    }
}
