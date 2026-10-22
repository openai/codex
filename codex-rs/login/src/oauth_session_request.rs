//! Coherent request credentials and unauthorized recovery for configured OAuth consumers.
//! The session manager owns observation, refresh, locking, and persistence; a request
//! retains only its rejected token so transport retries cannot forget recovery.

use super::OAuthSessionManager;
use super::RefreshPolicy;
use super::login;
use std::fmt;
use std::io;

/// A token and the identity of the exact grant that supplied it.
pub struct OAuthRequestCredentials {
    pub access_token: String,
    pub authorization_id: Option<String>,
}

impl fmt::Debug for OAuthRequestCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OAuthRequestCredentials")
            .field("access_token", &"<redacted>")
            .field("authorization_id", &self.authorization_id)
            .finish()
    }
}

/// One consumer request and its retries, using the existing session's credential state.
pub struct OAuthSessionRequest {
    manager: OAuthSessionManager,
    policy: RefreshPolicy,
    sent_token: Option<String>,
}

impl OAuthSessionRequest {
    pub(super) fn new(manager: OAuthSessionManager) -> Self {
        Self {
            manager,
            policy: RefreshPolicy::ObserveStored,
            sent_token: None,
        }
    }

    /// Observes persisted credentials using the existing session lifecycle. The returned
    /// identity is captured with the token, never read separately after resolution.
    pub async fn credentials(&mut self) -> io::Result<OAuthRequestCredentials> {
        let token = self
            .manager
            .resolve_credentials_with_browser(self.policy.clone(), login::open_browser)
            .await?;
        self.sent_token = Some(token.access_token.clone());
        Ok(OAuthRequestCredentials {
            access_token: token.access_token,
            authorization_id: token.authorization_id,
        })
    }

    /// Marks the sent credential as rejected. Callers bound unauthorized replays;
    /// transient transport failures retain this state until resolution succeeds.
    pub fn reject_credentials(&mut self) -> bool {
        let Some(token) = self.sent_token.take() else {
            return false;
        };
        self.policy = RefreshPolicy::AfterRejection(token);
        true
    }

    /// Identifies credential failures that belong to unauthorized recovery.
    pub fn is_recovering(&self) -> bool {
        matches!(self.policy, RefreshPolicy::AfterRejection(_))
    }
}
