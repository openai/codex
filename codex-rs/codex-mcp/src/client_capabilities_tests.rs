use std::collections::HashMap;

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
    let meta = with_mcp_app_ui_client_capabilities_meta(
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
        with_mcp_app_ui_client_capabilities_meta(
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
        with_mcp_app_ui_client_capabilities_meta(
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
