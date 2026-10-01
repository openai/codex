//! Pin enterprise credentials to a non-secret version advanced by cooperating writers.
//! Exchanges reread the pinned keyring record and reject changes made outside this protocol.

use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use codex_keyring_store::DefaultKeyringStore;
use codex_keyring_store::KeyringStore;
use tracing::instrument::WithSubscriber;

use crate::ema_auth_policy::EmaAuthFailure;
use crate::ema_auth_policy::ema_reauthentication_required;
use crate::ema_claims::OidcClaims;
use crate::ema_claims::oidc_identity;
use crate::oauth::EnterpriseOAuthGeneration;
use crate::oauth::EnterpriseOAuthGenerationFile;
use crate::oauth::EnterpriseOAuthGenerationKind;
use crate::oauth::RefreshCredentialLock;
use crate::oauth::ResolvedOAuthCredentialStore;
use crate::oauth::StoredOAuthCredentialSnapshot;
use crate::oauth::StoredOAuthTokens;
use crate::oauth::invalidate_enterprise_credential_version;

/// One validated keyring record and its cross-process credential version.
#[derive(Clone)]
pub struct EmaCredentialSnapshot {
    snapshot: StoredOAuthCredentialSnapshot,
    version: EnterpriseOAuthGeneration,
}

/// A credential snapshot admitted by a short generation check.
/// Logout may replace the grant while an admitted exchange finishes.
pub struct EmaCredentialLease {
    pub(crate) credentials: StoredOAuthCredentialSnapshot,
}

// Discard storage and lock error chains: keyring backends can include the stable
// account/workspace key. Keep only the sanitized authentication classification.
fn sanitize_ema_credential_error(error: anyhow::Error, message: &'static str) -> anyhow::Error {
    match error.downcast::<EmaAuthFailure>() {
        Ok(failure) => anyhow::Error::new(failure).context(message),
        Err(_) => anyhow!(message),
    }
}

impl std::fmt::Debug for EmaCredentialLease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmaCredentialLease").finish_non_exhaustive()
    }
}

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
    /// Bind the record and its version under one lease, validating any earlier snapshot read.
    pub async fn pin_ema_credentials(self) -> Result<EmaCredentialSnapshot> {
        self.pin_ema_credentials_in(&DefaultKeyringStore).await
    }

    async fn pin_ema_credentials_in<K: KeyringStore + Clone + 'static>(
        self,
        keyring_store: &K,
    ) -> Result<EmaCredentialSnapshot> {
        if !self.credentials.server_name.starts_with("ema-idp:") {
            bail!("enterprise credentials require the enterprise namespace");
        }
        let lock = RefreshCredentialLock::acquire_for_server(
            &self.credentials.server_name,
            &self.credentials.url,
        )
        .with_subscriber(tracing::subscriber::NoSubscriber::default())
        .await
        .map_err(|_| anyhow!("failed to lock enterprise credentials"))?;
        let keyring_store = keyring_store.clone();
        tokio::task::spawn_blocking(move || {
            tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
                self.load_ema_credentials(&keyring_store)?;
                let versions = EnterpriseOAuthGenerationFile::open(
                    &self.credentials.server_name,
                    &self.credentials.url,
                    EnterpriseOAuthGenerationKind::Credential,
                    &lock,
                )?;
                let version = match versions.current()? {
                    Some(version) => version,
                    None => versions.replace()?,
                };
                Ok(EmaCredentialSnapshot {
                    snapshot: self,
                    version,
                })
            })
        })
        .await
        .map_err(|_| anyhow!("enterprise credential pinning task failed"))?
        .map_err(|error: anyhow::Error| {
            sanitize_ema_credential_error(error, "failed to pin enterprise credentials")
        })
    }
}

impl EmaCredentialSnapshot {
    /// Validate the pinned version under the credential lock without reading the keyring.
    pub async fn acquire_current_ema_credentials(&self) -> Result<EmaCredentialLease> {
        let credential_lock = RefreshCredentialLock::acquire_for_server(
            &self.snapshot.credentials.server_name,
            &self.snapshot.credentials.url,
        )
        .with_subscriber(tracing::subscriber::NoSubscriber::default())
        .await
        .map_err(|_| anyhow!("failed to lock enterprise credentials"))?;
        let pinned = self.clone();
        tokio::task::spawn_blocking(move || {
            tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
                let credentials = pinned.snapshot.credentials();
                let versions = EnterpriseOAuthGenerationFile::open(
                    &credentials.server_name,
                    &credentials.url,
                    EnterpriseOAuthGenerationKind::Credential,
                    &credential_lock,
                )?;
                if versions.current()?.as_ref() != Some(&pinned.version) {
                    return Err(ema_reauthentication_required(
                        "enterprise credentials changed; reconnect",
                    ));
                }
                Ok(EmaCredentialLease {
                    credentials: pinned.snapshot,
                })
            })
        })
        .await
        .map_err(|_| anyhow!("enterprise credential validation task failed"))?
        .map_err(|error: anyhow::Error| {
            sanitize_ema_credential_error(error, "failed to validate enterprise credentials")
        })
    }

    /// Exclusively lock and delete a rejected grant only if its pinned record is still current.
    pub async fn invalidate_ema_credentials_if_current(&self) -> Result<()> {
        self.invalidate_ema_credentials_if_current_in(&DefaultKeyringStore)
            .await
    }

    async fn invalidate_ema_credentials_if_current_in<K: KeyringStore + Clone + 'static>(
        &self,
        keyring_store: &K,
    ) -> Result<()> {
        let credential_lock = RefreshCredentialLock::acquire_for_server(
            &self.snapshot.credentials.server_name,
            &self.snapshot.credentials.url,
        )
        .with_subscriber(tracing::subscriber::NoSubscriber::default())
        .await
        .map_err(|_| anyhow!("failed to lock enterprise credentials"))?;
        let pinned = self.clone();
        let keyring_store = keyring_store.clone();
        tokio::task::spawn_blocking(move || {
            tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
                let snapshot = &pinned.snapshot;
                let previous = snapshot.credentials();
                let versions = EnterpriseOAuthGenerationFile::open(
                    &previous.server_name,
                    &previous.url,
                    EnterpriseOAuthGenerationKind::Credential,
                    &credential_lock,
                )?;
                if versions.current()?.as_ref() != Some(&pinned.version) {
                    return Ok(());
                }
                if let Some(mut latest) =
                    snapshot
                        .store
                        .load(&keyring_store, &previous.server_name, &previous.url)?
                {
                    latest.token_response.0.set_expires_in(None);
                    if latest == *previous {
                        invalidate_enterprise_credential_version(
                            &previous.server_name,
                            &previous.url,
                            &credential_lock,
                        )?;
                        snapshot.store.delete(
                            &keyring_store,
                            &previous.server_name,
                            &previous.url,
                        )?;
                    }
                }
                Ok(())
            })
        })
        .await
        .map_err(|_| anyhow!("enterprise credential invalidation task failed"))?
        .map_err(|error: anyhow::Error| {
            sanitize_ema_credential_error(error, "failed to invalidate enterprise credentials")
        })
    }
}

impl StoredOAuthCredentialSnapshot {
    /// Reread only the pinned keyring authority; a changed record rejects the exchange.
    pub(crate) fn load_ema_credentials<K: KeyringStore + Clone + 'static>(
        &self,
        keyring_store: &K,
    ) -> Result<StoredOAuthTokens> {
        if self.store == ResolvedOAuthCredentialStore::file() {
            bail!("enterprise IdP credentials require keyring storage");
        }
        let previous = &self.credentials;
        let mut latest =
            tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
                self.store
                    .load(keyring_store, &previous.server_name, &previous.url)
            })
            .map_err(|_| anyhow!("failed to read enterprise IdP credentials from keyring"))?
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
