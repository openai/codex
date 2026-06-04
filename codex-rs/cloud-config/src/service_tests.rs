use super::*;
use crate::backend::BundleClient;
use crate::backend::BundleRequestError;
use crate::backend::RetryableFailureKind;
use crate::backend::bundle_from_response;
use crate::cache::CLOUD_CONFIG_BUNDLE_CACHE_FILENAME;
use crate::cache::CloudConfigBundleCache;
use crate::metrics::bundle_shape_tag;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use codex_backend_client::ConfigBundleResponse;
use codex_backend_client::DeliveredTomlFragment;
use codex_config::AbsolutePathBuf;
use codex_config::CloudConfigFragment;
use codex_config::CloudConfigTomlBundle;
use codex_config::CloudRequirementsFragment;
use codex_config::CloudRequirementsTomlBundle;
use codex_config::types::AuthCredentialsStoreMode;
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

fn create_test_cache(codex_home: &Path) -> CloudConfigBundleCache {
    CloudConfigBundleCache::new(AbsolutePathBuf::resolve_path_against_base(codex_home, "/"))
}

async fn auth_manager_with_api_key() -> Arc<AuthManager> {
    let tmp = tempdir().expect("tempdir");
    let auth_json = json!({
        "OPENAI_API_KEY": "sk-test-key",
        "tokens": null,
        "last_refresh": null,
    });
    write_auth_json(tmp.path(), auth_json).expect("write auth");
    Arc::new(
        AuthManager::new(
            tmp.path().to_path_buf(),
            /*enable_codex_api_key_env*/ false,
            AuthCredentialsStoreMode::File,
            /*chatgpt_base_url*/ None,
        )
        .await,
    )
}

async fn auth_manager_with_plan_and_identity(
    plan_type: &str,
    chatgpt_user_id: Option<&str>,
    account_id: Option<&str>,
) -> Arc<AuthManager> {
    let tmp = tempdir().expect("tempdir");
    write_auth_json(
        tmp.path(),
        chatgpt_auth_json(
            plan_type,
            chatgpt_user_id,
            account_id,
            "test-access-token",
            "test-refresh-token",
        ),
    )
    .expect("write auth");
    Arc::new(
        AuthManager::new(
            tmp.path().to_path_buf(),
            /*enable_codex_api_key_env*/ false,
            AuthCredentialsStoreMode::File,
            /*chatgpt_base_url*/ None,
        )
        .await,
    )
}

async fn auth_manager_with_plan(plan_type: &str) -> Arc<AuthManager> {
    auth_manager_with_plan_and_identity(plan_type, Some("user-12345"), Some("account-12345")).await
}

fn chatgpt_auth_json(
    plan_type: &str,
    chatgpt_user_id: Option<&str>,
    account_id: Option<&str>,
    access_token: &str,
    refresh_token: &str,
) -> serde_json::Value {
    chatgpt_auth_json_with_last_refresh(
        plan_type,
        chatgpt_user_id,
        account_id,
        access_token,
        refresh_token,
        "2025-01-01T00:00:00Z",
    )
}

fn chatgpt_auth_json_with_last_refresh(
    plan_type: &str,
    chatgpt_user_id: Option<&str>,
    account_id: Option<&str>,
    access_token: &str,
    refresh_token: &str,
    last_refresh: &str,
) -> serde_json::Value {
    chatgpt_auth_json_with_mode(
        plan_type,
        chatgpt_user_id,
        account_id,
        access_token,
        refresh_token,
        last_refresh,
        /*auth_mode*/ None,
    )
}

fn chatgpt_auth_json_with_mode(
    plan_type: &str,
    chatgpt_user_id: Option<&str>,
    account_id: Option<&str>,
    access_token: &str,
    refresh_token: &str,
    last_refresh: &str,
    auth_mode: Option<&str>,
) -> serde_json::Value {
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

    let mut auth_json = json!({
        "OPENAI_API_KEY": null,
        "tokens": {
            "id_token": fake_jwt,
            "access_token": access_token,
            "refresh_token": refresh_token,
            "account_id": account_id,
        },
        "last_refresh": last_refresh,
    });
    if let Some(auth_mode) = auth_mode {
        auth_json["auth_mode"] = serde_json::Value::String(auth_mode.to_string());
    }
    auth_json
}

fn test_bundle() -> CloudConfigBundle {
    CloudConfigBundle {
        config_toml: CloudConfigTomlBundle {
            product_defaults: Vec::new(),
            enterprise_managed: vec![test_config_fragment()],
        },
        requirements_toml: CloudRequirementsTomlBundle {
            enterprise_managed: vec![test_requirements_fragment()],
        },
    }
}

fn test_config_fragment() -> CloudConfigFragment {
    CloudConfigFragment {
        id: "cfg_1".to_string(),
        name: "Base config".to_string(),
        contents: "model = \"gpt-5\"".to_string(),
    }
}

fn test_requirements_fragment() -> CloudRequirementsFragment {
    CloudRequirementsFragment {
        id: "req_1".to_string(),
        name: "Base requirements".to_string(),
        contents: "allowed_approval_policies = [\"never\"]".to_string(),
    }
}

fn invalid_config_bundle() -> CloudConfigBundle {
    CloudConfigBundle {
        config_toml: CloudConfigTomlBundle {
            product_defaults: Vec::new(),
            enterprise_managed: vec![CloudConfigFragment {
                id: "cfg_invalid".to_string(),
                name: "Invalid config".to_string(),
                contents: "model = [".to_string(),
            }],
        },
        requirements_toml: CloudRequirementsTomlBundle::default(),
    }
}

fn request_error() -> BundleRequestError {
    BundleRequestError::Retryable(RetryableFailureKind::Request { status_code: None })
}

struct StaticBundleClient {
    bundle: CloudConfigBundle,
    request_count: AtomicUsize,
}

impl StaticBundleClient {
    fn new(bundle: CloudConfigBundle) -> Self {
        Self {
            bundle,
            request_count: AtomicUsize::new(0),
        }
    }
}

impl BundleClient for StaticBundleClient {
    async fn get_bundle(&self, _auth: &CodexAuth) -> Result<CloudConfigBundle, BundleRequestError> {
        self.request_count.fetch_add(1, Ordering::SeqCst);
        Ok(self.bundle.clone())
    }
}

struct PendingBundleClient;

impl BundleClient for PendingBundleClient {
    async fn get_bundle(&self, _auth: &CodexAuth) -> Result<CloudConfigBundle, BundleRequestError> {
        pending::<()>().await;
        Ok(CloudConfigBundle::default())
    }
}

struct SequenceBundleClient {
    responses: tokio::sync::Mutex<VecDeque<Result<CloudConfigBundle, BundleRequestError>>>,
    request_count: AtomicUsize,
}

impl SequenceBundleClient {
    fn new(responses: Vec<Result<CloudConfigBundle, BundleRequestError>>) -> Self {
        Self {
            responses: tokio::sync::Mutex::new(VecDeque::from(responses)),
            request_count: AtomicUsize::new(0),
        }
    }
}

impl BundleClient for SequenceBundleClient {
    async fn get_bundle(&self, _auth: &CodexAuth) -> Result<CloudConfigBundle, BundleRequestError> {
        self.request_count.fetch_add(1, Ordering::SeqCst);
        let mut responses = self.responses.lock().await;
        responses
            .pop_front()
            .unwrap_or_else(|| Ok(CloudConfigBundle::default()))
    }
}

struct TokenBundleClient {
    expected_token: String,
    bundle: CloudConfigBundle,
    request_count: AtomicUsize,
}

impl BundleClient for TokenBundleClient {
    async fn get_bundle(&self, auth: &CodexAuth) -> Result<CloudConfigBundle, BundleRequestError> {
        self.request_count.fetch_add(1, Ordering::SeqCst);
        if matches!(
            auth.get_token().as_deref(),
            Ok(token) if token == self.expected_token.as_str()
        ) {
            Ok(self.bundle.clone())
        } else {
            Err(BundleRequestError::Unauthorized {
                status_code: Some(401),
                message: "GET /config/bundle failed: 401".to_string(),
            })
        }
    }
}

struct UnauthorizedBundleClient {
    message: String,
    request_count: AtomicUsize,
}

impl BundleClient for UnauthorizedBundleClient {
    async fn get_bundle(&self, _auth: &CodexAuth) -> Result<CloudConfigBundle, BundleRequestError> {
        self.request_count.fetch_add(1, Ordering::SeqCst);
        Err(BundleRequestError::Unauthorized {
            status_code: Some(401),
            message: self.message.clone(),
        })
    }
}

#[test]
fn bundle_shape_tag_describes_sorted_enterprise_sources() {
    assert_eq!(bundle_shape_tag(/*bundle*/ None), "none");
    assert_eq!(
        bundle_shape_tag(Some(&CloudConfigBundle::default())),
        "empty"
    );
    assert_eq!(
        bundle_shape_tag(Some(&CloudConfigBundle {
            config_toml: CloudConfigTomlBundle {
                product_defaults: Vec::new(),
                enterprise_managed: vec![test_config_fragment()],
            },
            requirements_toml: CloudRequirementsTomlBundle::default(),
        })),
        "enterprise_config"
    );
    assert_eq!(
        bundle_shape_tag(Some(&CloudConfigBundle {
            config_toml: CloudConfigTomlBundle::default(),
            requirements_toml: CloudRequirementsTomlBundle {
                enterprise_managed: vec![test_requirements_fragment()],
            },
        })),
        "enterprise_requirements"
    );
    assert_eq!(
        bundle_shape_tag(Some(&CloudConfigBundle {
            config_toml: CloudConfigTomlBundle {
                product_defaults: Vec::new(),
                enterprise_managed: vec![test_config_fragment()],
            },
            requirements_toml: CloudRequirementsTomlBundle {
                enterprise_managed: vec![test_requirements_fragment()],
            },
        })),
        "enterprise_config,enterprise_requirements"
    );
}

#[tokio::test]
async fn get_bundle_skips_non_chatgpt_auth() {
    let fetcher = Arc::new(StaticBundleClient::new(test_bundle()));
    let codex_home = tempdir().expect("tempdir");
    let service = CloudConfigBundleService::new(
        auth_manager_with_api_key().await,
        fetcher.clone(),
        codex_home.path().to_path_buf(),
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
    );

    assert_eq!(service.load_startup_bundle().await, Ok(None));
    assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn get_bundle_skips_individual_plan() {
    let fetcher = Arc::new(StaticBundleClient::new(test_bundle()));
    let codex_home = tempdir().expect("tempdir");
    let service = CloudConfigBundleService::new(
        auth_manager_with_plan("pro").await,
        fetcher.clone(),
        codex_home.path().to_path_buf(),
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
    );

    assert_eq!(service.load_startup_bundle().await, Ok(None));
    assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn get_bundle_allows_eligible_workspace_plans_and_writes_cache() {
    for plan_type in [
        "business",
        "enterprise_cbp_usage_based",
        "enterprise",
        "hc",
        "edu",
        "education",
    ] {
        let bundle = test_bundle();
        let fetcher = Arc::new(StaticBundleClient::new(bundle.clone()));
        let codex_home = tempdir().expect("tempdir");
        let service = CloudConfigBundleService::new(
            auth_manager_with_plan(plan_type).await,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_CONFIG_BUNDLE_TIMEOUT,
        );

        assert_eq!(
            service.load_startup_bundle().await,
            Ok(Some(bundle)),
            "plan_type: {plan_type}"
        );
        assert_eq!(
            fetcher.request_count.load(Ordering::SeqCst),
            1,
            "plan_type: {plan_type}"
        );
        assert!(
            codex_home
                .path()
                .join(CLOUD_CONFIG_BUNDLE_CACHE_FILENAME)
                .exists(),
            "plan_type: {plan_type}"
        );
    }
}

#[tokio::test]
async fn get_bundle_skips_team_like_usage_based_plan() {
    let fetcher = Arc::new(StaticBundleClient::new(test_bundle()));
    let codex_home = tempdir().expect("tempdir");
    let service = CloudConfigBundleService::new(
        auth_manager_with_plan("self_serve_business_usage_based").await,
        fetcher.clone(),
        codex_home.path().to_path_buf(),
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
    );

    assert_eq!(service.load_startup_bundle().await, Ok(None));
    assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn get_bundle_rejects_invalid_remote_bundle_before_cache_write() {
    let codex_home = tempdir().expect("tempdir");
    let fetcher = Arc::new(StaticBundleClient::new(invalid_config_bundle()));
    let service = CloudConfigBundleService::new(
        auth_manager_with_plan("business").await,
        fetcher.clone(),
        codex_home.path().to_path_buf(),
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
    );

    let err = service
        .load_startup_bundle()
        .await
        .expect_err("invalid remote bundle should fail closed");

    assert_eq!(err.code(), CloudConfigBundleLoadErrorCode::InvalidBundle);
    assert!(err.to_string().contains("invalid cloud config bundle"));
    assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
    assert!(
        !codex_home
            .path()
            .join(CLOUD_CONFIG_BUNDLE_CACHE_FILENAME)
            .exists()
    );
}

#[tokio::test]
async fn get_bundle_ignores_invalid_cache_and_refetches() {
    let codex_home = tempdir().expect("tempdir");
    let cache = create_test_cache(codex_home.path());
    cache
        .save(
            Some("user-12345".to_string()),
            Some("account-12345".to_string()),
            invalid_config_bundle(),
        )
        .await
        .expect("write invalid cache");
    let replacement_bundle = test_bundle();
    let fetcher = Arc::new(StaticBundleClient::new(replacement_bundle.clone()));
    let service = CloudConfigBundleService::new(
        auth_manager_with_plan("business").await,
        fetcher.clone(),
        codex_home.path().to_path_buf(),
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
    );

    assert_eq!(
        service.load_startup_bundle().await,
        Ok(Some(replacement_bundle.clone()))
    );
    assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
    assert_eq!(
        cache
            .load(Some("user-12345"), Some("account-12345"))
            .await
            .expect("load refreshed cache")
            .bundle,
        replacement_bundle
    );
}

#[tokio::test]
async fn get_bundle_empty_response_is_success_and_cached() {
    let codex_home = tempdir().expect("tempdir");
    let fetcher = Arc::new(StaticBundleClient::new(CloudConfigBundle::default()));
    let service = CloudConfigBundleService::new(
        auth_manager_with_plan("enterprise").await,
        fetcher.clone(),
        codex_home.path().to_path_buf(),
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
    );

    assert_eq!(service.load_startup_bundle().await, Ok(None));
    assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
    assert!(
        codex_home
            .path()
            .join(CLOUD_CONFIG_BUNDLE_CACHE_FILENAME)
            .exists()
    );
}

#[tokio::test]
async fn get_bundle_uses_cache_when_valid() {
    let bundle = test_bundle();
    let codex_home = tempdir().expect("tempdir");
    let prime_service = CloudConfigBundleService::new(
        auth_manager_with_plan("business").await,
        Arc::new(StaticBundleClient::new(bundle.clone())),
        codex_home.path().to_path_buf(),
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
    );
    let _ = prime_service.load_startup_bundle().await;

    let fetcher = Arc::new(SequenceBundleClient::new(vec![Err(request_error())]));
    let service = CloudConfigBundleService::new(
        auth_manager_with_plan("business").await,
        fetcher.clone(),
        codex_home.path().to_path_buf(),
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
    );

    assert_eq!(service.load_startup_bundle().await, Ok(Some(bundle)));
    assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn get_bundle_ignores_cache_for_different_auth_identity() {
    let codex_home = tempdir().expect("tempdir");
    let prime_service = CloudConfigBundleService::new(
        auth_manager_with_plan_and_identity("business", Some("user-12345"), Some("account-12345"))
            .await,
        Arc::new(StaticBundleClient::new(test_bundle())),
        codex_home.path().to_path_buf(),
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
    );
    let _ = prime_service.load_startup_bundle().await;

    let replacement_bundle = CloudConfigBundle {
        config_toml: CloudConfigTomlBundle::default(),
        requirements_toml: CloudRequirementsTomlBundle {
            enterprise_managed: vec![CloudRequirementsFragment {
                id: "req_2".to_string(),
                name: "Replacement requirements".to_string(),
                contents: "allowed_approval_policies = [\"on-request\"]".to_string(),
            }],
        },
    };
    let fetcher = Arc::new(SequenceBundleClient::new(vec![Ok(
        replacement_bundle.clone()
    )]));
    let service = CloudConfigBundleService::new(
        auth_manager_with_plan_and_identity("business", Some("user-99999"), Some("account-12345"))
            .await,
        fetcher.clone(),
        codex_home.path().to_path_buf(),
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
    );

    assert_eq!(
        service.load_startup_bundle().await,
        Ok(Some(replacement_bundle))
    );
    assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn get_bundle_times_out() {
    let codex_home = tempdir().expect("tempdir");
    let service = CloudConfigBundleService::new(
        auth_manager_with_plan("enterprise").await,
        Arc::new(PendingBundleClient),
        codex_home.path().to_path_buf(),
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
    );
    let handle = tokio::spawn(async move { service.load_startup_bundle_with_timeout().await });
    tokio::time::advance(CLOUD_CONFIG_BUNDLE_TIMEOUT + Duration::from_millis(1)).await;

    let result = handle.await.expect("cloud config bundle task");
    let err = result.expect_err("cloud config bundle timeout should fail closed");
    assert!(
        err.to_string()
            .contains("timed out waiting for cloud config bundle")
    );
}

#[tokio::test(start_paused = true)]
async fn get_bundle_retries_until_success() {
    let fetcher = Arc::new(SequenceBundleClient::new(vec![
        Err(request_error()),
        Ok(test_bundle()),
    ]));
    let codex_home = tempdir().expect("tempdir");
    let service = CloudConfigBundleService::new(
        auth_manager_with_plan("business").await,
        fetcher.clone(),
        codex_home.path().to_path_buf(),
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
    );

    let handle = tokio::spawn(async move { service.load_startup_bundle().await });
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(1)).await;

    assert_eq!(handle.await.expect("bundle task"), Ok(Some(test_bundle())));
    assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn get_bundle_recovers_after_unauthorized_reload() {
    let auth_home = tempdir().expect("tempdir");
    write_auth_json(
        auth_home.path(),
        chatgpt_auth_json_with_last_refresh(
            "business",
            Some("user-12345"),
            Some("account-12345"),
            "stale-access-token",
            "test-refresh-token",
            // Keep auth "fresh" so the first request hits unauthorized recovery
            // instead of AuthManager::auth() proactively reloading from disk.
            "3025-01-01T00:00:00Z",
        ),
    )
    .expect("write initial auth");
    let auth_manager = Arc::new(
        AuthManager::new(
            auth_home.path().to_path_buf(),
            /*enable_codex_api_key_env*/ false,
            AuthCredentialsStoreMode::File,
            /*chatgpt_base_url*/ None,
        )
        .await,
    );

    write_auth_json(
        auth_home.path(),
        chatgpt_auth_json_with_last_refresh(
            "business",
            Some("user-12345"),
            Some("account-12345"),
            "fresh-access-token",
            "test-refresh-token",
            "3025-01-01T00:00:00Z",
        ),
    )
    .expect("write refreshed auth");
    let fetcher = Arc::new(TokenBundleClient {
        expected_token: "fresh-access-token".to_string(),
        bundle: test_bundle(),
        request_count: AtomicUsize::new(0),
    });
    let codex_home = tempdir().expect("tempdir");
    let service = CloudConfigBundleService::new(
        auth_manager,
        fetcher.clone(),
        codex_home.path().to_path_buf(),
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
    );

    assert_eq!(service.load_startup_bundle().await, Ok(Some(test_bundle())));
    assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn get_bundle_recovers_after_unauthorized_reload_updates_cache_identity() {
    let auth_home = tempdir().expect("tempdir");
    write_auth_json(
        auth_home.path(),
        chatgpt_auth_json_with_last_refresh(
            "business",
            Some("user-12345"),
            Some("account-12345"),
            "stale-access-token",
            "test-refresh-token",
            "3025-01-01T00:00:00Z",
        ),
    )
    .expect("write initial auth");
    let auth_manager = Arc::new(
        AuthManager::new(
            auth_home.path().to_path_buf(),
            /*enable_codex_api_key_env*/ false,
            AuthCredentialsStoreMode::File,
            /*chatgpt_base_url*/ None,
        )
        .await,
    );

    write_auth_json(
        auth_home.path(),
        chatgpt_auth_json_with_last_refresh(
            "business",
            Some("user-99999"),
            Some("account-12345"),
            "fresh-access-token",
            "test-refresh-token",
            "3025-01-01T00:00:00Z",
        ),
    )
    .expect("write refreshed auth");
    let fetcher = Arc::new(TokenBundleClient {
        expected_token: "fresh-access-token".to_string(),
        bundle: test_bundle(),
        request_count: AtomicUsize::new(0),
    });
    let codex_home = tempdir().expect("tempdir");
    let service = CloudConfigBundleService::new(
        auth_manager,
        fetcher.clone(),
        codex_home.path().to_path_buf(),
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
    );

    assert_eq!(service.load_startup_bundle().await, Ok(Some(test_bundle())));
    let cache = create_test_cache(codex_home.path());
    assert_eq!(
        cache
            .load(Some("user-99999"), Some("account-12345"))
            .await
            .expect("load cache")
            .bundle,
        test_bundle()
    );
    assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn get_bundle_surfaces_auth_recovery_message() {
    let auth_home = tempdir().expect("tempdir");
    write_auth_json(
        auth_home.path(),
        chatgpt_auth_json(
            "enterprise",
            Some("user-12345"),
            Some("account-12345"),
            "stale-access-token",
            "test-refresh-token",
        ),
    )
    .expect("write auth");
    let auth_manager = Arc::new(
        AuthManager::new(
            auth_home.path().to_path_buf(),
            /*enable_codex_api_key_env*/ false,
            AuthCredentialsStoreMode::File,
            /*chatgpt_base_url*/ None,
        )
        .await,
    );

    write_auth_json(
        auth_home.path(),
        chatgpt_auth_json(
            "enterprise",
            Some("user-12345"),
            Some("account-99999"),
            "fresh-access-token",
            "test-refresh-token",
        ),
    )
    .expect("write mismatched auth");
    let fetcher = Arc::new(UnauthorizedBundleClient {
        message: "GET /config/bundle failed: 401".to_string(),
        request_count: AtomicUsize::new(0),
    });
    let codex_home = tempdir().expect("tempdir");
    let service = CloudConfigBundleService::new(
        auth_manager,
        fetcher.clone(),
        codex_home.path().to_path_buf(),
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
    );

    let err = service
        .load_startup_bundle()
        .await
        .expect_err("cloud config bundle should surface auth recovery errors");
    assert_eq!(
        err,
        CloudConfigBundleLoadError::new(
            CloudConfigBundleLoadErrorCode::Auth,
            Some(401),
            "Your access token could not be refreshed because you have since logged out or signed in to another account. Please sign in again.",
        )
    );
    assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn get_bundle_unauthorized_without_recovery_uses_generic_message() {
    let auth_home = tempdir().expect("tempdir");
    write_auth_json(
        auth_home.path(),
        chatgpt_auth_json_with_mode(
            "enterprise",
            Some("user-12345"),
            Some("account-12345"),
            "test-access-token",
            "test-refresh-token",
            "2025-01-01T00:00:00Z",
            Some("chatgptAuthTokens"),
        ),
    )
    .expect("write auth");
    let auth_manager = Arc::new(
        AuthManager::new(
            auth_home.path().to_path_buf(),
            /*enable_codex_api_key_env*/ false,
            AuthCredentialsStoreMode::File,
            /*chatgpt_base_url*/ None,
        )
        .await,
    );

    let fetcher = Arc::new(UnauthorizedBundleClient {
        message:
            "GET https://chatgpt.com/backend-api/wham/config/bundle failed: 401; content-type=text/html; body=<html>nope</html>"
                .to_string(),
        request_count: AtomicUsize::new(0),
    });
    let codex_home = tempdir().expect("tempdir");
    let service = CloudConfigBundleService::new(
        auth_manager,
        fetcher.clone(),
        codex_home.path().to_path_buf(),
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
    );

    let err = service
        .load_startup_bundle()
        .await
        .expect_err("cloud config bundle should fail closed");
    assert_eq!(
        err,
        CloudConfigBundleLoadError::new(
            CloudConfigBundleLoadErrorCode::Auth,
            Some(401),
            CLOUD_CONFIG_BUNDLE_AUTH_RECOVERY_FAILED_MESSAGE,
        )
    );
    assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn get_bundle_does_not_use_cache_when_auth_identity_is_incomplete() {
    let codex_home = tempdir().expect("tempdir");
    let prime_service = CloudConfigBundleService::new(
        auth_manager_with_plan("business").await,
        Arc::new(StaticBundleClient::new(test_bundle())),
        codex_home.path().to_path_buf(),
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
    );
    let _ = prime_service.load_startup_bundle().await;

    let replacement_bundle = CloudConfigBundle {
        config_toml: CloudConfigTomlBundle::default(),
        requirements_toml: CloudRequirementsTomlBundle {
            enterprise_managed: vec![CloudRequirementsFragment {
                id: "req_2".to_string(),
                name: "Replacement requirements".to_string(),
                contents: "allowed_approval_policies = [\"on-request\"]".to_string(),
            }],
        },
    };
    let fetcher = Arc::new(SequenceBundleClient::new(vec![Ok(
        replacement_bundle.clone()
    )]));
    let service = CloudConfigBundleService::new(
        auth_manager_with_plan_and_identity(
            "business",
            /*chatgpt_user_id*/ None,
            Some("account-12345"),
        )
        .await,
        fetcher.clone(),
        codex_home.path().to_path_buf(),
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
    );

    assert_eq!(
        service.load_startup_bundle().await,
        Ok(Some(replacement_bundle))
    );
    assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn get_bundle_stops_after_max_retries() {
    let fetcher = Arc::new(SequenceBundleClient::new(vec![
        Err(request_error());
        CLOUD_CONFIG_BUNDLE_MAX_ATTEMPTS
    ]));
    let codex_home = tempdir().expect("tempdir");
    let service = CloudConfigBundleService::new(
        auth_manager_with_plan("enterprise").await,
        fetcher.clone(),
        codex_home.path().to_path_buf(),
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
    );

    let handle = tokio::spawn(async move { service.load_startup_bundle().await });
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(5)).await;
    tokio::task::yield_now().await;

    let err = handle
        .await
        .expect("cloud config bundle task")
        .expect_err("cloud config bundle retry exhaustion should fail closed");
    assert_eq!(err.to_string(), CLOUD_CONFIG_BUNDLE_LOAD_FAILED_MESSAGE);
    assert_eq!(err.code(), CloudConfigBundleLoadErrorCode::RequestFailed);
    assert_eq!(
        fetcher.request_count.load(Ordering::SeqCst),
        CLOUD_CONFIG_BUNDLE_MAX_ATTEMPTS
    );
}

#[tokio::test]
async fn refresh_from_remote_updates_cached_bundle() {
    let replacement_bundle = CloudConfigBundle {
        config_toml: CloudConfigTomlBundle::default(),
        requirements_toml: CloudRequirementsTomlBundle {
            enterprise_managed: vec![CloudRequirementsFragment {
                id: "req_2".to_string(),
                name: "Replacement requirements".to_string(),
                contents: "allowed_approval_policies = [\"on-request\"]".to_string(),
            }],
        },
    };
    let codex_home = tempdir().expect("tempdir");
    let fetcher = Arc::new(SequenceBundleClient::new(vec![
        Ok(test_bundle()),
        Ok(replacement_bundle.clone()),
    ]));
    let service = CloudConfigBundleService::new(
        auth_manager_with_plan("business").await,
        fetcher,
        codex_home.path().to_path_buf(),
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
    );

    assert_eq!(service.load_startup_bundle().await, Ok(Some(test_bundle())));
    assert!(service.refresh_cache_once().await);

    let cache = create_test_cache(codex_home.path());
    let signed_payload = cache
        .load(Some("user-12345"), Some("account-12345"))
        .await
        .expect("load cache");
    assert_eq!(signed_payload.bundle, replacement_bundle);
}

#[test]
fn bundle_response_conversion_preserves_fragment_order() {
    let response = ConfigBundleResponse {
        config_toml: Some(Some(Box::new(codex_backend_client::DeliveredConfigToml {
            product_defaults: Some(Some(vec![DeliveredTomlFragment::new(
                "cfg_default".to_string(),
                "Product defaults".to_string(),
                "model = \"default\"".to_string(),
            )])),
            enterprise_managed: Some(Some(vec![
                DeliveredTomlFragment::new(
                    "cfg_high".to_string(),
                    "High config".to_string(),
                    "model = \"high\"".to_string(),
                ),
                DeliveredTomlFragment::new(
                    "cfg_low".to_string(),
                    "Low config".to_string(),
                    "model = \"low\"".to_string(),
                ),
            ])),
        }))),
        requirements_toml: Some(Some(Box::new(
            codex_backend_client::DeliveredRequirementsToml {
                enterprise_managed: Some(Some(vec![DeliveredTomlFragment::new(
                    "req_high".to_string(),
                    "High requirements".to_string(),
                    "allowed_approval_policies = [\"never\"]".to_string(),
                )])),
            },
        ))),
    };

    assert_eq!(
        bundle_from_response(response),
        CloudConfigBundle {
            config_toml: CloudConfigTomlBundle {
                product_defaults: vec![CloudConfigFragment {
                    id: "cfg_default".to_string(),
                    name: "Product defaults".to_string(),
                    contents: "model = \"default\"".to_string(),
                }],
                enterprise_managed: vec![
                    CloudConfigFragment {
                        id: "cfg_high".to_string(),
                        name: "High config".to_string(),
                        contents: "model = \"high\"".to_string(),
                    },
                    CloudConfigFragment {
                        id: "cfg_low".to_string(),
                        name: "Low config".to_string(),
                        contents: "model = \"low\"".to_string(),
                    },
                ],
            },
            requirements_toml: CloudRequirementsTomlBundle {
                enterprise_managed: vec![CloudRequirementsFragment {
                    id: "req_high".to_string(),
                    name: "High requirements".to_string(),
                    contents: "allowed_approval_policies = [\"never\"]".to_string(),
                }],
            },
        }
    );
}

#[test]
fn bundle_response_conversion_treats_missing_sections_as_empty() {
    assert_eq!(
        bundle_from_response(ConfigBundleResponse::new()),
        CloudConfigBundle::default()
    );
}
