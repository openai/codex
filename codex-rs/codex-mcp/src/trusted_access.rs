use std::sync::Arc;
use std::time::Duration;

use crate::connection_manager::McpConnectionSet;
use crate::runtime::McpRuntimeInput;
use crate::server::McpServerMetadata;
use crate::server::McpServerOrigin;
use crate::tools::ToolInfo;
use codex_exec_server::HttpClient;
use codex_exec_server::HttpHeader;
use codex_exec_server::HttpRedirectPolicy;
use codex_exec_server::HttpRequestParams;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_protocol::auth::AuthMode;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;
use serde_json::json;

pub(crate) const ENTITLEMENT_CONTEXT_KEY: &str = "openai/entitlementContext";
const MAX_VERIFIED_ACCESS_RESPONSE_BYTES: usize = 1024 * 1024;
const REQUESTED_ENTITLEMENTS_KEY: &str = "openai/requestedEntitlements";
const CYBER_TRUSTED_ACCESS_ENTITLEMENT: &str = "cyber_trusted_access";
const TRUSTED_ACCESS_TIMEOUT: Duration = Duration::from_millis(2_500);

impl McpConnectionSet {
    /// Installed and task-selected plugins may request supported advisory entitlement metadata.
    /// Model calls use the local, read-only, zero-argument boundary.
    pub(crate) async fn add_trusted_access_context(
        &self,
        tool: &ToolInfo,
        server: &McpServerMetadata,
        arguments: Option<&Value>,
        meta: Option<Value>,
    ) -> Option<Value> {
        if tool
            .tool
            .meta
            .as_deref()
            .and_then(|meta| meta.get(REQUESTED_ENTITLEMENTS_KEY))
            .and_then(Value::as_array)
            .is_some_and(|entitlements| {
                entitlements.iter().all(Value::is_string)
                    && entitlements.iter().any(|entitlement| {
                        entitlement.as_str() == Some(CYBER_TRUSTED_ACCESS_ENTITLEMENT)
                    })
            })
            && self
                .plugin_id_for_mcp_server_name(&tool.server_name)
                .is_some()
            && arguments.is_none_or(|arguments| arguments.as_object().is_some_and(Map::is_empty))
            && server.environment_id == codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID
            && matches!(server.origin, Some(McpServerOrigin::Stdio))
            && tool
                .tool
                .annotations
                .as_ref()
                .and_then(|annotations| annotations.read_only_hint)
                == Some(true)
            && let Some(context) = self.trusted_access.as_ref()
        {
            context.add_context(meta).await
        } else {
            meta
        }
    }
}

#[derive(Deserialize)]
struct VerifiedAccessResponse {
    programs: Vec<Value>,
}

#[derive(Deserialize)]
struct VerifiedAccessProgram {
    state: VerifiedAccessState,
    grants: Vec<VerifiedAccessGrant>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum VerifiedAccessState {
    Active,
    Inactive,
    Unavailable,
}

#[derive(Deserialize)]
struct VerifiedAccessGrant {
    level: TrustedAccessLevel,
    source: VerifiedAccessSource,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum TrustedAccessLevel {
    Tac1,
    Tac2,
    Tac3,
    Government,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum VerifiedAccessSource {
    Individual,
    Organization,
}

/// Fetches account-bound verified access for trusted, host-owned MCP metadata.
/// Callers must authorize the receiving plugin and tool before attaching it.
pub struct TrustedAccessContext {
    auth: CodexAuth,
    auth_manager: Arc<AuthManager>,
    chatgpt_base_url: String,
    http_client: Arc<dyn HttpClient>,
}

impl TrustedAccessContext {
    pub(crate) fn from_runtime(input: &McpRuntimeInput) -> Option<Self> {
        let auth = input.auth.as_ref()?;
        if !matches!(
            auth.api_auth_mode(),
            AuthMode::Chatgpt | AuthMode::ChatgptAuthTokens
        ) {
            return None;
        }
        Some(Self::new(
            auth.clone(),
            input.auth_manager.clone()?,
            input.config.chatgpt_base_url.clone(),
            input.runtime_context.local_http_client(),
        ))
    }

    pub fn new(
        auth: CodexAuth,
        auth_manager: Arc<AuthManager>,
        chatgpt_base_url: String,
        http_client: Arc<dyn HttpClient>,
    ) -> Self {
        Self {
            auth,
            auth_manager,
            chatgpt_base_url,
            http_client,
        }
    }

    /// Replaces caller-supplied entitlement metadata with a fresh verified result.
    pub async fn add_context(&self, meta: Option<Value>) -> Option<Value> {
        let mut meta = match meta {
            Some(Value::Object(meta)) => meta,
            None => Map::new(),
            other => return other,
        };
        meta.remove(ENTITLEMENT_CONTEXT_KEY);

        let status = tokio::time::timeout(TRUSTED_ACCESS_TIMEOUT, self.fetch_status())
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| {
                json!({
                    "schemaVersion": 1,
                    "status": "unknown",
                    "grants": [],
                    "stale": false
                })
            });
        meta.insert(
            ENTITLEMENT_CONTEXT_KEY.to_string(),
            json!({
                "schemaVersion": 1,
                "entitlements": { "cyber_trusted_access": status }
            }),
        );
        Some(Value::Object(meta))
    }

    async fn fetch_status(&self) -> Option<Value> {
        let auth = self.auth_manager.auth().await?;
        let expected_account_id = self
            .auth
            .get_account_id()
            .filter(|account_id| !account_id.trim().is_empty())?;
        let account_id = auth
            .get_account_id()
            .filter(|account_id| !account_id.trim().is_empty())?;
        if !matches!(
            self.auth.api_auth_mode(),
            AuthMode::Chatgpt | AuthMode::ChatgptAuthTokens
        ) || !matches!(
            auth.api_auth_mode(),
            AuthMode::Chatgpt | AuthMode::ChatgptAuthTokens
        ) || account_id != expected_account_id
            || auth.get_chatgpt_user_id() != self.auth.get_chatgpt_user_id()
            || auth.is_workspace_account() != self.auth.is_workspace_account()
            || auth.is_fedramp_account() != self.auth.is_fedramp_account()
        {
            return None;
        }

        let headers = codex_model_provider::auth_provider_from_auth(&auth)
            .to_auth_headers()
            .ok()?
            .iter()
            .map(|(name, value)| {
                Some(HttpHeader {
                    name: name.as_str().to_string(),
                    value: value.to_str().ok()?.to_string(),
                    value_env_var: None,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let (response, mut response_body) = self
            .http_client
            .http_request_stream(HttpRequestParams {
                method: "GET".to_string(),
                url: format!(
                    "{}/accounts/verified_access",
                    self.chatgpt_base_url.trim_end_matches('/')
                ),
                headers,
                body: None,
                timeout_ms: Some(TRUSTED_ACCESS_TIMEOUT.as_millis() as u64),
                redirect_policy: HttpRedirectPolicy::Stop,
                request_id: "trusted-access-status".to_string(),
                stream_response: true,
            })
            .await
            .ok()?;
        if response.status != 200 {
            return None;
        }
        let mut response_bytes = Vec::new();
        while let Some(chunk) = response_body.recv().await.ok()? {
            if chunk.len() > MAX_VERIFIED_ACCESS_RESPONSE_BYTES - response_bytes.len() {
                return None;
            }
            response_bytes.extend_from_slice(&chunk);
        }
        let response: VerifiedAccessResponse = serde_json::from_slice(&response_bytes).ok()?;
        let mut programs = response
            .programs
            .into_iter()
            .filter(|program| program.get("program").and_then(Value::as_str) == Some("cyber"));
        let program = programs.next()?;
        if programs.next().is_some() {
            return None;
        }
        let program: VerifiedAccessProgram = serde_json::from_value(program).ok()?;

        if self.auth_manager.auth_cached().is_none_or(|current| {
            !matches!(
                current.api_auth_mode(),
                AuthMode::Chatgpt | AuthMode::ChatgptAuthTokens
            ) || current.get_account_id().as_deref() != Some(account_id.as_str())
                || current.get_chatgpt_user_id() != auth.get_chatgpt_user_id()
                || current.is_workspace_account() != auth.is_workspace_account()
                || current.is_fedramp_account() != auth.is_fedramp_account()
        }) {
            return None;
        }

        let status = match program.state {
            VerifiedAccessState::Active if !program.grants.is_empty() => "granted",
            VerifiedAccessState::Inactive if program.grants.is_empty() => "not_granted",
            VerifiedAccessState::Unavailable if program.grants.is_empty() => "unknown",
            VerifiedAccessState::Active
            | VerifiedAccessState::Inactive
            | VerifiedAccessState::Unavailable => return None,
        };
        let grants = program
            .grants
            .into_iter()
            .map(|grant| {
                let source = match grant.source {
                    VerifiedAccessSource::Individual => "user",
                    VerifiedAccessSource::Organization => "current_account",
                };
                json!({ "level": grant.level, "source": source })
            })
            .collect::<Vec<_>>();
        Some(json!({
            "schemaVersion": 1,
            "status": status,
            "grants": grants,
            "stale": false
        }))
    }
}

#[cfg(test)]
#[path = "trusted_access_tests.rs"]
mod tests;
