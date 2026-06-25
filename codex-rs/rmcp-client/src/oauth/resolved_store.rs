//! Resolves the configured MCP OAuth store and pins that concrete source for one client lifecycle.

use anyhow::Context;
use anyhow::Result;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_keyring_store::KeyringStore;
use tracing::warn;

use super::StoredOAuthTokens;
use super::load_oauth_tokens_from_file;
use super::load_oauth_tokens_from_keyring;
use super::resolution_state::StoreResolutionReason;
use super::resolution_state::record_store_resolution;

/// Concrete credential store resolved for one MCP OAuth client lifecycle.
///
/// This selection is intentionally not persisted as credential authority. `Auto` may resolve
/// differently in a later process, but a client that loaded credentials from one store must
/// reread, refresh, persist, and remove only through that store. A separate best-effort diagnostic
/// record warns when later Auto resolution changes; it never influences this selection. A
/// mid-lifecycle backend failure is unexpected and must return an error rather than falling back
/// to another possibly stale refresh token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResolvedOAuthCredentialStore {
    File,
    Keyring(AuthKeyringBackendKind),
}

#[derive(Debug)]
pub(crate) struct ResolvedOAuthTokens {
    pub(crate) tokens: StoredOAuthTokens,
    pub(crate) store: ResolvedOAuthCredentialStore,
}

pub(crate) fn resolve_oauth_tokens<K: KeyringStore + Clone + 'static>(
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
            // Different processes can still resolve Auto to different stores when keyring
            // availability differs. We persist only a diagnostic observation so that transition
            // emits a warning and metric. Making it authoritative would require durable backend
            // selection or reconciliation of legacy entries and remains intentionally out of
            // scope.
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
                .with_context(|| "failed to read OAuth tokens from keyring".to_string())?
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

pub(crate) fn load_oauth_tokens_from_store<K: KeyringStore + Clone + 'static>(
    keyring_store: &K,
    server_name: &str,
    url: &str,
    store: ResolvedOAuthCredentialStore,
) -> Result<Option<StoredOAuthTokens>> {
    match store {
        ResolvedOAuthCredentialStore::File => load_oauth_tokens_from_file(server_name, url)
            .context("failed to reread OAuth tokens from resolved file storage"),
        ResolvedOAuthCredentialStore::Keyring(keyring_backend_kind) => {
            load_oauth_tokens_from_keyring(
                keyring_store,
                keyring_backend_kind,
                server_name,
                url,
            )
            .context(
                "failed to reread OAuth tokens from resolved keyring storage; refusing file fallback",
            )
        }
    }
}
