//! Auth elicitation helpers.
//!
//! Codex Apps owns protocol-neutral auth elicitation parsing and payload shaping.
//! Session orchestration stays in `codex-core`.

use rmcp::model::Content;
use serde::Serialize;

use codex_protocol::mcp::MCP_ERROR_CODE_META_KEY;

pub(crate) const MCP_TOOL_CODEX_APPS_META_KEY: &str = "_codex_apps";
const CONNECTOR_AUTH_FAILURE_META_KEY: &str = "connector_auth_failure";
const CONNECTOR_AUTH_FAILURE_IS_AUTH_FAILURE_KEY: &str = "is_auth_failure";
const CONNECTOR_AUTH_FAILURE_AUTH_REASON_KEY: &str = "auth_reason";
const CONNECTOR_AUTH_FAILURE_CONNECTOR_ID_KEY: &str = "connector_id";
const CONNECTOR_AUTH_FAILURE_LINK_ID_KEY: &str = "link_id";
const CONNECTOR_AUTH_FAILURE_ERROR_CODE_KEY: &str = "error_code";
const CONNECTOR_AUTH_FAILURE_ERROR_HTTP_STATUS_CODE_KEY: &str = "error_http_status_code";
const CONNECTOR_AUTH_FAILURE_ERROR_ACTION_KEY: &str = "error_action";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodexAppsConnectorAuthFailure {
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
pub(crate) struct CodexAppsAuthElicitation {
    pub meta: serde_json::Value,
    pub message: String,
    pub url: String,
    pub elicitation_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CodexAppsAuthElicitationPlan {
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

pub(crate) fn build_auth_elicitation_plan_from_rmcp_result(
    call_id: &str,
    result: &rmcp::model::CallToolResult,
    connector_id: Option<&str>,
    connector_name: Option<&str>,
    install_url: Option<String>,
) -> Option<CodexAppsAuthElicitationPlan> {
    let auth_failure = connector_auth_failure_from_meta(
        result.is_error,
        result.meta.as_ref().map(|meta| &meta.0)?,
        connector_id,
        connector_name,
        install_url,
    )?;
    let elicitation = build_auth_elicitation(call_id, &auth_failure);
    Some(CodexAppsAuthElicitationPlan {
        auth_failure,
        elicitation,
    })
}

/// Copies the hosted auth error code into model-private MCP result metadata.
///
/// Core telemetry consumes the generic metadata field. Keep the Apps envelope private to this
/// proxy and leave model-visible structured content exactly as the upstream tool supplied it.
pub(crate) fn expose_auth_error_code_to_telemetry(result: &mut rmcp::model::CallToolResult) {
    if result.is_error != Some(true) {
        return;
    }
    let Some(auth_failure) = result
        .meta
        .as_ref()
        .and_then(|meta| meta.0.get(MCP_TOOL_CODEX_APPS_META_KEY))
        .and_then(serde_json::Value::as_object)
        .and_then(|apps| apps.get(CONNECTOR_AUTH_FAILURE_META_KEY))
        .and_then(serde_json::Value::as_object)
        .filter(|auth_failure| {
            auth_failure
                .get(CONNECTOR_AUTH_FAILURE_IS_AUTH_FAILURE_KEY)
                .and_then(serde_json::Value::as_bool)
                == Some(true)
        })
    else {
        return;
    };
    let Some(error_code) =
        string_auth_failure_field(auth_failure, CONNECTOR_AUTH_FAILURE_ERROR_CODE_KEY)
    else {
        return;
    };
    result
        .meta
        .get_or_insert_with(rmcp::model::Meta::new)
        .0
        .entry(MCP_ERROR_CODE_META_KEY.to_string())
        .or_insert(serde_json::Value::String(error_code));
}

fn connector_auth_failure_from_meta(
    is_error: Option<bool>,
    meta: &serde_json::Map<String, serde_json::Value>,
    connector_id: Option<&str>,
    connector_name: Option<&str>,
    install_url: Option<String>,
) -> Option<CodexAppsConnectorAuthFailure> {
    if is_error != Some(true) {
        return None;
    }

    let auth_failure = meta
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

fn build_auth_elicitation(
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

pub(crate) fn rmcp_auth_elicitation_completed_result(
    auth_failure: &CodexAppsConnectorAuthFailure,
    original: rmcp::model::CallToolResult,
) -> rmcp::model::CallToolResult {
    let mut result = rmcp::model::CallToolResult::error(vec![Content::text(format!(
        "Authentication for {} was requested and accepted. Retry this tool call now.",
        auth_failure.connector_name
    ))]);
    result.meta = original.meta;
    // Preserve upstream structured content verbatim. Telemetry-only normalization lives in
    // model-private result metadata.
    result.structured_content = original.structured_content;
    result
}

fn auth_elicitation_id(call_id: &str) -> String {
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
#[path = "auth_elicitation_tests.rs"]
mod tests;
