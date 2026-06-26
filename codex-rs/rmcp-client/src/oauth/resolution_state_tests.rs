use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use pretty_assertions::assert_eq;

use super::ObservedStore;
use super::ResolutionState;
use super::StoreResolution;
use super::StoreResolutionChange;
use super::record_store_resolution_in;
use crate::oauth::ResolvedOAuthCredentialStore;
use crate::oauth::compute_store_key;

#[test]
fn auto_resolution_change_is_reported_and_persisted() -> anyhow::Result<()> {
    let codex_home = tempfile::tempdir()?;
    let server_name = "test-server";
    let url = "https://example.test/mcp";

    assert_eq!(
        record_store_resolution_in(
            codex_home.path(),
            server_name,
            url,
            OAuthCredentialsStoreMode::Auto,
            AuthKeyringBackendKind::Direct,
            ResolvedOAuthCredentialStore::Keyring(AuthKeyringBackendKind::Direct),
        )?,
        None
    );
    assert_eq!(
        record_store_resolution_in(
            codex_home.path(),
            server_name,
            url,
            OAuthCredentialsStoreMode::Auto,
            AuthKeyringBackendKind::Direct,
            ResolvedOAuthCredentialStore::File,
        )?,
        Some(StoreResolutionChange {
            previous: ObservedStore::Keyring,
            current: ObservedStore::File,
        })
    );

    let state: ResolutionState = serde_json::from_str(&std::fs::read_to_string(
        codex_home.path().join(super::RESOLUTION_STATE_FILENAME),
    )?)?;
    assert_eq!(
        state,
        ResolutionState {
            version: super::RESOLUTION_STATE_VERSION,
            resolutions: [(
                compute_store_key(server_name, url)?,
                StoreResolution {
                    store_mode: OAuthCredentialsStoreMode::Auto,
                    keyring_backend: AuthKeyringBackendKind::Direct,
                    resolved_store: ObservedStore::File,
                },
            )]
            .into(),
        }
    );
    Ok(())
}

#[test]
fn intentional_configuration_changes_reset_the_auto_comparison() -> anyhow::Result<()> {
    let codex_home = tempfile::tempdir()?;
    let server_name = "test-server";
    let url = "https://example.test/mcp";

    assert_eq!(
        record_store_resolution_in(
            codex_home.path(),
            server_name,
            url,
            OAuthCredentialsStoreMode::Auto,
            AuthKeyringBackendKind::Direct,
            ResolvedOAuthCredentialStore::Keyring(AuthKeyringBackendKind::Direct),
        )?,
        None
    );
    assert_eq!(
        record_store_resolution_in(
            codex_home.path(),
            server_name,
            url,
            OAuthCredentialsStoreMode::File,
            AuthKeyringBackendKind::Direct,
            ResolvedOAuthCredentialStore::File,
        )?,
        None
    );
    assert_eq!(
        record_store_resolution_in(
            codex_home.path(),
            server_name,
            url,
            OAuthCredentialsStoreMode::Auto,
            AuthKeyringBackendKind::Direct,
            ResolvedOAuthCredentialStore::File,
        )?,
        None
    );
    assert_eq!(
        record_store_resolution_in(
            codex_home.path(),
            server_name,
            url,
            OAuthCredentialsStoreMode::Auto,
            AuthKeyringBackendKind::Secrets,
            ResolvedOAuthCredentialStore::Keyring(AuthKeyringBackendKind::Secrets),
        )?,
        None
    );
    assert_eq!(
        record_store_resolution_in(
            codex_home.path(),
            server_name,
            url,
            OAuthCredentialsStoreMode::Auto,
            AuthKeyringBackendKind::Secrets,
            ResolvedOAuthCredentialStore::File,
        )?,
        Some(StoreResolutionChange {
            previous: ObservedStore::Keyring,
            current: ObservedStore::File,
        })
    );
    Ok(())
}
