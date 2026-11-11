//! Resolves the configured MCP OAuth store and pins that concrete source for one client lifecycle.
//! Retains the originating client for later background storage observations.

use super::telemetry;
use anyhow::Context;
use anyhow::Result;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_keyring_store::KeyringStore;
use codex_otel::auth_storage::AuthStorageOriginator;
use codex_otel::auth_storage::Operation;
use codex_otel::auth_storage::Store;
use rmcp::transport::auth::AuthError;

use super::FallbackTokenEntry;
use super::OAuthKeyringLoadError;
use super::OAuthStore;
use super::OAuthStoreLock;
use super::OAuthStoreLockFailure;
use super::StoredOAuthTokens;
use super::compute_store_key;
use super::delete_oauth_tokens_from_direct_keyring;
use super::delete_oauth_tokens_from_file;
use super::delete_oauth_tokens_from_secrets_keyring;
use super::load_oauth_tokens_from_file;
use super::load_oauth_tokens_from_file_with_lock_held;
use super::load_oauth_tokens_from_keyring;
use super::load_oauth_tokens_from_secrets_keyring_with_lock_held;
use super::normalized_file_oauth_credentials;
use super::normalized_oauth_credentials;
use super::read_fallback_file_unlocked;
use super::save_oauth_tokens_to_direct_keyring;
use super::save_oauth_tokens_to_file;
use super::save_oauth_tokens_to_secrets_keyring_with_lock_held;
use super::save_oauth_tokens_with_keyring;
use super::write_fallback_file;

/// Concrete credential store resolved for one MCP OAuth client lifecycle.
///
/// This is intentionally not durable. `Auto` may resolve differently in a later process, but a
/// client that loaded credentials from one store must reread, refresh, persist, and remove only
/// through that store. A mid-lifecycle backend failure is unexpected and must return an error
/// rather than falling back to another possibly stale refresh token.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ResolvedOAuthCredentialStore {
    pub(super) backend: Backend,
    pub(super) mode: OAuthCredentialsStoreMode,
    pub(super) kind: AuthKeyringBackendKind,
    pub(super) originator: AuthStorageOriginator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Backend {
    File,
    Keyring(AuthKeyringBackendKind),
}

// Telemetry metadata must not change credential identity or trigger a reconnect.
impl PartialEq for ResolvedOAuthCredentialStore {
    fn eq(&self, other: &Self) -> bool {
        self.backend == other.backend
    }
}
impl Eq for ResolvedOAuthCredentialStore {}

impl ResolvedOAuthCredentialStore {
    pub(crate) fn file() -> Self {
        Self {
            backend: Backend::File,
            mode: OAuthCredentialsStoreMode::File,
            kind: AuthKeyringBackendKind::Direct,
            originator: AuthStorageOriginator::current(),
        }
    }

    pub(crate) fn keyring(kind: AuthKeyringBackendKind) -> Self {
        Self {
            backend: Backend::Keyring(kind),
            mode: OAuthCredentialsStoreMode::Keyring,
            kind,
            originator: AuthStorageOriginator::current(),
        }
    }

    fn with_policy(
        mut self,
        mode: OAuthCredentialsStoreMode,
        kind: AuthKeyringBackendKind,
    ) -> Self {
        self.mode = mode;
        self.kind = kind;
        self
    }

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
        let mut observation = telemetry::resolved(self, Operation::Load);
        let result = match self.backend {
            Backend::File => load_oauth_tokens_from_file(server_name, url)
                .context("failed to reread OAuth tokens from resolved file storage"),
            Backend::Keyring(keyring_backend_kind) => load_oauth_tokens_from_keyring(
                keyring_store,
                keyring_backend_kind,
                server_name,
                url,
            )
            .map_err(anyhow::Error::from)
            .context(
                "failed to reread OAuth tokens from resolved keyring storage; refusing file fallback",
            ),
        };
        observation.record_load_attempt(telemetry::store(self), &result);
        if matches!(self.backend, Backend::Keyring(_))
            && let Err(error) = &result
        {
            telemetry::record_secure_error(&mut observation, error.as_ref());
        }
        result
    }

    /// Reads the selected authority without waiting for its aggregate-store lock.
    pub(crate) fn try_load<K: KeyringStore + Clone + 'static>(
        self,
        keyring_store: &K,
        server_name: &str,
        url: &str,
    ) -> Result<Option<StoredOAuthTokens>> {
        match self.backend {
            Backend::File => {
                let _store_lock = OAuthStoreLock::try_acquire_for_read(OAuthStore::File)?;
                load_oauth_tokens_from_file_with_lock_held(server_name, url)
                    .map(|entry| entry.map(|(_, tokens)| tokens))
                    .context("failed to probe OAuth tokens from resolved file storage")
            }
            Backend::Keyring(AuthKeyringBackendKind::Direct) => {
                load_oauth_tokens_from_keyring(keyring_store, AuthKeyringBackendKind::Direct, server_name, url)
                    .map_err(anyhow::Error::from)
                    .context("failed to reread OAuth tokens from resolved keyring storage; refusing file fallback")
            }
            Backend::Keyring(AuthKeyringBackendKind::Secrets) => {
                let _store_lock = OAuthStoreLock::try_acquire_for_read(OAuthStore::Secrets)?;
                load_oauth_tokens_from_secrets_keyring_with_lock_held(
                    keyring_store,
                    server_name,
                    url,
                )
                .map_err(anyhow::Error::from)
            }
        }
    }

    /// Saves credentials only to this already-resolved authority.
    pub(crate) fn save<K: KeyringStore + Clone + 'static>(
        self,
        keyring_store: &K,
        server_name: &str,
        tokens: &StoredOAuthTokens,
    ) -> Result<()> {
        let mut observation = telemetry::resolved(self, Operation::Save);
        let result = match self.backend {
            Backend::File => save_oauth_tokens_to_file(tokens),
            Backend::Keyring(keyring_backend_kind) => save_oauth_tokens_with_keyring(
                keyring_store,
                keyring_backend_kind,
                server_name,
                tokens,
            ),
        };
        observation.record_save_attempt(telemetry::store(self), &result);
        if matches!(self.backend, Backend::Keyring(_))
            && let Err(error) = &result
        {
            telemetry::record_secure_error(&mut observation, error.as_ref());
        }
        result
    }

    /// Persists a refresh to the pinned authority and records the refresh-persistence milestone.
    ///
    /// File replaces the matched row only if it still holds the expected credentials. Login
    /// writes continue to use the ordinary save path.
    pub(crate) fn save_with_refresh_telemetry<K: KeyringStore + Clone + 'static>(
        self,
        keyring_store: &K,
        server_name: &str,
        expected: Option<&StoredOAuthTokens>,
        tokens: &StoredOAuthTokens,
    ) -> Result<()> {
        let mut observation = telemetry::resolved(self, Operation::RefreshPersist);
        let result = match self.backend {
            Backend::File => {
                let mut save = telemetry::resolved(self, Operation::Save);
                let result = (|| {
                    let _store_lock = OAuthStoreLock::acquire_for_write(OAuthStore::File)?;
                    let (key, current) =
                        load_oauth_tokens_from_file_with_lock_held(server_name, &tokens.url)?
                            .ok_or(AuthError::AuthorizationRequired)?;
                    if normalized_oauth_credentials(Some(&current))
                        != normalized_file_oauth_credentials(expected)
                    {
                        return Err(AuthError::AuthorizationRequired.into());
                    }

                    let mut store =
                        read_fallback_file_unlocked()?.context("OAuth file disappeared")?;
                    // Keep the same matched row and stored identity, including legacy keys.
                    let entry = store
                        .get_mut(&key)
                        .context("OAuth file entry disappeared")?;
                    *entry = FallbackTokenEntry {
                        server_name: current.server_name,
                        server_url: current.url,
                        executor_owned: entry.executor_owned,
                        ..FallbackTokenEntry::from(tokens)
                    };
                    write_fallback_file(&store)
                })();
                save.record_save_attempt(Store::File, &result);
                result
            }
            Backend::Keyring(_) => self.save(keyring_store, server_name, tokens),
        };
        observation.record_save_attempt(telemetry::store(self), &result);
        if matches!(self.backend, Backend::Keyring(_))
            && let Err(error) = &result
        {
            telemetry::record_secure_error(&mut observation, error.as_ref());
        }
        result
    }

    /// Retires a rejected refresh token only while its durable snapshot still matches.
    ///
    /// File and Secrets keep one exclusive aggregate lock across the comparison and write,
    /// including against legacy writers that do not take the caller's refresh guard. Direct
    /// keyring callers must hold that guard and use a home-scoped enterprise credential.
    pub(crate) fn retire_refresh_token_if_matches<K: KeyringStore + Clone + 'static>(
        self,
        keyring_store: &K,
        server_name: &str,
        url: &str,
        expected: &StoredOAuthTokens,
    ) -> Result<()> {
        let mut observation = telemetry::resolved(self, Operation::Load);
        let mut _store_lock = None;
        let mut file_key = None;
        let current = (|| -> Result<_> {
            _store_lock = match self.backend {
                Backend::File => Some(OAuthStoreLock::acquire_for_write(OAuthStore::File)?),
                Backend::Keyring(AuthKeyringBackendKind::Secrets) => {
                    Some(OAuthStoreLock::acquire_for_write(OAuthStore::Secrets)?)
                }
                Backend::Keyring(AuthKeyringBackendKind::Direct) => None,
            };
            match self.backend {
                Backend::File => Ok(
                    load_oauth_tokens_from_file_with_lock_held(server_name, url)?.map(
                        |(key, tokens)| {
                            file_key = Some(key);
                            tokens
                        },
                    ),
                ),
                Backend::Keyring(AuthKeyringBackendKind::Secrets) => {
                    load_oauth_tokens_from_secrets_keyring_with_lock_held(
                        keyring_store,
                        server_name,
                        url,
                    )
                    .map_err(anyhow::Error::from)
                }
                Backend::Keyring(AuthKeyringBackendKind::Direct) => load_oauth_tokens_from_keyring(
                    keyring_store,
                    AuthKeyringBackendKind::Direct,
                    server_name,
                    url,
                )
                .map_err(anyhow::Error::from),
            }
        })();
        observation.record_load_attempt(telemetry::store(self), &current);
        if matches!(self.backend, Backend::Keyring(_))
            && let Err(error) = &current
        {
            telemetry::record_secure_error(&mut observation, error.as_ref());
        }
        drop(observation);
        let current = current?;
        if normalized_oauth_credentials(current.as_ref())
            != normalized_oauth_credentials(Some(expected))
        {
            return Ok(());
        }
        let mut retired = expected.clone();
        retired.token_response.0.set_refresh_token(None);
        let mut observation = telemetry::resolved(self, Operation::Save);
        let result = match self.backend {
            Backend::File => {
                let mut store = read_fallback_file_unlocked()?.context("OAuth file disappeared")?;
                // Reuse the key we compared: another binary may compute a different key.
                let key = file_key.context("missing loaded OAuth file key")?;
                store
                    .get_mut(&key)
                    .context("OAuth file entry disappeared")?
                    .refresh_token = None;
                write_fallback_file(&store)
            }
            Backend::Keyring(AuthKeyringBackendKind::Secrets) => {
                let serialized =
                    serde_json::to_string(&retired).context("failed to serialize OAuth tokens")?;
                save_oauth_tokens_to_secrets_keyring_with_lock_held(
                    keyring_store,
                    server_name,
                    &retired,
                    &serialized,
                )
            }
            Backend::Keyring(AuthKeyringBackendKind::Direct) => {
                save_oauth_tokens_to_direct_keyring(keyring_store, server_name, &retired)
            }
        };
        observation.record_save_attempt(telemetry::store(self), &result);
        if matches!(self.backend, Backend::Keyring(_))
            && let Err(error) = &result
        {
            telemetry::record_secure_error(&mut observation, error.as_ref());
        }
        result
    }

    /// Deletes credentials only from this already-resolved authority.
    pub(crate) fn delete<K: KeyringStore + Clone + 'static>(
        self,
        keyring_store: &K,
        server_name: &str,
        url: &str,
    ) -> Result<bool> {
        let mut observation = telemetry::resolved(self, Operation::Delete);
        let result = match self.backend {
            Backend::File => compute_store_key(server_name, url)
                .and_then(|key| delete_oauth_tokens_from_file(server_name, url, &key)),
            Backend::Keyring(AuthKeyringBackendKind::Direct) => {
                delete_oauth_tokens_from_direct_keyring(keyring_store, server_name, url)
            }
            Backend::Keyring(AuthKeyringBackendKind::Secrets) => {
                delete_oauth_tokens_from_secrets_keyring(keyring_store, server_name, url)
            }
        };
        observation.record_delete_attempt(telemetry::store(self), &result);
        if matches!(self.backend, Backend::Keyring(_))
            && let Err(error) = &result
        {
            telemetry::record_secure_error(&mut observation, error.as_ref());
        }
        result
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
    let mut observation = telemetry::policy(store_mode, keyring_backend_kind, Operation::Load);
    match store_mode {
        OAuthCredentialsStoreMode::Auto => {
            // Auto remains keyring-first at lifecycle startup. The returned source is then pinned
            // by the client transport recipe and OAuth persistor so retries, recovery, and
            // refresh work cannot hot-switch stores.
            // TODO(stevenlee): Different processes can still resolve Auto to different stores
            // when keyring availability differs. Solving that safely requires durable backend
            // selection or reconciliation of legacy entries and is intentionally outside this
            // stack.
            let result = load_oauth_tokens_from_keyring(
                keyring_store,
                keyring_backend_kind,
                server_name,
                url,
            );
            observation.record_load_attempt(telemetry::keyring(keyring_backend_kind), &result);
            match result {
                Ok(Some(tokens)) => Ok(Some(ResolvedOAuthTokens {
                    tokens,
                    store: ResolvedOAuthCredentialStore::keyring(keyring_backend_kind)
                        .with_policy(store_mode, keyring_backend_kind),
                })),
                Ok(None) => {
                    let result = load_oauth_tokens_from_file(server_name, url);
                    observation.record_load_attempt(Store::File, &result);
                    Ok(result?.map(|tokens| ResolvedOAuthTokens {
                        tokens,
                        store: ResolvedOAuthCredentialStore::file()
                            .with_policy(store_mode, keyring_backend_kind),
                    }))
                }
                // Auto may fall back when the keyring backend is unavailable, but a Secrets
                // aggregate-lock failure means authority may be changing. Consulting File in
                // that state could replay credentials hidden behind a newer Secrets entry.
                Err(OAuthKeyringLoadError::StoreLock(error)) => {
                    telemetry::record_secure_error(&mut observation, &error);
                    Err(error.into())
                }
                Err(error) => {
                    telemetry::record_secure_error(&mut observation, &error);
                    let result = load_oauth_tokens_from_file(server_name, url);
                    observation.record_load_attempt(Store::File, &result);
                    Ok(result
                        .with_context(|| {
                            format!("failed to read OAuth tokens from keyring: {error}")
                        })?
                        .map(|tokens| ResolvedOAuthTokens {
                            tokens,
                            store: ResolvedOAuthCredentialStore::file()
                                .with_policy(store_mode, keyring_backend_kind),
                        }))
                }
            }
        }
        OAuthCredentialsStoreMode::File => {
            let result = load_oauth_tokens_from_file(server_name, url);
            observation.record_load_attempt(Store::File, &result);
            Ok(result?.map(|tokens| ResolvedOAuthTokens {
                tokens,
                store: ResolvedOAuthCredentialStore::file()
                    .with_policy(store_mode, keyring_backend_kind),
            }))
        }
        OAuthCredentialsStoreMode::Keyring => {
            let result = load_oauth_tokens_from_keyring(
                keyring_store,
                keyring_backend_kind,
                server_name,
                url,
            );
            observation.record_load_attempt(telemetry::keyring(keyring_backend_kind), &result);
            if let Err(error) = &result {
                telemetry::record_secure_error(&mut observation, error);
            }
            Ok(result
                .map_err(anyhow::Error::from)
                .context("failed to read OAuth tokens from keyring")?
                .map(|tokens| ResolvedOAuthTokens {
                    tokens,
                    store: ResolvedOAuthCredentialStore::keyring(keyring_backend_kind)
                        .with_policy(store_mode, keyring_backend_kind),
                }))
        }
    }
}

pub(crate) fn try_resolve_oauth_tokens_from_store_policy<K: KeyringStore + Clone + 'static>(
    keyring_store: &K,
    server_name: &str,
    url: &str,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
) -> Result<Option<ResolvedOAuthTokens>> {
    let mut observation = telemetry::policy(store_mode, keyring_backend_kind, Operation::Load);
    let mut load = |store: ResolvedOAuthCredentialStore| {
        let result = store.try_load(keyring_store, server_name, url);
        observation.record_load_attempt(telemetry::store(store), &result);
        if matches!(store.backend, Backend::Keyring(_))
            && let Err(error) = &result
        {
            telemetry::record_secure_error(&mut observation, error.as_ref());
        }
        result.map(|tokens| tokens.map(|tokens| ResolvedOAuthTokens { tokens, store }))
    };
    let keyring = ResolvedOAuthCredentialStore::keyring(keyring_backend_kind)
        .with_policy(store_mode, keyring_backend_kind);
    match store_mode {
        OAuthCredentialsStoreMode::File => {
            load(ResolvedOAuthCredentialStore::file().with_policy(store_mode, keyring_backend_kind))
        }
        OAuthCredentialsStoreMode::Keyring => load(keyring),
        OAuthCredentialsStoreMode::Auto => match load(keyring) {
            Ok(Some(tokens)) => Ok(Some(tokens)),
            Ok(None) => load(
                ResolvedOAuthCredentialStore::file().with_policy(store_mode, keyring_backend_kind),
            ),
            Err(error) if error.downcast_ref::<OAuthStoreLockFailure>().is_some() => Err(error),
            Err(error) => load(
                ResolvedOAuthCredentialStore::file().with_policy(store_mode, keyring_backend_kind),
            )
            .with_context(|| format!("failed to read OAuth tokens from keyring: {error}")),
        },
    }
}
