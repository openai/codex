//! Validates persisted account grants against the configured enterprise issuer.

use codex_config::McpServerIdpOAuthConfig;
use codex_login::CodexAuth;
use codex_login::TokenData;
use codex_rmcp_client::account_oidc_identity_is_usable;
use codex_rmcp_client::account_oidc_issuer_hint;

pub(crate) fn account_grant_is_usable(tokens: &TokenData, idp: &McpServerIdpOAuthConfig) -> bool {
    tokens.oauth_provenance.as_ref().is_some_and(|provenance| {
        provenance.issuer == idp.issuer
            && tokens
                .login_owner_id
                .as_ref()
                .is_some_and(|id| !id.is_empty())
            && !tokens.refresh_token.trim().is_empty()
            && tokens
                .id_token
                .chatgpt_user_id
                .as_ref()
                .is_some_and(|id| !id.is_empty())
            && tokens.account_id.as_ref().is_some_and(|id| !id.is_empty())
            && tokens.id_token.chatgpt_account_id == tokens.account_id
            && account_oidc_identity_is_usable(
                &tokens.id_token.raw_jwt,
                &provenance.issuer,
                &provenance.client_id,
            )
    })
}

pub(crate) fn account_configuration_issue(
    auth: &CodexAuth,
    idp: &McpServerIdpOAuthConfig,
) -> Option<&'static str> {
    let tokens = auth.get_token_data().ok()?;
    if tokens.login_owner_id.is_none()
        && (tokens
            .oauth_provenance
            .as_ref()
            .is_some_and(|provenance| provenance.issuer == idp.issuer)
            || account_oidc_issuer_hint(&tokens.id_token.raw_jwt, &idp.issuer))
    {
        return Some("Sign in to your Codex account again to enable this enterprise MCP server");
    }
    if tokens.oauth_provenance.as_ref().is_some_and(|provenance| {
        provenance.issuer != idp.issuer
            && provenance.issuer.trim_end_matches('/') == idp.issuer.trim_end_matches('/')
    }) {
        return Some(
            "Codex sign-in and enterprise MCP issuer strings differ by a trailing slash; align their OIDC issuer configuration",
        );
    }
    None
}
