//! Test-only helpers exposed for cross-crate integration tests.
//!
//! Production code should receive an [`AuthRouteConfig`](crate::AuthRouteConfig) adapted from the
//! application's resolved HTTP client factory instead of depending on this module.

use crate::AuthManager;
use crate::AuthRouteConfig;
use crate::CodexAuth;
use crate::auth::AgentIdentityAuth;
use crate::auth::AgentIdentityAuthRecord;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_protocol::account::PlanType as AccountPlanType;
use std::sync::Arc;

/// Creates an authentication manager with optional credentials for cross-crate tests.
pub fn auth_manager_from_optional_auth(auth: Option<CodexAuth>) -> Arc<AuthManager> {
    AuthManager::from_optional_auth_for_testing(auth)
}

/// Creates an authentication manager with initialized Agent Identity credentials for tests.
pub async fn auth_manager_with_agent_identity() -> std::io::Result<Arc<AuthManager>> {
    let key_material =
        codex_agent_identity::generate_agent_key_material().map_err(std::io::Error::other)?;
    let auth = AgentIdentityAuth::from_record(
        AgentIdentityAuthRecord {
            agent_runtime_id: "test-agent-runtime-id".to_string(),
            agent_private_key: key_material.private_key_pkcs8_base64,
            account_id: "test-account-id".to_string(),
            chatgpt_user_id: "test-user-id".to_string(),
            email: Some("test-user@example.com".to_string()),
            plan_type: AccountPlanType::Pro,
            chatgpt_account_is_fedramp: false,
            task_id: Some("test-task-id".to_string()),
        },
        "https://unused.invalid",
        &transport_default_auth_route_config(),
    )
    .await?;
    Ok(auth_manager_from_optional_auth(Some(
        CodexAuth::AgentIdentity(auth),
    )))
}

/// Returns auth routing that preserves the transport's built-in proxy behavior.
pub fn transport_default_auth_route_config() -> AuthRouteConfig {
    AuthRouteConfig::from_http_client_factory(HttpClientFactory::new(
        OutboundProxyPolicy::ReqwestDefault,
    ))
}
