//! Scoped access to a persisted login grant. Refresh exclusion lasts only through the IdP
//! exchange; callers must recheck ownership before publishing its result. Storage reads
//! observe other processes at these boundaries, not continuously.

use sha2::Digest;
use sha2::Sha256;
use std::fmt;
use std::io;
use std::sync::Arc;
use tokio::sync::OwnedSemaphorePermit;

use super::AuthManager;
use crate::token_data::TokenData;

/// A snapshot of one account grant, checked against both cached ownership and storage.
/// This does not authorize a destination; consumers must validate the recorded issuer/client.
pub struct AccountOAuthGrant {
    pub(super) manager: Arc<AuthManager>,
    pub(super) tokens: TokenData,
    pub(super) login_generation: u64,
}

/// Identity of a refresh grant, excluding access and ID token rotation.
/// This opaque digest compares revisions; it does not authorize a destination.
#[derive(Clone, PartialEq, Eq)]
pub struct AccountOAuthGrantRevision([u8; 32]);

impl From<&TokenData> for AccountOAuthGrantRevision {
    fn from(tokens: &TokenData) -> Self {
        let mut hash = Sha256::new();
        let provenance = tokens.oauth_provenance.as_ref();
        for value in [
            tokens.login_owner_id.as_deref(),
            tokens.account_id.as_deref(),
            tokens.id_token.chatgpt_user_id.as_deref(),
            provenance.map(|provenance| provenance.issuer.as_str()),
            provenance.map(|provenance| provenance.client_id.as_str()),
            Some(tokens.refresh_token.as_str()),
        ] {
            let value = value.unwrap_or_default();
            hash.update(value.len().to_be_bytes());
            hash.update(value.as_bytes());
        }
        Self(hash.finalize().into())
    }
}

impl fmt::Debug for AccountOAuthGrantRevision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccountOAuthGrantRevision")
            .finish_non_exhaustive()
    }
}

/// Keeps ordinary refresh from rotating the account token during an IdP exchange.
/// Login and logout remain available and invalidate the grant independently of this lease.
pub struct AccountOAuthGrantLease {
    pub(super) grant: AccountOAuthGrant,
    pub(super) permit: OwnedSemaphorePermit,
}

impl fmt::Debug for AccountOAuthGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccountOAuthGrant").finish_non_exhaustive()
    }
}

impl fmt::Debug for AccountOAuthGrantLease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccountOAuthGrantLease")
            .finish_non_exhaustive()
    }
}

impl AccountOAuthGrantLease {
    pub fn tokens(&self) -> &TokenData {
        &self.grant.tokens
    }

    /// Release refresh exclusion before contacting a resource authorization server.
    pub fn release(self) -> AccountOAuthGrant {
        drop(self.permit);
        self.grant
    }
}

impl AccountOAuthGrant {
    pub fn tokens(&self) -> &TokenData {
        &self.tokens
    }

    /// Compare credential revisions without exposing the refresh token to state holders.
    pub fn same_credential(&self, other: &Self) -> bool {
        self.login_generation == other.login_generation
            && AccountOAuthGrantRevision::from(&self.tokens)
                == AccountOAuthGrantRevision::from(&other.tokens)
    }

    pub(super) fn owner_is_current(&self) -> bool {
        let Ok(cached) = self.manager.inner.read() else {
            return false;
        };
        self.manager.auth_change_state_tx.borrow().login_generation == self.login_generation
            && cached
                .auth
                .as_ref()
                .filter(|auth| matches!(auth, super::CodexAuth::Chatgpt(_)))
                .and_then(|auth| auth.get_token_data().ok())
                .is_some_and(|tokens| same_account_grant(&self.tokens, &tokens))
    }

    /// Read the still-current grant after asynchronous work. Consumers must validate
    /// the latest ID token's eligibility before accepting an exchange result.
    /// A storage failure is an error, never proof that the saved grant is still valid.
    pub async fn current_tokens(&self) -> io::Result<Option<TokenData>> {
        if !self.owner_is_current() {
            return Ok(None);
        }
        let stored = self.manager.read_account_grant_tokens().await?;
        Ok(stored.filter(|tokens| {
            self.owner_is_current()
                && AccountOAuthGrantRevision::from(&self.tokens)
                    == AccountOAuthGrantRevision::from(tokens)
        }))
    }
}

pub(super) fn same_account_grant(a: &TokenData, b: &TokenData) -> bool {
    a.oauth_provenance == b.oauth_provenance && same_account_login(a, b)
}

pub(super) fn same_account_login(a: &TokenData, b: &TokenData) -> bool {
    a.login_owner_id == b.login_owner_id
        && a.account_id == b.account_id
        && a.id_token.chatgpt_user_id == b.id_token.chatgpt_user_id
}
