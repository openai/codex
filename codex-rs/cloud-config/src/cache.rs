//! Signed on-disk cache for cloud config bundles.
//!
//! The cache is scoped to the authenticated ChatGPT user and account, has a
//! short TTL, and is HMAC-signed so malformed or edited files fail closed.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use chrono::DateTime;
use chrono::Duration as ChronoDuration;
use chrono::Utc;
use codex_config::AbsolutePathBuf;
use codex_config::CloudConfigBundle;
use hmac::Hmac;
use hmac::Mac;
use serde::Deserialize;
use serde::Serialize;
use sha2::Sha256;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;
use tokio::fs;

const CLOUD_CONFIG_BUNDLE_CACHE_VERSION: u32 = 1;
pub(super) const CLOUD_CONFIG_BUNDLE_CACHE_FILENAME: &str = "cloud-config-bundle-cache.json";
pub(super) const CLOUD_CONFIG_BUNDLE_CACHE_LOCK_FILENAME: &str = "cloud-config-bundle-cache.lock";
pub(super) const CLOUD_CONFIG_BUNDLE_CACHE_TTL: Duration = Duration::from_secs(15 * 60);
const CLOUD_CONFIG_BUNDLE_CACHE_WRITE_HMAC_KEY: &[u8] =
    b"codex-cloud-config-bundle-cache-v1-6160ae70-bcfd-4ca8-a99b-40f73b3b072e";
const CLOUD_CONFIG_BUNDLE_CACHE_READ_HMAC_KEYS: &[&[u8]] =
    &[CLOUD_CONFIG_BUNDLE_CACHE_WRITE_HMAC_KEY];

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
pub(super) struct CloudConfigBundleCache {
    path: AbsolutePathBuf,
}

pub(super) enum CacheLockAttempt {
    Acquired(std::fs::File),
    Contended,
}

impl CloudConfigBundleCache {
    pub(super) fn new(codex_home: AbsolutePathBuf) -> Self {
        Self {
            path: codex_home.join(CLOUD_CONFIG_BUNDLE_CACHE_FILENAME),
        }
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) async fn try_acquire_lock(&self) -> std::io::Result<CacheLockAttempt> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let lock_path = self
            .path
            .as_path()
            .parent()
            .map(|parent| parent.join(CLOUD_CONFIG_BUNDLE_CACHE_LOCK_FILENAME))
            .unwrap_or_else(|| PathBuf::from(CLOUD_CONFIG_BUNDLE_CACHE_LOCK_FILENAME));
        // The file intentionally persists between owners. Lock ownership is
        // attached to this handle and is released when the handle is dropped.
        let lock_file = tokio::task::spawn_blocking(move || {
            std::fs::File::options()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(lock_path)
        })
        .await
        .map_err(|err| std::io::Error::other(format!("cache lock task failed: {err}")))??;

        match lock_file.try_lock() {
            Ok(()) => Ok(CacheLockAttempt::Acquired(lock_file)),
            Err(std::fs::TryLockError::WouldBlock) => Ok(CacheLockAttempt::Contended),
            Err(std::fs::TryLockError::Error(err)) => Err(err),
        }
    }

    pub(super) async fn load(
        &self,
        chatgpt_user_id: Option<&str>,
        account_id: Option<&str>,
    ) -> Result<CloudConfigBundleCacheSignedPayload, CacheLoadStatus> {
        let (Some(chatgpt_user_id), Some(account_id)) = (chatgpt_user_id, account_id) else {
            return Err(CacheLoadStatus::AuthIdentityIncomplete);
        };

        let bytes = match fs::read(&self.path).await {
            Ok(bytes) => bytes,
            Err(err) => {
                if err.kind() != std::io::ErrorKind::NotFound {
                    return Err(CacheLoadStatus::CacheReadFailed(err.to_string()));
                }
                return Err(CacheLoadStatus::CacheFileNotFound);
            }
        };

        let cache_file: CloudConfigBundleCacheFile = match serde_json::from_slice(&bytes) {
            Ok(cache_file) => cache_file,
            Err(err) => {
                return Err(CacheLoadStatus::CacheParseFailed(err.to_string()));
            }
        };
        let payload_bytes = match cache_payload_bytes(&cache_file.signed_payload) {
            Some(payload_bytes) => payload_bytes,
            None => {
                return Err(CacheLoadStatus::CacheParseFailed(
                    "failed to serialize cache payload".to_string(),
                ));
            }
        };
        if !verify_cache_signature(&payload_bytes, &cache_file.signature) {
            return Err(CacheLoadStatus::CacheSignatureInvalid);
        }
        if cache_file.signed_payload.version != CLOUD_CONFIG_BUNDLE_CACHE_VERSION {
            return Err(CacheLoadStatus::CacheVersionUnsupported(
                cache_file.signed_payload.version,
            ));
        }

        let (Some(cached_chatgpt_user_id), Some(cached_account_id)) = (
            cache_file.signed_payload.chatgpt_user_id.as_deref(),
            cache_file.signed_payload.account_id.as_deref(),
        ) else {
            return Err(CacheLoadStatus::CacheIdentityIncomplete);
        };

        if cached_chatgpt_user_id != chatgpt_user_id || cached_account_id != account_id {
            return Err(CacheLoadStatus::CacheIdentityMismatch);
        }

        let now = Utc::now();
        let cache_age = now
            .signed_duration_since(cache_file.signed_payload.cached_at)
            .to_std();
        if cache_file.signed_payload.expires_at <= now
            || !matches!(cache_age, Ok(cache_age) if cache_age < CLOUD_CONFIG_BUNDLE_CACHE_TTL)
        {
            return Err(CacheLoadStatus::CacheExpired);
        }

        Ok(cache_file.signed_payload)
    }

    pub(super) fn log_load_status(&self, status: &CacheLoadStatus) {
        if matches!(status, CacheLoadStatus::CacheFileNotFound) {
            return;
        }

        let warn = matches!(
            status,
            CacheLoadStatus::CacheReadFailed(_)
                | CacheLoadStatus::CacheParseFailed(_)
                | CacheLoadStatus::CacheSignatureInvalid
        );

        if warn {
            tracing::warn!(path = %self.path.display(), "{status}");
        } else {
            tracing::info!(path = %self.path.display(), "{status}");
        }
    }

    pub(super) async fn save(
        &self,
        chatgpt_user_id: Option<String>,
        account_id: Option<String>,
        bundle: CloudConfigBundle,
    ) -> Result<(), CloudConfigBundleCacheError> {
        let now = Utc::now();
        let expires_at = now
            .checked_add_signed(
                ChronoDuration::from_std(CLOUD_CONFIG_BUNDLE_CACHE_TTL)
                    .map_err(|_| CloudConfigBundleCacheError)?,
            )
            .ok_or(CloudConfigBundleCacheError)?;
        let signed_payload = CloudConfigBundleCacheSignedPayload {
            version: CLOUD_CONFIG_BUNDLE_CACHE_VERSION,
            cached_at: now,
            expires_at,
            chatgpt_user_id,
            account_id,
            bundle,
        };
        let payload_bytes =
            cache_payload_bytes(&signed_payload).ok_or(CloudConfigBundleCacheError)?;
        let serialized = serde_json::to_vec_pretty(&CloudConfigBundleCacheFile {
            signature: sign_cache_payload(&payload_bytes).ok_or(CloudConfigBundleCacheError)?,
            signed_payload,
        })
        .map_err(|_| CloudConfigBundleCacheError)?;

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|_| CloudConfigBundleCacheError)?;
        }

        fs::write(&self.path, serialized)
            .await
            .map_err(|_| CloudConfigBundleCacheError)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(super) enum CacheLoadStatus {
    #[error("Skipping cloud config bundle cache read because auth identity is incomplete.")]
    AuthIdentityIncomplete,
    #[error("Cloud config bundle cache file not found.")]
    CacheFileNotFound,
    #[error("Failed to read cloud config bundle cache: {0}.")]
    CacheReadFailed(String),
    #[error("Failed to parse cloud config bundle cache: {0}.")]
    CacheParseFailed(String),
    #[error("Cloud config bundle cache failed signature verification.")]
    CacheSignatureInvalid,
    #[error("Ignoring cloud config bundle cache because cached identity is incomplete.")]
    CacheIdentityIncomplete,
    #[error("Ignoring cloud config bundle cache for different auth identity.")]
    CacheIdentityMismatch,
    #[error("Ignoring cloud config bundle cache with unsupported version {0}.")]
    CacheVersionUnsupported(u32),
    #[error("Cloud config bundle cache expired.")]
    CacheExpired,
    #[error("Ignoring cloud config bundle cache because the cached bundle is invalid.")]
    CacheInvalidBundle,
}

#[derive(Debug, Error)]
#[error("failed to write cloud config bundle cache")]
pub(super) struct CloudConfigBundleCacheError;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct CloudConfigBundleCacheFile {
    pub(super) signed_payload: CloudConfigBundleCacheSignedPayload,
    pub(super) signature: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct CloudConfigBundleCacheSignedPayload {
    pub(super) version: u32,
    pub(super) cached_at: DateTime<Utc>,
    pub(super) expires_at: DateTime<Utc>,
    pub(super) chatgpt_user_id: Option<String>,
    pub(super) account_id: Option<String>,
    pub(super) bundle: CloudConfigBundle,
}

pub(super) fn cache_payload_bytes(
    payload: &CloudConfigBundleCacheSignedPayload,
) -> Option<Vec<u8>> {
    serde_json::to_vec(&payload).ok()
}

pub(super) fn sign_cache_payload(payload_bytes: &[u8]) -> Option<String> {
    let mut mac = HmacSha256::new_from_slice(CLOUD_CONFIG_BUNDLE_CACHE_WRITE_HMAC_KEY).ok()?;
    mac.update(payload_bytes);
    let signature = mac.finalize().into_bytes();
    Some(BASE64_STANDARD.encode(signature))
}

pub(super) fn verify_cache_signature(payload_bytes: &[u8], signature: &str) -> bool {
    let signature_bytes = match BASE64_STANDARD.decode(signature) {
        Ok(signature_bytes) => signature_bytes,
        Err(_) => return false,
    };

    CLOUD_CONFIG_BUNDLE_CACHE_READ_HMAC_KEYS
        .iter()
        .any(|key| verify_cache_signature_with_key(payload_bytes, &signature_bytes, key))
}

fn verify_cache_signature_with_key(
    payload_bytes: &[u8],
    signature_bytes: &[u8],
    key: &[u8],
) -> bool {
    let mut mac = match HmacSha256::new_from_slice(key) {
        Ok(mac) => mac,
        Err(_) => return false,
    };
    mac.update(payload_bytes);
    mac.verify_slice(signature_bytes).is_ok()
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
