//! Cloud-hosted config requirements for Codex.
//!
//! This crate fetches `requirements.toml` data from the backend as an alternative to loading it
//! from the local filesystem. It only applies to Business (aka Enterprise CBP) or Enterprise ChatGPT
//! customers.
//!
//! Today, fetching is best-effort: on error or timeout, Codex continues without cloud requirements.
//! We expect to tighten this so that Enterprise ChatGPT customers must successfully fetch these
//! requirements before Codex will run.

use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use chrono::DateTime;
use chrono::Duration as ChronoDuration;
use chrono::Utc;
use codex_backend_client::Client as BackendClient;
use codex_core::AuthManager;
use codex_core::auth::CodexAuth;
use codex_core::config_loader::CloudRequirementsLoader;
use codex_core::config_loader::ConfigRequirementsToml;
use codex_core::util::backoff;
use codex_protocol::account::PlanType;
use hmac::Hmac;
use hmac::Mac;
use serde::Deserialize;
use serde::Serialize;
use sha2::Sha256;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use thiserror::Error;
use tokio::fs;
use tokio::time::sleep;
use tokio::time::timeout;

const CLOUD_REQUIREMENTS_TIMEOUT: Duration = Duration::from_secs(15);
const CLOUD_REQUIREMENTS_MAX_ATTEMPTS: usize = 5;
const CLOUD_REQUIREMENTS_CACHE_FILENAME: &str = "cloud-requirements-cache.json";
const CLOUD_REQUIREMENTS_CACHE_TTL: Duration = Duration::from_secs(60 * 60);
const CLOUD_REQUIREMENTS_CACHE_WRITE_HMAC_KEY: &[u8] =
    b"codex-cloud-requirements-cache-v3-064f8542-75b4-494c-a294-97d3ce597271";
const CLOUD_REQUIREMENTS_CACHE_READ_HMAC_KEYS: &[&[u8]] =
    &[CLOUD_REQUIREMENTS_CACHE_WRITE_HMAC_KEY];

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FetchCloudRequirementsStatus {
    BackendClientInit,
    Request,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
enum CacheLoadStatus {
    #[error("Skipping cloud requirements cache read because auth identity is incomplete.")]
    AuthIdentityIncomplete,
    #[error("Cloud requirements cache file not found.")]
    CacheFileNotFound,
    #[error("Failed to read cloud requirements cache: {0}.")]
    CacheReadFailed(String),
    #[error("Failed to parse cloud requirements cache: {0}.")]
    CacheParseFailed(String),
    #[error("Cloud requirements cache failed signature verification.")]
    CacheSignatureInvalid,
    #[error("Ignoring cloud requirements cache because cached identity is incomplete.")]
    CacheIdentityIncomplete,
    #[error("Ignoring cloud requirements cache for different auth identity.")]
    CacheIdentityMismatch,
    #[error("Cloud requirements cache expired.")]
    CacheExpired,
}

#[derive(Debug, Error)]
enum CloudRequirementsError {
    #[error("failed to write cloud requirements cache")]
    CacheWrite,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CloudRequirementsCacheFile {
    signed_payload: CloudRequirementsCacheSignedPayload,
    signature: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CloudRequirementsCacheSignedPayload {
    cached_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    chatgpt_user_id: Option<String>,
    account_id: Option<String>,
    contents: Option<String>,
}

impl CloudRequirementsCacheSignedPayload {
    fn requirements(&self) -> Option<ConfigRequirementsToml> {
        self.contents
            .as_deref()
            .and_then(|contents| parse_cloud_requirements(contents).ok().flatten())
    }
}
fn sign_cache_payload(payload_bytes: &[u8]) -> Option<String> {
    let mut mac = HmacSha256::new_from_slice(CLOUD_REQUIREMENTS_CACHE_WRITE_HMAC_KEY).ok()?;
    mac.update(payload_bytes);
    let signature = mac.finalize().into_bytes();
    Some(BASE64_STANDARD.encode(signature))
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

fn verify_cache_signature(payload_bytes: &[u8], signature: &str) -> bool {
    let signature_bytes = match BASE64_STANDARD.decode(signature) {
        Ok(signature_bytes) => signature_bytes,
        Err(_) => return false,
    };

    CLOUD_REQUIREMENTS_CACHE_READ_HMAC_KEYS
        .iter()
        .any(|key| verify_cache_signature_with_key(payload_bytes, &signature_bytes, key))
}

fn cache_payload_bytes(payload: &CloudRequirementsCacheSignedPayload) -> Option<Vec<u8>> {
    serde_json::to_vec(&payload).ok()
}

#[async_trait]
trait RequirementsFetcher: Send + Sync {
    /// Returns `Ok(None)` when there are no cloud requirements for the account.
    ///
    /// Returning `Err` indicates cloud requirements could not be fetched.
    async fn fetch_requirements(
        &self,
        auth: &CodexAuth,
    ) -> Result<Option<String>, FetchCloudRequirementsStatus>;
}

struct BackendRequirementsFetcher {
    base_url: String,
}

impl BackendRequirementsFetcher {
    fn new(base_url: String) -> Self {
        Self { base_url }
    }
}

#[async_trait]
impl RequirementsFetcher for BackendRequirementsFetcher {
    async fn fetch_requirements(
        &self,
        auth: &CodexAuth,
    ) -> Result<Option<String>, FetchCloudRequirementsStatus> {
        let client = BackendClient::from_auth(self.base_url.clone(), auth)
            .inspect_err(|err| {
                tracing::warn!(
                    error = %err,
                    "Failed to construct backend client for cloud requirements"
                );
            })
            .map_err(|_| FetchCloudRequirementsStatus::BackendClientInit)?;

        let response = client
            .get_config_requirements_file()
            .await
            .inspect_err(|err| tracing::warn!(error = %err, "Failed to fetch cloud requirements"))
            .map_err(|_| FetchCloudRequirementsStatus::Request)?;

        let Some(contents) = response.contents else {
            tracing::info!(
                "Cloud requirements response missing contents; treating as no requirements"
            );
            return Ok(None);
        };

        Ok(Some(contents))
    }
}

struct CloudRequirementsService {
    auth_manager: Arc<AuthManager>,
    fetcher: Arc<dyn RequirementsFetcher>,
    cache_path: PathBuf,
    timeout: Duration,
}

impl CloudRequirementsService {
    fn new(
        auth_manager: Arc<AuthManager>,
        fetcher: Arc<dyn RequirementsFetcher>,
        codex_home: PathBuf,
        timeout: Duration,
    ) -> Self {
        Self {
            auth_manager,
            fetcher,
            cache_path: codex_home.join(CLOUD_REQUIREMENTS_CACHE_FILENAME),
            timeout,
        }
    }

    async fn fetch_with_timeout(&self) -> Option<ConfigRequirementsToml> {
        let _timer =
            codex_otel::start_global_timer("codex.cloud_requirements.fetch.duration_ms", &[]);
        let started_at = Instant::now();
        let result = timeout(self.timeout, self.fetch())
            .await
            .inspect_err(|_| {
                tracing::warn!("Timed out waiting for cloud requirements; continuing without them");
            })
            .ok()?;

        match result.as_ref() {
            Some(requirements) => {
                tracing::info!(
                    elapsed_ms = started_at.elapsed().as_millis(),
                    requirements = ?requirements,
                    "Cloud requirements load completed"
                );
            }
            None => {
                tracing::info!(
                    elapsed_ms = started_at.elapsed().as_millis(),
                    "Cloud requirements load completed (none)"
                );
            }
        }

        result
    }

    async fn fetch(&self) -> Option<ConfigRequirementsToml> {
        let auth = self.auth_manager.auth().await?;
        if !auth.is_chatgpt_auth()
            || !matches!(
                auth.account_plan_type(),
                Some(PlanType::Business | PlanType::Enterprise)
            )
        {
            return None;
        }
        let token_data = auth.get_token_data().ok();
        let chatgpt_user_id = token_data
            .as_ref()
            .and_then(|token_data| token_data.id_token.chatgpt_user_id.as_deref());
        let account_id = auth.get_account_id();
        let account_id = account_id.as_deref();

        match self.load_cache(chatgpt_user_id, account_id).await {
            Ok(signed_payload) => {
                tracing::info!(
                    path = %self.cache_path.display(),
                    "Using cached cloud requirements"
                );
                return signed_payload.requirements();
            }
            Err(cache_load_status) => {
                self.log_cache_load_status(&cache_load_status);
            }
        }

        self.fetch_with_retries(&auth, chatgpt_user_id, account_id)
            .await?
    }

    async fn fetch_with_retries(
        &self,
        auth: &CodexAuth,
        chatgpt_user_id: Option<&str>,
        account_id: Option<&str>,
    ) -> Option<Option<ConfigRequirementsToml>> {
        for attempt in 1..=CLOUD_REQUIREMENTS_MAX_ATTEMPTS {
            let contents = match self.fetcher.fetch_requirements(auth).await {
                Ok(contents) => contents,
                Err(status) => {
                    if attempt < CLOUD_REQUIREMENTS_MAX_ATTEMPTS {
                        tracing::warn!(
                            status = ?status,
                            attempt,
                            max_attempts = CLOUD_REQUIREMENTS_MAX_ATTEMPTS,
                            "Failed to fetch cloud requirements; retrying"
                        );
                        sleep(backoff(attempt as u64)).await;
                    }
                    continue;
                }
            };

            let requirements = match contents.as_deref() {
                Some(contents) => match parse_cloud_requirements(contents) {
                    Ok(requirements) => requirements,
                    Err(err) => {
                        tracing::warn!(error = %err, "Failed to parse cloud requirements");
                        return None;
                    }
                },
                None => None,
            };

            if let Err(err) = self
                .save_cache(
                    chatgpt_user_id.map(str::to_owned),
                    account_id.map(str::to_owned),
                    contents,
                )
                .await
            {
                tracing::warn!(error = %err, "Failed to write cloud requirements cache");
            }

            return Some(requirements);
        }

        None
    }

    async fn load_cache(
        &self,
        chatgpt_user_id: Option<&str>,
        account_id: Option<&str>,
    ) -> Result<CloudRequirementsCacheSignedPayload, CacheLoadStatus> {
        let (Some(chatgpt_user_id), Some(account_id)) = (chatgpt_user_id, account_id) else {
            return Err(CacheLoadStatus::AuthIdentityIncomplete);
        };

        let bytes = match fs::read(&self.cache_path).await {
            Ok(bytes) => bytes,
            Err(err) => {
                if err.kind() != std::io::ErrorKind::NotFound {
                    return Err(CacheLoadStatus::CacheReadFailed(err.to_string()));
                }
                return Err(CacheLoadStatus::CacheFileNotFound);
            }
        };

        let cache_file: CloudRequirementsCacheFile = match serde_json::from_slice(&bytes) {
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

    fn log_cache_load_status(&self, status: &CacheLoadStatus) {
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
            tracing::warn!(path = %self.cache_path.display(), "{status}");
        } else {
            tracing::info!(path = %self.cache_path.display(), "{status}");
        }
    }

    async fn save_cache(
        &self,
        chatgpt_user_id: Option<String>,
        account_id: Option<String>,
        contents: Option<String>,
    ) -> Result<(), CloudRequirementsError> {
        let now = Utc::now();
        let expires_at = now
            .checked_add_signed(
                ChronoDuration::from_std(CLOUD_REQUIREMENTS_CACHE_TTL)
                    .map_err(|_| CloudRequirementsError::CacheWrite)?,
            )
            .ok_or(CloudRequirementsError::CacheWrite)?;
        let signed_payload = CloudRequirementsCacheSignedPayload {
            cached_at: now,
            expires_at,
            chatgpt_user_id,
            account_id,
            contents,
        };
        let payload_bytes =
            cache_payload_bytes(&signed_payload).ok_or(CloudRequirementsError::CacheWrite)?;
        let serialized = serde_json::to_vec_pretty(&CloudRequirementsCacheFile {
            signature: sign_cache_payload(&payload_bytes)
                .ok_or(CloudRequirementsError::CacheWrite)?,
            signed_payload,
        })
        .map_err(|_| CloudRequirementsError::CacheWrite)?;

        if let Some(parent) = self.cache_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|_| CloudRequirementsError::CacheWrite)?;
        }

        fs::write(&self.cache_path, serialized)
            .await
            .map_err(|_| CloudRequirementsError::CacheWrite)?;
        Ok(())
    }
}

pub fn cloud_requirements_loader(
    auth_manager: Arc<AuthManager>,
    chatgpt_base_url: String,
    codex_home: PathBuf,
) -> CloudRequirementsLoader {
    let service = CloudRequirementsService::new(
        auth_manager,
        Arc::new(BackendRequirementsFetcher::new(chatgpt_base_url)),
        codex_home,
        CLOUD_REQUIREMENTS_TIMEOUT,
    );
    let task = tokio::spawn(async move { service.fetch_with_timeout().await });
    CloudRequirementsLoader::new(async move {
        task.await
            .inspect_err(|err| tracing::warn!(error = %err, "Cloud requirements task failed"))
            .ok()
            .flatten()
    })
}

fn parse_cloud_requirements(
    contents: &str,
) -> Result<Option<ConfigRequirementsToml>, toml::de::Error> {
    if contents.trim().is_empty() {
        return Ok(None);
    }

    let requirements: ConfigRequirementsToml = toml::from_str(contents)?;
    if requirements.is_empty() {
        Ok(None)
    } else {
        Ok(Some(requirements))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use codex_core::auth::AuthCredentialsStoreMode;
    use codex_protocol::protocol::AskForApproval;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::collections::VecDeque;
    use std::future::pending;
    use std::path::Path;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use tempfile::tempdir;

    fn write_auth_json(codex_home: &Path, value: serde_json::Value) -> std::io::Result<()> {
        std::fs::write(codex_home.join("auth.json"), serde_json::to_string(&value)?)?;
        Ok(())
    }

    fn auth_manager_with_api_key() -> Arc<AuthManager> {
        let tmp = tempdir().expect("tempdir");
        let auth_json = json!({
            "OPENAI_API_KEY": "sk-test-key",
            "tokens": null,
            "last_refresh": null,
        });
        write_auth_json(tmp.path(), auth_json).expect("write auth");
        Arc::new(AuthManager::new(
            tmp.path().to_path_buf(),
            false,
            AuthCredentialsStoreMode::File,
        ))
    }

    fn auth_manager_with_plan_and_identity(
        plan_type: &str,
        chatgpt_user_id: Option<&str>,
        account_id: Option<&str>,
    ) -> Arc<AuthManager> {
        let tmp = tempdir().expect("tempdir");
        let header = json!({ "alg": "none", "typ": "JWT" });
        let auth_payload = json!({
            "chatgpt_plan_type": plan_type,
            "chatgpt_user_id": chatgpt_user_id,
            "user_id": chatgpt_user_id,
        });
        let payload = json!({
            "email": "user@example.com",
            "https://api.openai.com/auth": auth_payload,
        });
        let header_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).expect("header"));
        let payload_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).expect("payload"));
        let signature_b64 = URL_SAFE_NO_PAD.encode(b"sig");
        let fake_jwt = format!("{header_b64}.{payload_b64}.{signature_b64}");

        let auth_json = json!({
            "OPENAI_API_KEY": null,
            "tokens": {
                "id_token": fake_jwt,
                "access_token": "test-access-token",
                "refresh_token": "test-refresh-token",
                "account_id": account_id,
            },
            "last_refresh": "2025-01-01T00:00:00Z",
        });
        write_auth_json(tmp.path(), auth_json).expect("write auth");
        Arc::new(AuthManager::new(
            tmp.path().to_path_buf(),
            false,
            AuthCredentialsStoreMode::File,
        ))
    }

    fn auth_manager_with_plan(plan_type: &str) -> Arc<AuthManager> {
        auth_manager_with_plan_and_identity(plan_type, Some("user-12345"), Some("account-12345"))
    }

    fn parse_for_fetch(contents: Option<&str>) -> Option<ConfigRequirementsToml> {
        contents.and_then(|contents| parse_cloud_requirements(contents).ok().flatten())
    }

    struct StaticFetcher {
        contents: Option<String>,
    }

    #[async_trait::async_trait]
    impl RequirementsFetcher for StaticFetcher {
        async fn fetch_requirements(
            &self,
            _auth: &CodexAuth,
        ) -> Result<Option<String>, FetchCloudRequirementsStatus> {
            Ok(self.contents.clone())
        }
    }

    struct PendingFetcher;

    #[async_trait::async_trait]
    impl RequirementsFetcher for PendingFetcher {
        async fn fetch_requirements(
            &self,
            _auth: &CodexAuth,
        ) -> Result<Option<String>, FetchCloudRequirementsStatus> {
            pending::<()>().await;
            Ok(None)
        }
    }

    struct SequenceFetcher {
        responses:
            tokio::sync::Mutex<VecDeque<Result<Option<String>, FetchCloudRequirementsStatus>>>,
        request_count: AtomicUsize,
    }

    impl SequenceFetcher {
        fn new(responses: Vec<Result<Option<String>, FetchCloudRequirementsStatus>>) -> Self {
            Self {
                responses: tokio::sync::Mutex::new(VecDeque::from(responses)),
                request_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait::async_trait]
    impl RequirementsFetcher for SequenceFetcher {
        async fn fetch_requirements(
            &self,
            _auth: &CodexAuth,
        ) -> Result<Option<String>, FetchCloudRequirementsStatus> {
            self.request_count.fetch_add(1, Ordering::SeqCst);
            let mut responses = self.responses.lock().await;
            responses.pop_front().unwrap_or(Ok(None))
        }
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_skips_non_chatgpt_auth() {
        let auth_manager = auth_manager_with_api_key();
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager,
            Arc::new(StaticFetcher { contents: None }),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        let result = service.fetch().await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_skips_non_business_or_enterprise_plan() {
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("pro"),
            Arc::new(StaticFetcher { contents: None }),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        let result = service.fetch().await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_allows_business_plan() {
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("business"),
            Arc::new(StaticFetcher {
                contents: Some("allowed_approval_policies = [\"never\"]".to_string()),
            }),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        assert_eq!(
            service.fetch().await,
            Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::Never]),
                allowed_sandbox_modes: None,
                allowed_web_search_modes: None,
                mcp_servers: None,
                rules: None,
                enforce_residency: None,
                network: None,
            })
        );
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_handles_missing_contents() {
        let result = parse_for_fetch(None);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_handles_empty_contents() {
        let result = parse_for_fetch(Some("   "));
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_handles_invalid_toml() {
        let result = parse_for_fetch(Some("not = ["));
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_ignores_empty_requirements() {
        let result = parse_for_fetch(Some("# comment"));
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_parses_valid_toml() {
        let result = parse_for_fetch(Some("allowed_approval_policies = [\"never\"]"));

        assert_eq!(
            result,
            Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::Never]),
                allowed_sandbox_modes: None,
                allowed_web_search_modes: None,
                mcp_servers: None,
                rules: None,
                enforce_residency: None,
                network: None,
            })
        );
    }

    #[tokio::test(start_paused = true)]
    async fn fetch_cloud_requirements_times_out() {
        let auth_manager = auth_manager_with_plan("enterprise");
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager,
            Arc::new(PendingFetcher),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        let handle = tokio::spawn(async move { service.fetch_with_timeout().await });
        tokio::time::advance(CLOUD_REQUIREMENTS_TIMEOUT + Duration::from_millis(1)).await;

        let result = handle.await.expect("cloud requirements task");
        assert!(result.is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn fetch_cloud_requirements_retries_until_success() {
        let fetcher = Arc::new(SequenceFetcher::new(vec![
            Err(FetchCloudRequirementsStatus::Request),
            Ok(Some("allowed_approval_policies = [\"never\"]".to_string())),
        ]));
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("business"),
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        let handle = tokio::spawn(async move { service.fetch().await });
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(1)).await;

        assert_eq!(
            handle.await.expect("cloud requirements task"),
            Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::Never]),
                allowed_sandbox_modes: None,
                allowed_web_search_modes: None,
                mcp_servers: None,
                rules: None,
                enforce_residency: None,
                network: None,
            })
        );
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_parse_error_does_not_retry() {
        let fetcher = Arc::new(SequenceFetcher::new(vec![
            Ok(Some("not = [".to_string())),
            Ok(Some("allowed_approval_policies = [\"never\"]".to_string())),
        ]));
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("business"),
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert!(service.fetch().await.is_none());
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_uses_cache_when_valid() {
        let codex_home = tempdir().expect("tempdir");
        let prime_service = CloudRequirementsService::new(
            auth_manager_with_plan("business"),
            Arc::new(StaticFetcher {
                contents: Some("allowed_approval_policies = [\"never\"]".to_string()),
            }),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        let _ = prime_service.fetch().await;

        let fetcher = Arc::new(SequenceFetcher::new(vec![Err(
            FetchCloudRequirementsStatus::Request,
        )]));
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("business"),
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(
            service.fetch().await,
            Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::Never]),
                allowed_sandbox_modes: None,
                allowed_web_search_modes: None,
                mcp_servers: None,
                rules: None,
                enforce_residency: None,
                network: None,
            })
        );
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_writes_cache_when_identity_is_incomplete() {
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager_with_plan_and_identity("business", None, Some("account-12345")),
            Arc::new(StaticFetcher {
                contents: Some("allowed_approval_policies = [\"never\"]".to_string()),
            }),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(
            service.fetch().await,
            Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::Never]),
                allowed_sandbox_modes: None,
                allowed_web_search_modes: None,
                mcp_servers: None,
                rules: None,
                enforce_residency: None,
                network: None,
            })
        );

        let path = codex_home.path().join(CLOUD_REQUIREMENTS_CACHE_FILENAME);
        let cache_file: CloudRequirementsCacheFile =
            serde_json::from_str(&std::fs::read_to_string(path).expect("read cache"))
                .expect("parse cache");
        assert_eq!(cache_file.signed_payload.chatgpt_user_id, None);
        assert_eq!(
            cache_file.signed_payload.account_id,
            Some("account-12345".to_string())
        );
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_does_not_use_cache_when_auth_identity_is_incomplete() {
        let codex_home = tempdir().expect("tempdir");
        let prime_service = CloudRequirementsService::new(
            auth_manager_with_plan("business"),
            Arc::new(StaticFetcher {
                contents: Some("allowed_approval_policies = [\"never\"]".to_string()),
            }),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        let _ = prime_service.fetch().await;

        let fetcher = Arc::new(SequenceFetcher::new(vec![Ok(Some(
            "allowed_approval_policies = [\"on-request\"]".to_string(),
        ))]));
        let service = CloudRequirementsService::new(
            auth_manager_with_plan_and_identity("business", None, Some("account-12345")),
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(
            service.fetch().await,
            Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::OnRequest]),
                allowed_sandbox_modes: None,
                allowed_web_search_modes: None,
                mcp_servers: None,
                rules: None,
                enforce_residency: None,
                network: None,
            })
        );
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_ignores_cache_for_different_auth_identity() {
        let codex_home = tempdir().expect("tempdir");
        let prime_service = CloudRequirementsService::new(
            auth_manager_with_plan_and_identity(
                "business",
                Some("user-12345"),
                Some("account-12345"),
            ),
            Arc::new(StaticFetcher {
                contents: Some("allowed_approval_policies = [\"never\"]".to_string()),
            }),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        let _ = prime_service.fetch().await;

        let fetcher = Arc::new(SequenceFetcher::new(vec![Ok(Some(
            "allowed_approval_policies = [\"on-request\"]".to_string(),
        ))]));
        let service = CloudRequirementsService::new(
            auth_manager_with_plan_and_identity(
                "business",
                Some("user-99999"),
                Some("account-12345"),
            ),
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(
            service.fetch().await,
            Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::OnRequest]),
                allowed_sandbox_modes: None,
                allowed_web_search_modes: None,
                mcp_servers: None,
                rules: None,
                enforce_residency: None,
                network: None,
            })
        );
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_ignores_tampered_cache() {
        let codex_home = tempdir().expect("tempdir");
        let prime_service = CloudRequirementsService::new(
            auth_manager_with_plan("business"),
            Arc::new(StaticFetcher {
                contents: Some("allowed_approval_policies = [\"never\"]".to_string()),
            }),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        let _ = prime_service.fetch().await;

        let path = codex_home.path().join(CLOUD_REQUIREMENTS_CACHE_FILENAME);
        let mut cache_file: CloudRequirementsCacheFile =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("read cache"))
                .expect("parse cache");
        cache_file.signed_payload.contents =
            Some("allowed_approval_policies = [\"on-request\"]".to_string());
        std::fs::write(
            &path,
            serde_json::to_vec_pretty(&cache_file).expect("serialize cache"),
        )
        .expect("write cache");

        let fetcher = Arc::new(SequenceFetcher::new(vec![Ok(Some(
            "allowed_approval_policies = [\"never\"]".to_string(),
        ))]));
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("enterprise"),
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(
            service.fetch().await,
            Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::Never]),
                allowed_sandbox_modes: None,
                allowed_web_search_modes: None,
                mcp_servers: None,
                rules: None,
                enforce_residency: None,
                network: None,
            })
        );
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_ignores_expired_cache() {
        let codex_home = tempdir().expect("tempdir");
        let path = codex_home.path().join(CLOUD_REQUIREMENTS_CACHE_FILENAME);
        let cache_file = CloudRequirementsCacheFile {
            signed_payload: CloudRequirementsCacheSignedPayload {
                cached_at: Utc::now(),
                expires_at: Utc::now() - ChronoDuration::seconds(1),
                chatgpt_user_id: Some("user-12345".to_string()),
                account_id: Some("account-12345".to_string()),
                contents: Some("allowed_approval_policies = [\"on-request\"]".to_string()),
            },
            signature: String::new(),
        };
        let payload_bytes = cache_payload_bytes(&cache_file.signed_payload).expect("payload");
        let signature = sign_cache_payload(&payload_bytes).expect("sign payload");
        let cache_file = CloudRequirementsCacheFile {
            signature,
            ..cache_file
        };
        std::fs::write(
            &path,
            serde_json::to_vec_pretty(&cache_file).expect("serialize cache"),
        )
        .expect("write cache");

        let fetcher = Arc::new(SequenceFetcher::new(vec![Ok(Some(
            "allowed_approval_policies = [\"never\"]".to_string(),
        ))]));
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("enterprise"),
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(
            service.fetch().await,
            Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::Never]),
                allowed_sandbox_modes: None,
                allowed_web_search_modes: None,
                mcp_servers: None,
                rules: None,
                enforce_residency: None,
                network: None,
            })
        );
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_writes_signed_cache() {
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("business"),
            Arc::new(StaticFetcher {
                contents: Some("allowed_approval_policies = [\"never\"]".to_string()),
            }),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        let _ = service.fetch().await;

        let path = codex_home.path().join(CLOUD_REQUIREMENTS_CACHE_FILENAME);
        let cache_file: CloudRequirementsCacheFile =
            serde_json::from_str(&std::fs::read_to_string(path).expect("read cache"))
                .expect("parse cache");
        assert!(cache_file.signed_payload.expires_at > Utc::now());
        assert!(cache_file.signed_payload.cached_at <= Utc::now());
        assert_eq!(
            cache_file.signed_payload.chatgpt_user_id,
            Some("user-12345".to_string())
        );
        assert_eq!(
            cache_file.signed_payload.account_id,
            Some("account-12345".to_string())
        );
        assert_eq!(
            cache_file
                .signed_payload
                .contents
                .as_deref()
                .and_then(|contents| parse_cloud_requirements(contents).ok().flatten()),
            Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::Never]),
                allowed_sandbox_modes: None,
                allowed_web_search_modes: None,
                mcp_servers: None,
                rules: None,
                enforce_residency: None,
                network: None,
            })
        );
        let payload_bytes = cache_payload_bytes(&cache_file.signed_payload).expect("payload bytes");
        assert!(verify_cache_signature(
            &payload_bytes,
            &cache_file.signature
        ));
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_none_is_success_without_retry() {
        let fetcher = Arc::new(SequenceFetcher::new(vec![
            Ok(None),
            Err(FetchCloudRequirementsStatus::Request),
        ]));
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("enterprise"),
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert!(service.fetch().await.is_none());
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn fetch_cloud_requirements_stops_after_max_retries() {
        let fetcher = Arc::new(SequenceFetcher::new(vec![
            Err(
                FetchCloudRequirementsStatus::Request
            );
            CLOUD_REQUIREMENTS_MAX_ATTEMPTS
        ]));
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("enterprise"),
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        let handle = tokio::spawn(async move { service.fetch().await });
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(5)).await;
        tokio::task::yield_now().await;

        assert!(handle.await.expect("cloud requirements task").is_none());
        assert_eq!(
            fetcher.request_count.load(Ordering::SeqCst),
            CLOUD_REQUIREMENTS_MAX_ATTEMPTS
        );
    }
}
