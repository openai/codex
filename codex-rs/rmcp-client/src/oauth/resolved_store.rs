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

/// Concrete credential store resolved for one MCP OAuth client lifecycle.
///
/// This is intentionally not durable. `Auto` may resolve differently in a later process, but a
/// client that loaded credentials from one store must reread, refresh, persist, and remove only
/// through that store. A mid-lifecycle backend failure is unexpected and must return an error
/// rather than falling back to another possibly stale refresh token.
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
    match store_mode {
        OAuthCredentialsStoreMode::Auto => {
            // Auto remains keyring-first at lifecycle startup. The returned source is then pinned
            // by the client transport recipe and OAuth persistor so retries, recovery, and
            // refresh work cannot hot-switch stores.
            // TODO(stevenlee): Different processes can still resolve Auto to different stores
            // when keyring availability differs. Solving that safely requires durable backend
            // selection or reconciliation of legacy entries and is intentionally outside this
            // stack.
            match load_oauth_tokens_from_keyring(
                keyring_store,
                keyring_backend_kind,
                server_name,
                url,
            ) {
                Ok(Some(tokens)) => Ok(Some(ResolvedOAuthTokens {
                    tokens,
                    store: ResolvedOAuthCredentialStore::Keyring(keyring_backend_kind),
                })),
                Ok(None) => Ok(
                    load_oauth_tokens_from_file(server_name, url)?.map(|tokens| {
                        ResolvedOAuthTokens {
                            tokens,
                            store: ResolvedOAuthCredentialStore::File,
                        }
                    }),
                ),
                Err(error) => {
                    warn!("failed to read OAuth tokens from keyring: {error}");
                    Ok(load_oauth_tokens_from_file(server_name, url)
                        .with_context(|| {
                            format!("failed to read OAuth tokens from keyring: {error}")
                        })?
                        .map(|tokens| ResolvedOAuthTokens {
                            tokens,
                            store: ResolvedOAuthCredentialStore::File,
                        }))
                }
            }
        }
        OAuthCredentialsStoreMode::File => Ok(load_oauth_tokens_from_file(server_name, url)?.map(
            |tokens| ResolvedOAuthTokens {
                tokens,
                store: ResolvedOAuthCredentialStore::File,
            },
        )),
        OAuthCredentialsStoreMode::Keyring => Ok(load_oauth_tokens_from_keyring(
            keyring_store,
            keyring_backend_kind,
            server_name,
            url,
        )
        .with_context(|| "failed to read OAuth tokens from keyring".to_string())?
        .map(|tokens| ResolvedOAuthTokens {
            tokens,
            store: ResolvedOAuthCredentialStore::Keyring(keyring_backend_kind),
        })),
    }
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
