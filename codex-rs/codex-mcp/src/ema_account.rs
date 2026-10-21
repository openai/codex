//! Account-grant eligibility and registration-specific sign-in requirements.
//! The MCP service owns this state across threads; it retains registration policy but no user tokens.

use std::sync::Mutex;

use codex_config::McpEmaRegistration;
use codex_config::McpServerIdpOAuthConfig;
use codex_login::AccountOAuthGrantRevision;
use codex_login::CodexAuth;
use codex_login::TokenData;
use codex_rmcp_client::account_oidc_identity_is_usable;
use codex_rmcp_client::account_oidc_issuer_hint;

/// Remembers a rejected account credential only for the affected EMA registration.
#[derive(Default)]
pub struct EmaState {
    rejected: Mutex<Vec<(McpEmaRegistration, AccountOAuthGrantRevision)>>,
}

impl std::fmt::Debug for EmaState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmaState").finish_non_exhaustive()
    }
}

impl EmaState {
    pub(crate) fn requires_sign_in(
        &self,
        registration: &McpEmaRegistration,
        tokens: &TokenData,
    ) -> bool {
        let revision = AccountOAuthGrantRevision::from(tokens);
        self.rejected
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .any(|(registered, rejected)| registered == registration && rejected == &revision)
    }

    pub(crate) fn reject(&self, registration: &McpEmaRegistration, tokens: &TokenData) {
        let mut rejected = self
            .rejected
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Replacing a registration's marker also bounds repeated refresh/rejection cycles.
        rejected.retain(|(registered, _)| registered != registration);
        rejected.push((
            registration.clone(),
            AccountOAuthGrantRevision::from(tokens),
        ));
    }

    pub(crate) fn account_is_usable(
        &self,
        auth: &CodexAuth,
        registration: &McpEmaRegistration,
    ) -> bool {
        matches!(auth, CodexAuth::Chatgpt(_))
            && auth.get_token_data().is_ok_and(|tokens| {
                account_grant_is_usable(&tokens, registration.idp())
                    && !self.requires_sign_in(registration, &tokens)
            })
    }
}

pub(crate) fn account_grant_is_usable(tokens: &TokenData, idp: &McpServerIdpOAuthConfig) -> bool {
    tokens.oauth_provenance.as_ref().is_some_and(|provenance| {
        provenance.issuer == idp.issuer
            // A provisioned secret belongs to the configured client, not another
            // primary login client at the same IdP. Use dedicated login in that case.
            && (idp.client_secret.is_none() || provenance.client_id == idp.client_id)
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
