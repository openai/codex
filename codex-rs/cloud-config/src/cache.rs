use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use chrono::DateTime;
use chrono::Duration as ChronoDuration;
use chrono::Utc;
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
const CLOUD_CONFIG_BUNDLE_CACHE_TTL: Duration = Duration::from_secs(30 * 60);
const CLOUD_CONFIG_BUNDLE_CACHE_WRITE_HMAC_KEY: &[u8] =
    b"codex-cloud-config-bundle-cache-v1-6160ae70-bcfd-4ca8-a99b-40f73b3b072e";
const CLOUD_CONFIG_BUNDLE_CACHE_READ_HMAC_KEYS: &[&[u8]] =
    &[CLOUD_CONFIG_BUNDLE_CACHE_WRITE_HMAC_KEY];

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
pub(super) struct CloudConfigBundleCache {
    path: PathBuf,
}

impl CloudConfigBundleCache {
    pub(super) fn new(codex_home: PathBuf) -> Self {
        Self {
            path: codex_home.join(CLOUD_CONFIG_BUNDLE_CACHE_FILENAME),
        }
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
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

        if cache_file.signed_payload.expires_at <= Utc::now() {
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
mod tests {
    use super::*;
    use codex_config::CloudConfigFragment;
    use codex_config::CloudConfigTomlBundle;
    use codex_config::CloudRequirementsFragment;
    use codex_config::CloudRequirementsTomlBundle;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    fn test_bundle() -> CloudConfigBundle {
        CloudConfigBundle {
            config_toml: CloudConfigTomlBundle {
                enterprise_managed: vec![CloudConfigFragment {
                    id: "cfg_1".to_string(),
                    name: "Base config".to_string(),
                    contents: "model = \"gpt-5\"".to_string(),
                }],
            },
            requirements_toml: CloudRequirementsTomlBundle {
                enterprise_managed: vec![CloudRequirementsFragment {
                    id: "req_1".to_string(),
                    name: "Base requirements".to_string(),
                    contents: "allowed_approval_policies = [\"never\"]".to_string(),
                }],
            },
        }
    }

    fn signed_cache_file(
        signed_payload: CloudConfigBundleCacheSignedPayload,
    ) -> CloudConfigBundleCacheFile {
        let payload_bytes = cache_payload_bytes(&signed_payload).expect("payload bytes");
        CloudConfigBundleCacheFile {
            signature: sign_cache_payload(&payload_bytes).expect("signature"),
            signed_payload,
        }
    }

    fn valid_signed_payload() -> CloudConfigBundleCacheSignedPayload {
        let cached_at = Utc::now();
        CloudConfigBundleCacheSignedPayload {
            version: CLOUD_CONFIG_BUNDLE_CACHE_VERSION,
            cached_at,
            expires_at: cached_at + ChronoDuration::minutes(30),
            chatgpt_user_id: Some("user-12345".to_string()),
            account_id: Some("account-12345".to_string()),
            bundle: test_bundle(),
        }
    }

    fn write_cache_file(cache: &CloudConfigBundleCache, cache_file: &CloudConfigBundleCacheFile) {
        std::fs::write(
            cache.path(),
            serde_json::to_vec_pretty(cache_file).expect("serialize cache"),
        )
        .expect("write cache");
    }

    #[tokio::test]
    async fn save_writes_signed_payload_and_loads_for_matching_identity() {
        let codex_home = tempdir().expect("tempdir");
        let cache = CloudConfigBundleCache::new(codex_home.path().to_path_buf());
        let bundle = test_bundle();

        cache
            .save(
                Some("user-12345".to_string()),
                Some("account-12345".to_string()),
                bundle.clone(),
            )
            .await
            .expect("save cache");

        let cache_file: CloudConfigBundleCacheFile =
            serde_json::from_slice(&std::fs::read(cache.path()).expect("read cache"))
                .expect("parse cache");
        assert_eq!(cache_file.signed_payload.version, 1);
        assert_eq!(
            cache_file.signed_payload.chatgpt_user_id,
            Some("user-12345".to_string())
        );
        assert_eq!(
            cache_file.signed_payload.account_id,
            Some("account-12345".to_string())
        );
        assert_eq!(cache_file.signed_payload.bundle, bundle);
        assert!(
            cache_file.signed_payload.expires_at
                <= cache_file.signed_payload.cached_at + ChronoDuration::minutes(30)
        );
        assert!(cache_file.signed_payload.expires_at > cache_file.signed_payload.cached_at);
        assert!(verify_cache_signature(
            &cache_payload_bytes(&cache_file.signed_payload).expect("payload bytes"),
            &cache_file.signature,
        ));

        assert_eq!(
            cache.load(Some("user-12345"), Some("account-12345")).await,
            Ok(cache_file.signed_payload)
        );
    }

    #[tokio::test]
    async fn load_rejects_missing_request_identity_before_reading_cache_file() {
        let codex_home = tempdir().expect("tempdir");
        let cache = CloudConfigBundleCache::new(codex_home.path().to_path_buf());

        assert_eq!(
            cache.load(None, Some("account-12345")).await,
            Err(CacheLoadStatus::AuthIdentityIncomplete)
        );
        assert_eq!(
            cache.load(Some("user-12345"), None).await,
            Err(CacheLoadStatus::AuthIdentityIncomplete)
        );
    }

    #[tokio::test]
    async fn load_reports_missing_and_malformed_cache_files() {
        let codex_home = tempdir().expect("tempdir");
        let cache = CloudConfigBundleCache::new(codex_home.path().to_path_buf());

        assert_eq!(
            cache.load(Some("user-12345"), Some("account-12345")).await,
            Err(CacheLoadStatus::CacheFileNotFound)
        );

        std::fs::write(cache.path(), "{").expect("write malformed cache");
        assert!(matches!(
            cache.load(Some("user-12345"), Some("account-12345")).await,
            Err(CacheLoadStatus::CacheParseFailed(_))
        ));
    }

    #[tokio::test]
    async fn load_rejects_tampered_payload() {
        let codex_home = tempdir().expect("tempdir");
        let cache = CloudConfigBundleCache::new(codex_home.path().to_path_buf());
        let mut cache_file = signed_cache_file(valid_signed_payload());
        cache_file
            .signed_payload
            .bundle
            .requirements_toml
            .enterprise_managed[0]
            .contents = "allowed_approval_policies = [\"on-request\"]".to_string();
        write_cache_file(&cache, &cache_file);

        assert_eq!(
            cache.load(Some("user-12345"), Some("account-12345")).await,
            Err(CacheLoadStatus::CacheSignatureInvalid)
        );
    }

    #[tokio::test]
    async fn load_rejects_cache_for_incomplete_or_different_identity() {
        let codex_home = tempdir().expect("tempdir");
        let cache = CloudConfigBundleCache::new(codex_home.path().to_path_buf());
        let cache_file = signed_cache_file(valid_signed_payload());
        write_cache_file(&cache, &cache_file);

        assert_eq!(
            cache.load(Some("user-99999"), Some("account-12345")).await,
            Err(CacheLoadStatus::CacheIdentityMismatch)
        );

        let mut signed_payload = valid_signed_payload();
        signed_payload.chatgpt_user_id = None;
        write_cache_file(&cache, &signed_cache_file(signed_payload));

        assert_eq!(
            cache.load(Some("user-12345"), Some("account-12345")).await,
            Err(CacheLoadStatus::CacheIdentityIncomplete)
        );
    }

    #[tokio::test]
    async fn load_rejects_expired_cache() {
        let codex_home = tempdir().expect("tempdir");
        let cache = CloudConfigBundleCache::new(codex_home.path().to_path_buf());
        let mut signed_payload = valid_signed_payload();
        signed_payload.expires_at = Utc::now() - ChronoDuration::seconds(1);
        write_cache_file(&cache, &signed_cache_file(signed_payload));

        assert_eq!(
            cache.load(Some("user-12345"), Some("account-12345")).await,
            Err(CacheLoadStatus::CacheExpired)
        );
    }

    #[tokio::test]
    async fn load_rejects_unsupported_cache_version() {
        let codex_home = tempdir().expect("tempdir");
        let cache = CloudConfigBundleCache::new(codex_home.path().to_path_buf());
        let mut signed_payload = valid_signed_payload();
        signed_payload.version = 2;
        write_cache_file(&cache, &signed_cache_file(signed_payload));

        assert_eq!(
            cache.load(Some("user-12345"), Some("account-12345")).await,
            Err(CacheLoadStatus::CacheVersionUnsupported(2))
        );
    }
}
