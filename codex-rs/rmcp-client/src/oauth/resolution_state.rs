//! Best-effort diagnostics for changes in MCP OAuth store resolution.
//!
//! This state is deliberately observational, not authoritative. OAuth continues to resolve from
//! configuration and available credentials; failure to read, lock, or write this file must never
//! change which credential store is selected or whether an OAuth operation succeeds.
//! The last observation intentionally survives logout: logout removes credential authority, while
//! this token-free history lets a later login report that `Auto` selected a different store. A
//! successful later resolution or save replaces the observation for that credential identity.

use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_utils_home_dir::find_codex_home;
use codex_utils_path::write_atomically;
use serde::Deserialize;
use serde::Serialize;
use tracing::warn;

use super::ResolvedOAuthCredentialStore;
use super::compute_store_key;
use super::store_lock::OAuthStore;
use super::store_lock::OAuthStoreLock;

const RESOLUTION_STATE_FILENAME: &str = ".mcp-oauth-store-resolutions.json";
const RESOLUTION_STATE_VERSION: u32 = 1;
const RESOLUTION_STATE_LOCK_TIMEOUT: Duration = Duration::from_millis(/*millis*/ 250);
const RESOLUTION_CHANGED_METRIC: &str = "codex.mcp.oauth.store_resolution_changed";

#[derive(Debug, Clone, Copy)]
pub(super) enum StoreResolutionReason {
    AutoLoadFromKeyring,
    AutoLoadFromFileAfterMissingKeyring,
    AutoLoadFromFileAfterKeyringError,
    AutoSaveToKeyring,
    AutoSaveToFileAfterKeyringError,
    ConfiguredLoad,
    ConfiguredSave,
}

impl StoreResolutionReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::AutoLoadFromKeyring => "auto_load_keyring",
            Self::AutoLoadFromFileAfterMissingKeyring => "auto_load_file_keyring_missing",
            Self::AutoLoadFromFileAfterKeyringError => "auto_load_file_keyring_error",
            Self::AutoSaveToKeyring => "auto_save_keyring",
            Self::AutoSaveToFileAfterKeyringError => "auto_save_file_keyring_error",
            Self::ConfiguredLoad => "configured_load",
            Self::ConfiguredSave => "configured_save",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum ObservedStore {
    File,
    Keyring,
}

impl ObservedStore {
    fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Keyring => "keyring",
        }
    }
}

impl From<ResolvedOAuthCredentialStore> for ObservedStore {
    fn from(store: ResolvedOAuthCredentialStore) -> Self {
        match store {
            ResolvedOAuthCredentialStore::File => Self::File,
            ResolvedOAuthCredentialStore::Keyring(_) => Self::Keyring,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
struct StoreResolution {
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend: AuthKeyringBackendKind,
    resolved_store: ObservedStore,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct ResolutionState {
    #[serde(default = "resolution_state_version")]
    version: u32,
    #[serde(default)]
    resolutions: BTreeMap<String, StoreResolution>,
}

impl Default for ResolutionState {
    fn default() -> Self {
        Self {
            version: RESOLUTION_STATE_VERSION,
            resolutions: BTreeMap::new(),
        }
    }
}

fn resolution_state_version() -> u32 {
    RESOLUTION_STATE_VERSION
}

#[derive(Debug, PartialEq, Eq)]
struct StoreResolutionChange {
    previous: ObservedStore,
    current: ObservedStore,
}

pub(super) fn record_store_resolution(
    server_name: &str,
    url: &str,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend: AuthKeyringBackendKind,
    resolved_store: ResolvedOAuthCredentialStore,
    reason: StoreResolutionReason,
) {
    let result = match find_codex_home() {
        Ok(codex_home) => record_store_resolution_in(
            &codex_home,
            server_name,
            url,
            store_mode,
            keyring_backend,
            resolved_store,
        ),
        Err(error) => Err(error.into()),
    };

    match result {
        Ok(Some(change)) => {
            warn!(
                server_name,
                previous_store = change.previous.as_str(),
                resolved_store = change.current.as_str(),
                resolution_reason = reason.as_str(),
                "MCP OAuth Auto storage resolved differently than its previous use"
            );
            if let Some(metrics) = codex_otel::global() {
                let _ = metrics.counter(
                    RESOLUTION_CHANGED_METRIC,
                    /*inc*/ 1,
                    &[
                        ("previous_store", change.previous.as_str()),
                        ("resolved_store", change.current.as_str()),
                        ("reason", reason.as_str()),
                    ],
                );
            }
        }
        Ok(None) => {}
        Err(error) => {
            warn!(
                server_name,
                resolution_reason = reason.as_str(),
                error = %error,
                "failed to update MCP OAuth store resolution diagnostics"
            );
        }
    }
}

fn record_store_resolution_in(
    codex_home: &Path,
    server_name: &str,
    url: &str,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend: AuthKeyringBackendKind,
    resolved_store: ResolvedOAuthCredentialStore,
) -> Result<Option<StoreResolutionChange>> {
    // This short, best-effort lock serializes only the diagnostic map. It must not make an OAuth
    // operation wait behind the 60-second credential-store lock budget.
    let _lock = OAuthStoreLock::acquire_in(
        codex_home,
        OAuthStore::ResolutionState,
        RESOLUTION_STATE_LOCK_TIMEOUT,
    )?;
    let path = codex_home.join(RESOLUTION_STATE_FILENAME);
    let mut state = read_resolution_state(&path)?;
    anyhow::ensure!(
        state.version == RESOLUTION_STATE_VERSION,
        "unsupported MCP OAuth store resolution state version {}",
        state.version
    );

    let store_key = compute_store_key(server_name, url)?;
    let current = StoreResolution {
        store_mode,
        keyring_backend,
        resolved_store: resolved_store.into(),
    };
    let previous = state.resolutions.get(&store_key).copied();
    if previous == Some(current) {
        return Ok(None);
    }

    state.resolutions.insert(store_key, current);
    let serialized = serde_json::to_string(&state)
        .context("failed to serialize MCP OAuth store resolution diagnostics")?;
    write_atomically(&path, &serialized).with_context(|| {
        format!(
            "failed to write MCP OAuth store resolution diagnostics at {}",
            path.display()
        )
    })?;

    // Explicit File/Keyring configuration and keyring-backend changes are intentional authority
    // changes, so they reset the comparison baseline. Only repeated Auto resolution under the
    // same backend indicates the availability drift this diagnostic is meant to surface.
    Ok(previous.and_then(|previous| {
        (previous.store_mode == OAuthCredentialsStoreMode::Auto
            && current.store_mode == OAuthCredentialsStoreMode::Auto
            && previous.keyring_backend == current.keyring_backend
            && previous.resolved_store != current.resolved_store)
            .then_some(StoreResolutionChange {
                previous: previous.resolved_store,
                current: current.resolved_store,
            })
    }))
}

fn read_resolution_state(path: &Path) -> Result<ResolutionState> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(ResolutionState::default()),
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "failed to read MCP OAuth store resolution diagnostics at {}",
                    path.display()
                )
            });
        }
    };
    serde_json::from_str(&contents).with_context(|| {
        format!(
            "failed to parse MCP OAuth store resolution diagnostics at {}",
            path.display()
        )
    })
}
