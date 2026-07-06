use super::*;
use crate::backend::BundleClient;
use crate::backend::BundleRequestError;
use crate::backend::RetryableFailureKind;
use crate::backend::bundle_from_response;
use crate::cache::CLOUD_CONFIG_BUNDLE_CACHE_FILENAME;
use crate::cache::CloudConfigBundleCache;
use crate::cache::sign_cache_payload;
use crate::cache::verify_cache_signature;
use crate::metrics::bundle_shape_tag;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use codex_backend_client::ConfigBundleResponse;
use codex_backend_client::DeliveredManagedLayers;
use codex_backend_client::DeliveredTomlFragment;
use codex_config::AbsolutePathBuf;
use codex_config::CloudConfigFragment;
use codex_config::CloudConfigTomlBundle;
use codex_config::CloudConfigTomlManagedLayers;
use codex_config::CloudRequirementsFragment;
use codex_config::CloudRequirementsTomlBundle;
use codex_config::CloudRequirementsTomlManagedLayers;
use codex_config::types::AuthCredentialsStoreMode;
use codex_login::AuthKeyringBackendKind;
use codex_login::auth::AgentIdentityAuth;
use codex_login::auth::AgentIdentityAuthRecord;
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

fn write_legacy_v1_cache(cache: &CloudConfigBundleCache) {
    #[derive(serde::Serialize)]
    struct LegacyTomlBundle<T> {
        enterprise_managed: Vec<T>,
    }
    #[derive(serde::Serialize)]
    struct LegacyBundle {
        config_toml: LegacyTomlBundle<CloudConfigFragment>,
        requirements_toml: LegacyTomlBundle<CloudRequirementsFragment>,
    }
    #[derive(serde::Serialize)]
    struct LegacySignedPayload {
        version: u32,
        cached_at: chrono::DateTime<chrono::Utc>,
        expires_at: chrono::DateTime<chrono::Utc>,
        chatgpt_user_id: Option<String>,
        account_id: Option<String>,
        bundle: LegacyBundle,
    }

    let cached_at = chrono::Utc::now();
    let signed_payload = LegacySignedPayload {
        version: 1,
        cached_at,
        expires_at: cached_at + chrono::Duration::minutes(30),
        chatgpt_user_id: Some("user-12345".to_string()),
        account_id: Some("account-12345".to_string()),
        bundle: LegacyBundle {
            config_toml: LegacyTomlBundle {
                enterprise_managed: vec![config_fragment("cfg_legacy", "model = \"legacy\"")],
            },
            requirements_toml: LegacyTomlBundle {
                enterprise_managed: vec![requirements_fragment(
                    "req_legacy",
                    "allowed_approval_policies = [\"never\"]",
                )],
            },
        },
    };
    let payload_bytes = serde_json::to_vec(&signed_payload).expect("serialize legacy payload");
    let signature = sign_cache_payload(&payload_bytes).expect("sign legacy payload");
    assert!(verify_cache_signature(&payload_bytes, &signature));
    std::fs::write(
        cache.path(),
        serde_json::to_vec_pretty(&json!({
            "signed_payload": serde_json::to_value(signed_payload)
                .expect("serialize legacy signed payload"),
            "signature": signature,
        }))
        .expect("serialize legacy cache"),
    )
    .expect("write legacy cache");
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
            /*forced_chatgpt_workspace_id*/ None,
            /*chatgpt_base_url*/ None,
            AuthKeyringBackendKind::default(),
            /*auth_route_config*/ None,
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
            /*forced_chatgpt_workspace_id*/ None,
            /*chatgpt_base_url*/ None,
            AuthKeyringBackendKind::default(),
            /*auth_route_config*/ None,
        )
        .await,
    )
}

async fn auth_manager_with_plan(plan_type: &str) -> Arc<AuthManager> {
    auth_manager_with_plan_and_identity(plan_type, Some("user-12345"), Some("account-12345")).await
}

async fn auth_manager_with_agent_identity_business_plan() -> Arc<AuthManager> {
    let key_material =
        codex_agent_identity::generate_agent_key_material().expect("generate agent key material");
    AuthManager::from_auth_for_testing(CodexAuth::AgentIdentity(
        AgentIdentityAuth::from_record(
            AgentIdentityAuthRecord {
                agent_runtime_id: "agent-runtime-123".to_string(),
                agent_private_key: key_material.private_key_pkcs8_base64,
                account_id: "account-12345".to_string(),
                chatgpt_user_id: "user-12345".to_string(),
                email: Some("user@example.com".to_string()),
                plan_type: PlanType::Business,
                chatgpt_account_is_fedramp: false,
                task_id: Some("task-123".to_string()),
            },
            "https://auth.openai.com/api/accounts",
            /*auth_route_config*/ None,
        )
        .await
        .expect("agent identity record should be complete"),
    ))
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
            managed_layers: CloudConfigTomlManagedLayers {
                baseline: vec![test_config_fragment()],
                system_overlay: Vec::new(),
            },
        },
        requirements_toml: CloudRequirementsTomlBundle {
            managed_layers: CloudRequirementsTomlManagedLayers {
                baseline: Vec::new(),
                system_overlay: vec![test_requirements_fragment()],
            },
        },
    }
}

fn test_config_fragment() -> CloudConfigFragment {
    config_fragment("cfg_1", "model = \"gpt-5\"")
}

fn test_requirements_fragment() -> CloudRequirementsFragment {
    requirements_fragment("req_1", "allowed_approval_policies = [\"never\"]")
}

fn delivered_fragment(id: &str, contents: &str) -> DeliveredTomlFragment {
    DeliveredTomlFragment::new(id.to_string(), id.to_string(), contents.to_string())
}

fn config_fragment(id: &str, contents: &str) -> CloudConfigFragment {
    CloudConfigFragment {
        id: id.to_string(),
        name: id.to_string(),
        contents: contents.to_string(),
    }
}

fn requirements_fragment(id: &str, contents: &str) -> CloudRequirementsFragment {
    CloudRequirementsFragment {
        id: id.to_string(),
        name: id.to_string(),
        contents: contents.to_string(),
    }
}

fn delivered_managed_layers(
    baseline: Vec<DeliveredTomlFragment>,
    system_overlay: Vec<DeliveredTomlFragment>,
) -> Option<Option<Box<DeliveredManagedLayers>>> {
    Some(Some(Box::new(DeliveredManagedLayers::new(
        baseline,
        system_overlay,
    ))))
}

fn replacement_requirements_bundle() -> CloudConfigBundle {
    CloudConfigBundle {
        config_toml: CloudConfigTomlBundle::default(),
        requirements_toml: CloudRequirementsTomlBundle {
            managed_layers: CloudRequirementsTomlManagedLayers {
                baseline: Vec::new(),
                system_overlay: vec![CloudRequirementsFragment {
                    id: "req_2".to_string(),
                    name: "Replacement requirements".to_string(),
                    contents: "allowed_approval_policies = [\"on-request\"]".to_string(),
                }],
            },
        },
    }
}

fn invalid_config_bundle() -> CloudConfigBundle {
    CloudConfigBundle {
        config_toml: CloudConfigTomlBundle {
            managed_layers: CloudConfigTomlManagedLayers {
                baseline: vec![CloudConfigFragment {
                    id: "cfg_invalid".to_string(),
                    name: "Invalid config".to_string(),
                    contents: "model = [".to_string(),
                }],
                system_overlay: Vec::new(),
            },
        },
        requirements_toml: CloudRequirementsTomlBundle::default(),
    }
}

fn conflicting_managed_requirements_bundle() -> CloudConfigBundle {
    CloudConfigBundle {
        config_toml: CloudConfigTomlBundle::default(),
        requirements_toml: CloudRequirementsTomlBundle {
            managed_layers: CloudRequirementsTomlManagedLayers {
                baseline: vec![CloudRequirementsFragment {
                    id: "req_baseline".to_string(),
                    name: "Baseline requirements".to_string(),
                    contents: r#"
[hooks]
managed_dir = "/managed/baseline"
windows_managed_dir = 'C:\managed\baseline'
"#
                    .to_string(),
                }],
                system_overlay: vec![CloudRequirementsFragment {
                    id: "req_overlay".to_string(),
                    name: "System overlay requirements".to_string(),
                    contents: r#"
[hooks]
managed_dir = "/managed/overlay"
windows_managed_dir = 'C:\managed\overlay'
"#
                    .to_string(),
                }],
            },
        },
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
fn bundle_shape_tag_describes_managed_documents() {
    assert_eq!(bundle_shape_tag(/*bundle*/ None), "none");
    assert_eq!(
        bundle_shape_tag(Some(&CloudConfigBundle::default())),
        "empty"
    );
    assert_eq!(
        bundle_shape_tag(Some(&CloudConfigBundle {
            config_toml: CloudConfigTomlBundle {
                managed_layers: CloudConfigTomlManagedLayers {
                    baseline: vec![test_config_fragment()],
                    system_overlay: vec![test_config_fragment()],
                },
            },
            requirements_toml: CloudRequirementsTomlBundle {
                managed_layers: CloudRequirementsTomlManagedLayers {
                    baseline: vec![test_requirements_fragment()],
                    system_overlay: vec![test_requirements_fragment()],
                },
            },
        })),
        "cloud_managed_config,cloud_managed_requirements"
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
async fn get_bundle_allows_agent_identity_business_plan() {
    let bundle = test_bundle();
    let fetcher = Arc::new(StaticBundleClient::new(bundle.clone()));
    let codex_home = tempdir().expect("tempdir");
    let service = CloudConfigBundleService::new(
        auth_manager_with_agent_identity_business_plan().await,
        fetcher.clone(),
        codex_home.path().to_path_buf(),
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
    );

    assert_eq!(service.load_startup_bundle().await, Ok(Some(bundle)));
    assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
    assert!(
        codex_home
            .path()
            .join(CLOUD_CONFIG_BUNDLE_CACHE_FILENAME)
            .exists()
    );
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
    for bundle in [
        invalid_config_bundle(),
        conflicting_managed_requirements_bundle(),
    ] {
        let codex_home = tempdir().expect("tempdir");
        let fetcher = Arc::new(StaticBundleClient::new(bundle));
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
}

#[tokio::test]
async fn get_bundle_does_not_retry_or_cache_invalid_transport_bundle() {
    let codex_home = tempdir().expect("tempdir");
    let fetcher = Arc::new(SequenceBundleClient::new(vec![
        Err(BundleRequestError::InvalidBundle {
            message:
                "cloud config bundle config_toml is present but managed_layers is missing or null"
                    .to_string(),
        }),
        Ok(test_bundle()),
    ]));
    let service = CloudConfigBundleService::new(
        auth_manager_with_plan("business").await,
        fetcher.clone(),
        codex_home.path().to_path_buf(),
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
    );

    let err = service
        .load_startup_bundle()
        .await
        .expect_err("invalid transport bundle should fail closed");

    assert_eq!(err.code(), CloudConfigBundleLoadErrorCode::InvalidBundle);
    assert!(
        err.to_string()
            .contains("managed_layers is missing or null")
    );
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
async fn get_bundle_treats_legacy_v1_cache_as_miss_and_rewrites_v2() {
    let codex_home = tempdir().expect("tempdir");
    let cache = create_test_cache(codex_home.path());
    write_legacy_v1_cache(&cache);
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
            .expect("load rewritten cache")
            .bundle,
        replacement_bundle
    );
    let cache_json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(cache.path()).expect("read rewritten cache"))
            .expect("parse rewritten cache");
    assert_eq!(cache_json["signed_payload"]["version"], 2);
    let bundle_json = &cache_json["signed_payload"]["bundle"];
    assert!(bundle_json["config_toml"]["managed_layers"].is_object());
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

    let replacement_bundle = replacement_requirements_bundle();
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
            /*forced_chatgpt_workspace_id*/ None,
            /*chatgpt_base_url*/ None,
            AuthKeyringBackendKind::default(),
            /*auth_route_config*/ None,
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
            /*forced_chatgpt_workspace_id*/ None,
            /*chatgpt_base_url*/ None,
            AuthKeyringBackendKind::default(),
            /*auth_route_config*/ None,
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
            /*forced_chatgpt_workspace_id*/ None,
            /*chatgpt_base_url*/ None,
            AuthKeyringBackendKind::default(),
            /*auth_route_config*/ None,
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
            /*forced_chatgpt_workspace_id*/ None,
            /*chatgpt_base_url*/ None,
            AuthKeyringBackendKind::default(),
            /*auth_route_config*/ None,
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

    let replacement_bundle = replacement_requirements_bundle();
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
    let replacement_bundle = replacement_requirements_bundle();
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
fn bundle_response_conversion_uses_only_managed_layers_and_preserves_order() {
    let legacy_config = delivered_fragment("legacy", "model = [");
    let baseline_config = delivered_fragment("baseline", "model = \"baseline\"");
    let high_config = delivered_fragment("high", "model = \"high\"");
    let low_config = delivered_fragment("low", "model = \"low\"");
    let overlay_requirements =
        delivered_fragment("overlay", "allowed_approval_policies = [\"never\"]");
    let response = ConfigBundleResponse {
        config_toml: Some(Some(Box::new(codex_backend_client::DeliveredConfigToml {
            enterprise_managed: Some(Some(vec![legacy_config])),
            managed_layers: delivered_managed_layers(
                vec![baseline_config],
                vec![high_config, low_config],
            ),
        }))),
        requirements_toml: Some(Some(Box::new(
            codex_backend_client::DeliveredRequirementsToml {
                enterprise_managed: Some(Some(vec![delivered_fragment(
                    "legacy",
                    "allowed_approval_policies = [",
                )])),
                managed_layers: delivered_managed_layers(Vec::new(), vec![overlay_requirements]),
            },
        ))),
    };

    assert_eq!(
        bundle_from_response(response),
        Ok(CloudConfigBundle {
            config_toml: CloudConfigTomlBundle {
                managed_layers: CloudConfigTomlManagedLayers {
                    baseline: vec![config_fragment("baseline", "model = \"baseline\"")],
                    system_overlay: vec![
                        config_fragment("high", "model = \"high\""),
                        config_fragment("low", "model = \"low\""),
                    ],
                },
            },
            requirements_toml: CloudRequirementsTomlBundle {
                managed_layers: CloudRequirementsTomlManagedLayers {
                    baseline: Vec::new(),
                    system_overlay: vec![requirements_fragment(
                        "overlay",
                        "allowed_approval_policies = [\"never\"]",
                    )],
                },
            },
        })
    );
}

#[test]
fn bundle_response_conversion_treats_missing_or_null_sections_as_empty() {
    assert_eq!(
        bundle_from_response(ConfigBundleResponse::new()),
        Ok(CloudConfigBundle::default())
    );
    assert_eq!(
        bundle_from_response(ConfigBundleResponse {
            config_toml: Some(None),
            requirements_toml: Some(None),
        }),
        Ok(CloudConfigBundle::default())
    );
}

#[test]
fn bundle_response_conversion_accepts_explicitly_empty_managed_layers() {
    assert_eq!(
        bundle_from_response(ConfigBundleResponse {
            config_toml: Some(Some(Box::new(codex_backend_client::DeliveredConfigToml {
                enterprise_managed: Some(Some(vec![delivered_fragment("legacy", "model = [")])),
                managed_layers: delivered_managed_layers(Vec::new(), Vec::new()),
            }))),
            requirements_toml: Some(Some(Box::new(
                codex_backend_client::DeliveredRequirementsToml {
                    enterprise_managed: None,
                    managed_layers: delivered_managed_layers(Vec::new(), Vec::new()),
                },
            ))),
        }),
        Ok(CloudConfigBundle::default())
    );
}

#[test]
fn bundle_response_conversion_rejects_present_section_without_managed_layers() {
    for managed_layers in [None, Some(None)] {
        assert_eq!(
            bundle_from_response(ConfigBundleResponse {
                config_toml: Some(Some(Box::new(codex_backend_client::DeliveredConfigToml {
                    enterprise_managed: Some(Some(vec![delivered_fragment(
                        "legacy",
                        "model = \"ignored\"",
                    )])),
                    managed_layers,
                }))),
                requirements_toml: Some(None),
            }),
            Err(BundleRequestError::InvalidBundle {
                message: concat!(
                    "cloud config bundle config_toml is present but managed_layers ",
                    "is missing or null"
                )
                .to_string(),
            })
        );
    }

    for managed_layers in [None, Some(None)] {
        assert_eq!(
            bundle_from_response(ConfigBundleResponse {
                config_toml: Some(None),
                requirements_toml: Some(Some(Box::new(
                    codex_backend_client::DeliveredRequirementsToml {
                        enterprise_managed: Some(Some(Vec::new())),
                        managed_layers,
                    },
                ))),
            }),
            Err(BundleRequestError::InvalidBundle {
                message: concat!(
                    "cloud config bundle requirements_toml is present but managed_layers ",
                    "is missing or null"
                )
                .to_string(),
            })
        );
    }
}
