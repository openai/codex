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
    let form = json!({"futureField": {"preserved": true}});
    let extensions = HashMap::from([
        (
            OPENAI_ELICITATION_EXTENSION_ID.to_string(),
            json!({"form": form, "userVerification": {}, "unsupported": {}}),
        ),
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
            (
                OPENAI_ELICITATION_EXTENSION_ID.to_string(),
                json!({"form": form}),
            ),
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

#[test]
fn user_verification_is_projected_only_to_the_host_owned_plugin_service() {
    use crate::catalog::McpServerRegistration;
    use crate::catalog::ResolvedMcpCatalog;
    use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
    use crate::mcp::codex_apps_mcp_server_config;
    use codex_rmcp_client::McpProtocolMode;

    let config = codex_apps_mcp_server_config(
        "https://example.com",
        /*apps_mcp_product_sku*/ None,
        /*originator*/ None,
    );
    let extensions = ClientMcpExtensions::new([(
        OPENAI_ELICITATION_EXTENSION_ID.to_string(),
        json!({"form": {}, "userVerification": {}}),
    )]);
    for (name, registration, settings) in [
        (
            CODEX_APPS_MCP_SERVER_NAME,
            McpServerRegistration::from_hosted_apps(
                "host",
                /*contribution_order*/ 0,
                config.clone(),
            ),
            json!({"form": {}, "userVerification": {}}),
        ),
        (
            CODEX_APPS_MCP_SERVER_NAME,
            McpServerRegistration::from_hosted_apps(
                "host",
                /*contribution_order*/ 0,
                config.clone(),
            )
            .with_protocol_mode(McpProtocolMode::V20260728),
            json!({"form": {}, "userVerification": {}}),
        ),
        (
            CODEX_APPS_MCP_SERVER_NAME,
            McpServerRegistration::from_extension(
                CODEX_APPS_MCP_SERVER_NAME.into(),
                "ordinary",
                /*contribution_order*/ 0,
                config.clone(),
            )
            .with_protocol_mode(McpProtocolMode::V20260728),
            json!({"form": {}}),
        ),
        (
            CODEX_APPS_MCP_SERVER_NAME,
            McpServerRegistration::from_config(CODEX_APPS_MCP_SERVER_NAME.into(), config.clone()),
            json!({"form": {}}),
        ),
        (
            "attached",
            McpServerRegistration::from_config("attached".into(), config),
            json!({"form": {}}),
        ),
    ] {
        let mut catalog = ResolvedMcpCatalog::builder();
        catalog.register(registration);
        let catalog = catalog.build();
        assert_eq!(
            server_mcp_extensions(&extensions, name, catalog.server(name)),
            ClientMcpExtensions::new([(OPENAI_ELICITATION_EXTENSION_ID.to_string(), settings)]),
        );
    }
}
