use std::collections::HashMap;

use anyhow::Result;
use async_channel::Sender;
use codex_api::SharedAuthProvider;
use codex_config::McpServerAuth;
use codex_config::McpServerConfig;
use codex_config::McpServerTransportConfig;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::McpStartupFailureReason;
use codex_protocol::protocol::McpStartupUpdateEvent;
use codex_rmcp_client::McpAuthState;
use codex_rmcp_client::McpLoginRequirement;

use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::rmcp_client::DEFAULT_STARTUP_TIMEOUT;
use crate::rmcp_client::StartupOutcomeError;
use crate::server::EffectiveMcpServer;

/// Makes ChatGPT authentication available to servers that explicitly opt in.
pub(super) fn chatgpt_auth_provider_for_server(
    server: &EffectiveMcpServer,
    chatgpt_auth_provider: Option<SharedAuthProvider>,
) -> Option<SharedAuthProvider> {
    if !matches!(&server.config().auth, McpServerAuth::ChatGpt) {
        return None;
    }
    chatgpt_auth_provider
}

pub(super) fn should_share_codex_apps_tools_cache(
    server_name: &str,
    uses_env_bearer_token: bool,
) -> bool {
    server_name == CODEX_APPS_MCP_SERVER_NAME && !uses_env_bearer_token
}

pub(super) async fn emit_update(
    submit_id: &str,
    tx_event: &Sender<Event>,
    update: McpStartupUpdateEvent,
) -> Result<(), async_channel::SendError<Event>> {
    tx_event
        .send(Event {
            id: submit_id.to_string(),
            msg: EventMsg::McpStartupUpdate(update),
        })
        .await
}

pub(super) fn mcp_startup_failure_reason(
    auth_state: Option<McpAuthState>,
    error: &StartupOutcomeError,
) -> Option<McpStartupFailureReason> {
    if !error.is_authentication_required() {
        return None;
    }
    match auth_state {
        Some(McpAuthState::LoggedOut(McpLoginRequirement::Reauthentication)) => {
            Some(McpStartupFailureReason::ReauthenticationRequired)
        }
        Some(
            McpAuthState::Unsupported
            | McpAuthState::LoggedOut(McpLoginRequirement::Login)
            | McpAuthState::BearerToken
            | McpAuthState::OAuth,
        )
        | None => None,
    }
}

pub(super) fn mcp_init_error_display(
    server_name: &str,
    config: Option<&McpServerConfig>,
    error: &StartupOutcomeError,
) -> String {
    if let Some(McpServerTransportConfig::StreamableHttp {
        url,
        bearer_token_env_var,
        http_headers,
        ..
    }) = config.map(|config| &config.transport)
        && url == "https://api.githubcopilot.com/mcp/"
        && bearer_token_env_var.is_none()
        && http_headers.as_ref().map(HashMap::is_empty).unwrap_or(true)
    {
        format!(
            "GitHub MCP does not support OAuth. Log in by adding a personal access token (https://github.com/settings/personal-access-tokens) to your environment and config.toml:\n[mcp_servers.{server_name}]\nbearer_token_env_var = CODEX_GITHUB_PERSONAL_ACCESS_TOKEN"
        )
    } else if matches!(
        error,
        StartupOutcomeError::Failed { error, .. } if error.contains("Auth required")
    ) {
        format!(
            "The {server_name} MCP server is not logged in. Run `codex mcp login {server_name}`."
        )
    } else if matches!(
        error,
        StartupOutcomeError::Failed { error, .. }
            if error.contains("request timed out")
                || error.contains("timed out handshaking with MCP server")
                || error.contains("MCP client startup timed out")
    ) {
        let startup_timeout_secs = config
            .and_then(|config| config.startup_timeout_sec)
            .unwrap_or(DEFAULT_STARTUP_TIMEOUT)
            .as_secs();
        format!(
            "MCP client for `{server_name}` timed out after {startup_timeout_secs} seconds. Add or adjust `startup_timeout_sec` in your config.toml:\n[mcp_servers.{server_name}]\nstartup_timeout_sec = XX"
        )
    } else {
        format!("MCP client for `{server_name}` failed to start: {error:#}")
    }
}
