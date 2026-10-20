//! Scoped access to a persisted login grant. Refresh exclusion lasts only through the IdP
//! exchange; callers must recheck ownership before publishing its result. Storage reads
//! observe other processes at these boundaries, not continuously.

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
        self.tokens == other.tokens
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

    /// Check after asynchronous work, before accepting a token or publishing a connection.
    /// A storage failure is an error, never proof that the saved grant is still valid.
    pub async fn check_current(&self) -> io::Result<bool> {
        if !self.owner_is_current() {
            return Ok(false);
        }
        let stored = self.manager.read_account_grant_tokens().await?;
        Ok(self.owner_is_current() && stored.as_ref().is_some_and(|tokens| self.tokens == *tokens))
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
