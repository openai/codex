//! Maps MCP storage policy, pinned stores, and typed failures to credential-free metric dimensions.

use super::OAuthKeyringLoadError;
use super::OAuthStoreLockFailure;
use super::ResolvedOAuthCredentialStore;
use super::resolved_store::Backend;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_otel::auth_storage::AuthStorageOriginator;
use codex_otel::auth_storage::CredentialKind;
use codex_otel::auth_storage::Operation;
use codex_otel::auth_storage::StoragePhase;
use codex_otel::auth_storage::StorageTelemetry;
use codex_otel::auth_storage::Store;
use codex_otel::auth_storage::StoreMode;
use std::error::Error;
use std::io::ErrorKind;

/// Preserve aggregate-lock categories even when a transparent load error hides their source.
pub(super) fn record_secure_error(
    observation: &mut StorageTelemetry,
    error: &(dyn Error + 'static),
) {
    let mut cause = Some(error);
    while let Some(current) = cause {
        let lock = match current.downcast_ref::<OAuthKeyringLoadError>() {
            Some(OAuthKeyringLoadError::StoreLock(lock)) => Some(lock),
            Some(OAuthKeyringLoadError::Backend(_)) | None => {
                current.downcast_ref::<OAuthStoreLockFailure>()
            }
        };
        if let Some(OAuthStoreLockFailure::Timeout {
            acquire_timeout, ..
        }) = lock
        {
            let kind = if acquire_timeout.is_zero() {
                ErrorKind::WouldBlock
            } else {
                ErrorKind::TimedOut
            };
            observation.record_secure_error(&std::io::Error::from(kind));
            return;
        }
        cause = current.source();
    }
    observation.record_secure_error(error);
}

pub(super) fn keyring(kind: AuthKeyringBackendKind) -> Store {
    match kind {
        AuthKeyringBackendKind::Direct => Store::DirectKeyring,
        AuthKeyringBackendKind::Secrets => Store::Secrets,
    }
}

pub(super) fn store(store: ResolvedOAuthCredentialStore) -> Store {
    match store.backend {
        Backend::File => Store::File,
        Backend::Keyring(kind) => keyring(kind),
    }
}

pub(super) fn policy(
    mode: OAuthCredentialsStoreMode,
    kind: AuthKeyringBackendKind,
    operation: Operation,
) -> StorageTelemetry {
    StorageTelemetry::new(
        CredentialKind::Mcp,
        store_mode(mode),
        keyring(kind),
        operation,
        AuthStorageOriginator::current(),
    )
}

pub(super) fn resolved(
    source: ResolvedOAuthCredentialStore,
    operation: Operation,
) -> StorageTelemetry {
    StorageTelemetry::new(
        CredentialKind::Mcp,
        store_mode(source.mode),
        keyring(source.kind),
        operation,
        source.originator,
    )
    .with_phase(StoragePhase::Pinned)
}

fn store_mode(mode: OAuthCredentialsStoreMode) -> StoreMode {
    match mode {
        OAuthCredentialsStoreMode::File => StoreMode::File,
        OAuthCredentialsStoreMode::Auto => StoreMode::Auto,
        OAuthCredentialsStoreMode::Keyring => StoreMode::Keyring,
    }
}
