use std::collections::HashMap;

use pretty_assertions::assert_eq;
use serde_json::json;

use super::*;

#[test]
fn selects_only_supported_mcp_extensions() {
    let app_ui = json!({
        "mimeTypes": [
            "text/html;profile=mcp-app",
            "text/x-dil;profile=mcp-app",
        ],
        "futureField": {"preserved": true},
    });
    let extensions = HashMap::from([
        (MCP_APP_UI_EXTENSION_ID.to_string(), app_ui.clone()),
        (OPENAI_FORM_EXTENSION_ID.to_string(), json!({})),
        (
            OPENAI_STANDARD_FORM_INPUT_EXTENSION_ID.to_string(),
            json!({}),
        ),
        ("example/other".to_string(), json!({"enabled": true})),
    ]);

    assert_eq!(
        client_mcp_extensions(
            Some(&extensions),
            /*legacy_openai_form_elicitation*/ false,
        ),
        ClientMcpExtensions::new(HashMap::from([
            (MCP_APP_UI_EXTENSION_ID.to_string(), app_ui),
            (OPENAI_FORM_EXTENSION_ID.to_string(), json!({})),
            (
                OPENAI_STANDARD_FORM_INPUT_EXTENSION_ID.to_string(),
                json!({}),
            ),
        ]))
    );
}

#[test]
fn normalizes_legacy_form_capability_into_extensions() {
    assert_eq!(
        client_mcp_extensions(
            /*extensions*/ None, /*legacy_openai_form_elicitation*/ true,
        ),
        ClientMcpExtensions::new(HashMap::from([(
            OPENAI_FORM_EXTENSION_ID.to_string(),
            json!({}),
        )]))
    );
}
