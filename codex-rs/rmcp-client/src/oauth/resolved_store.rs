//! Resolves the configured MCP OAuth store and pins that concrete source for one client lifecycle.

use anyhow::Context;
use anyhow::Result;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_keyring_store::KeyringStore;
use tracing::warn;

use super::OAuthKeyringLoadError;
use super::StoredOAuthTokens;
use super::load_oauth_tokens_from_file;
use super::load_oauth_tokens_from_keyring;
use super::resolution_state::StoreResolutionReason;
use super::resolution_state::record_store_resolution;
use super::save_oauth_tokens_to_file;
use super::save_oauth_tokens_with_keyring;

/// Concrete credential store resolved for one MCP OAuth client lifecycle.
///
/// This selection is intentionally not persisted as credential authority. `Auto` may resolve
/// differently in a later process, but a client that loaded credentials from one store must
/// reread, refresh, persist, and remove only through that store. A separate best-effort diagnostic
/// record warns when later Auto resolution changes; it never influences this selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResolvedOAuthCredentialStore {
    File,
    Keyring(AuthKeyringBackendKind),
}

impl ResolvedOAuthCredentialStore {
    /// Loads credentials only from this already-resolved authority.
    ///
    /// Unlike `resolve_oauth_tokens_from_store_policy`, this never evaluates configured
    /// `Auto` fallback policy.
    pub(crate) fn load<K: KeyringStore + Clone + 'static>(
        self,
        keyring_store: &K,
        server_name: &str,
        url: &str,
    ) -> Result<Option<StoredOAuthTokens>> {
        match self {
            Self::File => load_oauth_tokens_from_file(server_name, url)
                .context("failed to reread OAuth tokens from resolved file storage"),
            Self::Keyring(keyring_backend_kind) => load_oauth_tokens_from_keyring(
                keyring_store,
                keyring_backend_kind,
                server_name,
                url,
            )
            .map_err(anyhow::Error::from)
            .context(
                "failed to reread OAuth tokens from resolved keyring storage; refusing file fallback",
            ),
        }
    }

    /// Saves credentials only to this already-resolved authority.
    pub(crate) fn save<K: KeyringStore + Clone + 'static>(
        self,
        keyring_store: &K,
        server_name: &str,
        tokens: &StoredOAuthTokens,
    ) -> Result<()> {
        match self {
            Self::File => save_oauth_tokens_to_file(tokens),
            Self::Keyring(keyring_backend_kind) => save_oauth_tokens_with_keyring(
                keyring_store,
                keyring_backend_kind,
                server_name,
                tokens,
            ),
        }
    }
}

#[derive(Debug)]
pub(crate) struct ResolvedOAuthTokens {
    pub(crate) tokens: StoredOAuthTokens,
    pub(crate) store: ResolvedOAuthCredentialStore,
}

pub(crate) fn resolve_oauth_tokens_from_store_policy<K: KeyringStore + Clone + 'static>(
    keyring_store: &K,
    server_name: &str,
    url: &str,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
) -> Result<Option<ResolvedOAuthTokens>> {
    let (resolved, reason) = match store_mode {
        OAuthCredentialsStoreMode::Auto => {
            // Auto remains keyring-first at lifecycle startup. The returned source is then pinned
            // by the client transport recipe and OAuth persistor so retries, recovery, and
            // refresh work cannot hot-switch stores.
            // Different processes can still resolve Auto differently when keyring availability
            // changes. The token-free observation below makes that drift visible without turning
            // the sidecar into another source of credential authority.
            match load_oauth_tokens_from_keyring(
                keyring_store,
                keyring_backend_kind,
                server_name,
                url,
            ) {
                Ok(Some(tokens)) => (
                    Some(ResolvedOAuthTokens {
                        tokens,
                        store: ResolvedOAuthCredentialStore::Keyring(keyring_backend_kind),
                    }),
                    StoreResolutionReason::AutoLoadFromKeyring,
                ),
                Ok(None) => (
                    load_oauth_tokens_from_file(server_name, url)?.map(|tokens| {
                        ResolvedOAuthTokens {
                            tokens,
                            store: ResolvedOAuthCredentialStore::File,
                        }
                    }),
                    StoreResolutionReason::AutoLoadFromFileAfterMissingKeyring,
                ),
                // Auto may fall back when the keyring backend is unavailable, but a Secrets
                // aggregate-lock failure means authority may be changing. Consulting File in
                // that state could replay credentials hidden behind a newer Secrets entry.
                Err(OAuthKeyringLoadError::StoreLock(error)) => {
                    return Err(error.into());
                }
                Err(error) => {
                    warn!("failed to read OAuth tokens from keyring: {error}");
                    (
                        load_oauth_tokens_from_file(server_name, url)
                            .with_context(|| {
                                format!("failed to read OAuth tokens from keyring: {error}")
                            })?
                            .map(|tokens| ResolvedOAuthTokens {
                                tokens,
                                store: ResolvedOAuthCredentialStore::File,
                            }),
                        StoreResolutionReason::AutoLoadFromFileAfterKeyringError,
                    )
                }
            }
        }
        OAuthCredentialsStoreMode::File => (
            load_oauth_tokens_from_file(server_name, url)?.map(|tokens| ResolvedOAuthTokens {
                tokens,
                store: ResolvedOAuthCredentialStore::File,
            }),
            StoreResolutionReason::ConfiguredLoad,
        ),
        OAuthCredentialsStoreMode::Keyring => (
            load_oauth_tokens_from_keyring(keyring_store, keyring_backend_kind, server_name, url)
                .map_err(anyhow::Error::from)
                .context("failed to read OAuth tokens from keyring")?
                .map(|tokens| ResolvedOAuthTokens {
                    tokens,
                    store: ResolvedOAuthCredentialStore::Keyring(keyring_backend_kind),
                }),
            StoreResolutionReason::ConfiguredLoad,
        ),
    };

    // Explicit modes select their store even when no credential exists. Auto has no concrete
    // selection until one store returns credentials or login successfully persists them.
    let resolved_store = resolved
        .as_ref()
        .map(|resolved| resolved.store)
        .or(match store_mode {
            OAuthCredentialsStoreMode::Auto => None,
            OAuthCredentialsStoreMode::File => Some(ResolvedOAuthCredentialStore::File),
            OAuthCredentialsStoreMode::Keyring => {
                Some(ResolvedOAuthCredentialStore::Keyring(keyring_backend_kind))
            }
        });
    if let Some(resolved_store) = resolved_store {
        record_store_resolution(
            server_name,
            url,
            store_mode,
            keyring_backend_kind,
            resolved_store,
            reason,
        );
    }
    Ok(resolved)
}
