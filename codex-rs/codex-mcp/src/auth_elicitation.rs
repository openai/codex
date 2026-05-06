//! Auth elicitation helpers.
//!
//! This module owns protocol-neutral auth elicitation parsing and payload shaping.
//! Session orchestration stays in `codex-core`.

use codex_protocol::mcp::CallToolResult;
use serde::Serialize;

pub const MCP_TOOL_CODEX_APPS_META_KEY: &str = "_codex_apps";
pub const CONNECTOR_AUTH_FAILURE_META_KEY: &str = "connector_auth_failure";
pub const CONNECTOR_AUTH_FAILURE_IS_AUTH_FAILURE_KEY: &str = "is_auth_failure";
pub const CONNECTOR_AUTH_FAILURE_AUTH_REASON_KEY: &str = "auth_reason";
pub const CONNECTOR_AUTH_FAILURE_CONNECTOR_ID_KEY: &str = "connector_id";
pub const CONNECTOR_AUTH_FAILURE_LINK_ID_KEY: &str = "link_id";
pub const CONNECTOR_AUTH_FAILURE_ERROR_CODE_KEY: &str = "error_code";
pub const CONNECTOR_AUTH_FAILURE_ERROR_HTTP_STATUS_CODE_KEY: &str = "error_http_status_code";
pub const CONNECTOR_AUTH_FAILURE_ERROR_ACTION_KEY: &str = "error_action";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexAppsConnectorAuthFailure {
    pub connector_id: String,
    pub connector_name: String,
    pub install_url: String,
    pub auth_reason: Option<String>,
    pub link_id: Option<String>,
    pub error_code: Option<String>,
    pub error_http_status_code: Option<i64>,
    pub error_action: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodexAppsAuthElicitation {
    pub meta: serde_json::Value,
    pub message: String,
    pub url: String,
    pub elicitation_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodexAppsAuthElicitationPlan {
    pub auth_failure: CodexAppsConnectorAuthFailure,
    pub elicitation: CodexAppsAuthElicitation,
}

#[derive(Serialize)]
struct CodexAppsConnectorAuthFailureMeta<'a> {
    is_auth_failure: bool,
    connector_id: &'a str,
    connector_name: &'a str,
    install_url: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth_reason: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    link_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_http_status_code: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_action: Option<&'a str>,
}

pub fn connector_auth_failure_from_tool_result(
    result: &CallToolResult,
    connector_id: Option<&str>,
    connector_name: Option<&str>,
    install_url: Option<String>,
) -> Option<CodexAppsConnectorAuthFailure> {
    if result.is_error != Some(true) {
        return None;
    }

    let auth_failure = result
        .meta
        .as_ref()?
        .as_object()?
        .get(MCP_TOOL_CODEX_APPS_META_KEY)?
        .as_object()?
        .get(CONNECTOR_AUTH_FAILURE_META_KEY)?
        .as_object()?;
    if auth_failure
        .get(CONNECTOR_AUTH_FAILURE_IS_AUTH_FAILURE_KEY)
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        return None;
    }

    let connector_id = connector_id
        .map(str::trim)
        .filter(|connector_id| !connector_id.is_empty())?;
    if let Some(auth_failure_connector_id) =
        string_auth_failure_field(auth_failure, CONNECTOR_AUTH_FAILURE_CONNECTOR_ID_KEY)
        && auth_failure_connector_id != connector_id
    {
        return None;
    }
    let connector_name = connector_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(connector_id)
        .to_string();

    Some(CodexAppsConnectorAuthFailure {
        connector_id: connector_id.to_string(),
        connector_name,
        install_url: install_url?,
        auth_reason: string_auth_failure_field(
            auth_failure,
            CONNECTOR_AUTH_FAILURE_AUTH_REASON_KEY,
        ),
        link_id: string_auth_failure_field(auth_failure, CONNECTOR_AUTH_FAILURE_LINK_ID_KEY),
        error_code: string_auth_failure_field(auth_failure, CONNECTOR_AUTH_FAILURE_ERROR_CODE_KEY),
        error_http_status_code: auth_failure
            .get(CONNECTOR_AUTH_FAILURE_ERROR_HTTP_STATUS_CODE_KEY)
            .and_then(serde_json::Value::as_i64),
        error_action: string_auth_failure_field(
            auth_failure,
            CONNECTOR_AUTH_FAILURE_ERROR_ACTION_KEY,
        ),
    })
}

pub fn build_auth_elicitation_plan(
    call_id: &str,
    result: &CallToolResult,
    connector_id: Option<&str>,
    connector_name: Option<&str>,
    install_url: Option<String>,
) -> Option<CodexAppsAuthElicitationPlan> {
    let auth_failure =
        connector_auth_failure_from_tool_result(result, connector_id, connector_name, install_url)?;
    let elicitation = build_auth_elicitation(call_id, &auth_failure);
    Some(CodexAppsAuthElicitationPlan {
        auth_failure,
        elicitation,
    })
}

pub fn build_auth_elicitation(
    call_id: &str,
    auth_failure: &CodexAppsConnectorAuthFailure,
) -> CodexAppsAuthElicitation {
    CodexAppsAuthElicitation {
        meta: serde_json::json!({
            MCP_TOOL_CODEX_APPS_META_KEY: {
                CONNECTOR_AUTH_FAILURE_META_KEY: CodexAppsConnectorAuthFailureMeta {
                    is_auth_failure: true,
                    connector_id: &auth_failure.connector_id,
                    connector_name: &auth_failure.connector_name,
                    install_url: &auth_failure.install_url,
                    auth_reason: auth_failure.auth_reason.as_deref(),
                    link_id: auth_failure.link_id.as_deref(),
                    error_code: auth_failure.error_code.as_deref(),
                    error_http_status_code: auth_failure.error_http_status_code,
                    error_action: auth_failure.error_action.as_deref(),
                },
            },
        }),
        message: auth_elicitation_message(auth_failure),
        url: auth_failure.install_url.clone(),
        elicitation_id: auth_elicitation_id(call_id),
    }
}

pub fn auth_elicitation_completed_result(
    auth_failure: &CodexAppsConnectorAuthFailure,
    meta: Option<serde_json::Value>,
) -> CallToolResult {
    CallToolResult {
        content: vec![serde_json::json!({
            "type": "text",
            "text": format!(
                "Authentication for {} was requested and accepted. Retry this tool call now.",
                auth_failure.connector_name
            ),
        })],
        structured_content: None,
        is_error: Some(true),
        meta,
    }
}

pub fn auth_elicitation_id(call_id: &str) -> String {
    format!("codex_apps_auth_{call_id}")
}

fn string_auth_failure_field(
    auth_failure: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<String> {
    auth_failure
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn auth_elicitation_message(auth_failure: &CodexAppsConnectorAuthFailure) -> String {
    match auth_failure.auth_reason.as_deref() {
        Some("oauth_upgrade_required") => format!(
            "Reconnect {} on ChatGPT to grant the permissions needed for this request.",
            auth_failure.connector_name
        ),
        Some("reauthentication_required") => format!(
            "Reconnect {} on ChatGPT to restore access for this request.",
            auth_failure.connector_name
        ),
        Some("missing_link") => format!(
            "Sign in to {} on ChatGPT to use it in Codex.",
            auth_failure.connector_name
        ),
        _ => format!(
            "Sign in to {} on ChatGPT to continue.",
            auth_failure.connector_name
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn auth_failure_result() -> CallToolResult {
        CallToolResult {
            content: vec![serde_json::json!({
                "type": "text",
                "text": "Connector reauthentication required",
            })],
            structured_content: None,
            is_error: Some(true),
            meta: Some(serde_json::json!({
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
            })),
        }
    }

    #[test]
    fn parses_auth_failure_from_trusted_connector_metadata() {
        assert_eq!(
            connector_auth_failure_from_tool_result(
                &auth_failure_result(),
                Some("connector_calendar"),
                Some("Google Calendar"),
                Some("https://chatgpt.com/apps/google-calendar/connector_calendar".to_string()),
            ),
            Some(CodexAppsConnectorAuthFailure {
                connector_id: "connector_calendar".to_string(),
                connector_name: "Google Calendar".to_string(),
                install_url: "https://chatgpt.com/apps/google-calendar/connector_calendar"
                    .to_string(),
                auth_reason: Some("reauthentication_required".to_string()),
                link_id: Some("link_123".to_string()),
                error_code: Some("UNAUTHORIZED".to_string()),
                error_http_status_code: Some(401),
                error_action: Some("TRIGGER_REAUTHENTICATION".to_string()),
            })
        );
    }

    #[test]
    fn rejects_missing_or_mismatched_connector_ids() {
        assert_eq!(
            connector_auth_failure_from_tool_result(
                &auth_failure_result(),
                /*connector_id*/ None,
                Some("Google Calendar"),
                Some("https://chatgpt.com/apps/google-calendar/connector_calendar".to_string()),
            ),
            None
        );
        assert_eq!(
            connector_auth_failure_from_tool_result(
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
        let auth_failure = connector_auth_failure_from_tool_result(
            &auth_failure_result(),
            Some("connector_calendar"),
            Some("Google Calendar"),
            Some("https://chatgpt.com/apps/google-calendar/connector_calendar".to_string()),
        )
        .expect("auth failure");

        assert_eq!(
            build_auth_elicitation("call_123", &auth_failure),
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

    #[test]
    fn builds_auth_elicitation_plan() {
        let plan = build_auth_elicitation_plan(
            "call_123",
            &auth_failure_result(),
            Some("connector_calendar"),
            Some("Google Calendar"),
            Some("https://chatgpt.com/apps/google-calendar/connector_calendar".to_string()),
        )
        .expect("auth elicitation plan");

        assert_eq!(plan.auth_failure.connector_name, "Google Calendar");
        assert_eq!(plan.elicitation.elicitation_id, "codex_apps_auth_call_123");
    }
}
