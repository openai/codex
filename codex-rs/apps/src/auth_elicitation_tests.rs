use pretty_assertions::assert_eq;

use super::*;

fn auth_failure_result() -> rmcp::model::CallToolResult {
    let mut result = rmcp::model::CallToolResult::error(vec![Content::text(
        "Connector reauthentication required",
    )]);
    result.meta = Some(rmcp::model::Meta(
        serde_json::from_value(serde_json::json!({
            MCP_TOOL_CODEX_APPS_META_KEY: {
                CONNECTOR_AUTH_FAILURE_META_KEY: {
                    CONNECTOR_AUTH_FAILURE_IS_AUTH_FAILURE_KEY: true,
                    CONNECTOR_AUTH_FAILURE_AUTH_REASON_KEY: "reauthentication_required",
                    CONNECTOR_AUTH_FAILURE_CONNECTOR_ID_KEY: "connector_calendar",
                    "connector_name": "Untrusted Calendar",
                    CONNECTOR_AUTH_FAILURE_LINK_ID_KEY: "link_123",
                    CONNECTOR_AUTH_FAILURE_ERROR_CODE_KEY: "UNAUTHORIZED",
                    CONNECTOR_AUTH_FAILURE_ERROR_HTTP_STATUS_CODE_KEY: 401,
                    CONNECTOR_AUTH_FAILURE_ERROR_ACTION_KEY: "TRIGGER_REAUTHENTICATION",
                },
            },
        }))
        .expect("object metadata"),
    ));
    result
}

#[test]
fn parses_auth_failure_from_trusted_connector_metadata() {
    assert_eq!(
        build_auth_elicitation_plan_from_rmcp_result(
            "call_123",
            &auth_failure_result(),
            Some("connector_calendar"),
            Some("Google Calendar"),
            Some("https://chatgpt.com/apps/google-calendar/connector_calendar".to_string()),
        )
        .map(|plan| plan.auth_failure),
        Some(CodexAppsConnectorAuthFailure {
            connector_id: "connector_calendar".to_string(),
            connector_name: "Google Calendar".to_string(),
            install_url: "https://chatgpt.com/apps/google-calendar/connector_calendar".to_string(),
            auth_reason: Some("reauthentication_required".to_string()),
            link_id: Some("link_123".to_string()),
            error_code: Some("UNAUTHORIZED".to_string()),
            error_http_status_code: Some(401),
            error_action: Some("TRIGGER_REAUTHENTICATION".to_string()),
        })
    );
}

#[test]
fn copies_auth_error_code_to_model_private_metadata() {
    let mut result = auth_failure_result();
    expose_auth_error_code_to_telemetry(&mut result);
    assert_eq!(
        result
            .meta
            .as_ref()
            .and_then(|meta| meta.0.get(MCP_ERROR_CODE_META_KEY)),
        Some(&serde_json::json!("UNAUTHORIZED"))
    );
    assert_eq!(result.structured_content, None);

    result.structured_content = Some(serde_json::json!({
        "error_code": "UPSTREAM_CODE",
        "detail": "safe to preserve",
    }));
    expose_auth_error_code_to_telemetry(&mut result);
    assert_eq!(
        result.structured_content,
        Some(serde_json::json!({
            "error_code": "UPSTREAM_CODE",
            "detail": "safe to preserve",
        }))
    );

    result.structured_content = Some(serde_json::json!("upstream opaque value"));
    expose_auth_error_code_to_telemetry(&mut result);
    assert_eq!(
        result.structured_content,
        Some(serde_json::json!("upstream opaque value"))
    );
}

#[test]
fn accepted_result_preserves_private_error_code_and_structured_content() {
    let mut original = auth_failure_result();
    expose_auth_error_code_to_telemetry(&mut original);
    let plan = build_auth_elicitation_plan_from_rmcp_result(
        "call_123",
        &original,
        Some("connector_calendar"),
        Some("Google Calendar"),
        Some("https://chatgpt.com/apps/google-calendar/connector_calendar".to_string()),
    )
    .expect("auth failure");

    let completed = rmcp_auth_elicitation_completed_result(&plan.auth_failure, original);

    assert_eq!(
        completed.content,
        vec![Content::text(
            "Authentication for Google Calendar was requested and accepted. Retry this tool call now."
        )]
    );
    assert_eq!(completed.structured_content, None);
    assert_eq!(
        completed
            .meta
            .as_ref()
            .and_then(|meta| meta.0.get(MCP_ERROR_CODE_META_KEY)),
        Some(&serde_json::json!("UNAUTHORIZED"))
    );

    let mut opaque = auth_failure_result();
    opaque.structured_content = Some(serde_json::json!("upstream opaque value"));
    expose_auth_error_code_to_telemetry(&mut opaque);
    let completed = rmcp_auth_elicitation_completed_result(&plan.auth_failure, opaque);
    assert_eq!(
        completed.structured_content,
        Some(serde_json::json!("upstream opaque value"))
    );
}

#[test]
fn does_not_expose_unverified_auth_metadata() {
    let mut result = auth_failure_result();
    result
        .meta
        .as_mut()
        .and_then(|meta| meta.0.get_mut(MCP_TOOL_CODEX_APPS_META_KEY))
        .and_then(serde_json::Value::as_object_mut)
        .and_then(|apps| apps.get_mut(CONNECTOR_AUTH_FAILURE_META_KEY))
        .and_then(serde_json::Value::as_object_mut)
        .expect("auth failure metadata")
        .insert(
            CONNECTOR_AUTH_FAILURE_IS_AUTH_FAILURE_KEY.to_string(),
            serde_json::Value::Bool(false),
        );

    expose_auth_error_code_to_telemetry(&mut result);
    assert_eq!(result.structured_content, None);
    assert!(
        result
            .meta
            .as_ref()
            .and_then(|meta| meta.0.get(MCP_ERROR_CODE_META_KEY))
            .is_none()
    );
}

#[test]
fn rejects_missing_or_mismatched_connector_ids() {
    assert_eq!(
        build_auth_elicitation_plan_from_rmcp_result(
            "call_123",
            &auth_failure_result(),
            /*connector_id*/ None,
            Some("Google Calendar"),
            Some("https://chatgpt.com/apps/google-calendar/connector_calendar".to_string()),
        ),
        None
    );
    assert_eq!(
        build_auth_elicitation_plan_from_rmcp_result(
            "call_123",
            &auth_failure_result(),
            Some("connector_drive"),
            Some("Google Drive"),
            Some("https://chatgpt.com/apps/google-drive/connector_drive".to_string()),
        ),
        None
    );
}

#[test]
fn builds_url_elicitation_payload() {
    let plan = build_auth_elicitation_plan_from_rmcp_result(
        "call_123",
        &auth_failure_result(),
        Some("connector_calendar"),
        Some("Google Calendar"),
        Some("https://chatgpt.com/apps/google-calendar/connector_calendar".to_string()),
    )
    .expect("auth failure");

    assert_eq!(
        plan.elicitation,
        CodexAppsAuthElicitation {
            meta: serde_json::json!({
                MCP_TOOL_CODEX_APPS_META_KEY: {
                    CONNECTOR_AUTH_FAILURE_META_KEY: {
                        CONNECTOR_AUTH_FAILURE_IS_AUTH_FAILURE_KEY: true,
                        CONNECTOR_AUTH_FAILURE_CONNECTOR_ID_KEY: "connector_calendar",
                        "connector_name": "Google Calendar",
                        "install_url":
                            "https://chatgpt.com/apps/google-calendar/connector_calendar",
                        CONNECTOR_AUTH_FAILURE_AUTH_REASON_KEY: "reauthentication_required",
                        CONNECTOR_AUTH_FAILURE_LINK_ID_KEY: "link_123",
                        CONNECTOR_AUTH_FAILURE_ERROR_CODE_KEY: "UNAUTHORIZED",
                        CONNECTOR_AUTH_FAILURE_ERROR_HTTP_STATUS_CODE_KEY: 401,
                        CONNECTOR_AUTH_FAILURE_ERROR_ACTION_KEY: "TRIGGER_REAUTHENTICATION",
                    },
                },
            }),
            message: "Reconnect Google Calendar on ChatGPT to restore access for this request."
                .to_string(),
            url: "https://chatgpt.com/apps/google-calendar/connector_calendar".to_string(),
            elicitation_id: "codex_apps_auth_call_123".to_string(),
        }
    );
}
