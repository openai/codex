//! Reread enterprise credentials without changing the connection's pinned identity.

use anyhow::Result;
use anyhow::bail;
use codex_keyring_store::DefaultKeyringStore;
use codex_keyring_store::KeyringStore;

use super::ResolvedOAuthCredentialStore;
use super::StoredOAuthCredentialSnapshot;
use super::StoredOAuthTokens;
use crate::ema_auth_policy::ema_reauthentication_required;
use crate::ema_claims::OidcClaims;
use crate::ema_claims::oidc_identity;

pub(crate) fn stored_oidc_identity(tokens: &StoredOAuthTokens) -> Result<OidcClaims> {
    let assertion = tokens
        .token_response
        .0
        .extra_fields()
        .0
        .get("id_token")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            ema_reauthentication_required(
                "enterprise IdP session has no OIDC ID token; sign in again",
            )
        })?;
    // The ID token binds the login identity. Its expiry does not determine
    // whether the IdP will accept the independently valid refresh token.
    oidc_identity(assertion, &tokens.url, &tokens.client_id).map_err(|error| {
        ema_reauthentication_required("stored enterprise IdP identity is invalid; sign in again")
            .context(error.to_string())
    })
}

impl StoredOAuthCredentialSnapshot {
    /// Reject deletion or replacement before using this session's cached resource bearer.
    /// Call from a blocking task: the pinned keyring backend may perform blocking I/O.
    pub fn validate_current_ema_credentials(&self) -> Result<()> {
        self.load_ema_credentials(&DefaultKeyringStore).map(|_| ())
    }

    /// Reread only the pinned keyring authority; exchange callers hold its credential lock.
    pub(crate) fn load_ema_credentials<K: KeyringStore + Clone + 'static>(
        &self,
        keyring_store: &K,
    ) -> Result<StoredOAuthTokens> {
        if self.store == ResolvedOAuthCredentialStore::file() {
            bail!("enterprise IdP credentials require keyring storage");
        }
        let previous = &self.credentials;
        let mut latest = self
            .store
            .load(keyring_store, &previous.server_name, &previous.url)?
            .ok_or_else(|| {
                ema_reauthentication_required(
                    "enterprise IdP credentials were removed; sign in again",
                )
            })?;
        // There is no refresh-token rotation writer in the supported EMA profile.
        // Pin the whole atomic login record, not just the claims of its ID token.
        latest.token_response.0.set_expires_in(None);
        if latest != *previous
            || previous.bound_issuer() != Some(previous.url.as_str())
            || !latest.has_refresh_token()
        {
            return Err(ema_reauthentication_required(
                "enterprise IdP identity changed; sign in again and reconnect",
            ));
        }
        stored_oidc_identity(&latest)?;
        Ok(latest)
    }
}

#[cfg(test)]
#[path = "ema_identity_tests.rs"]
mod tests;
