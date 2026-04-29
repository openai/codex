use super::*;
use crate::ModelsManagerConfig;
use chrono::Utc;
use codex_app_server_protocol::AuthMode;
use codex_login::AuthCredentialsStoreMode;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::ExternalAuth;
use codex_login::ExternalAuthRefreshContext;
use codex_login::ExternalAuthTokens;
use codex_login::TokenData;
use codex_protocol::openai_models::ModelsResponse;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::VecDeque;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tempfile::tempdir;

#[path = "model_info_overrides_tests.rs"]
mod model_info_overrides_tests;

fn remote_model(slug: &str, display: &str, priority: i32) -> ModelInfo {
    remote_model_with_visibility(slug, display, priority, "list")
}

fn remote_model_with_visibility(
    slug: &str,
    display: &str,
    priority: i32,
    visibility: &str,
) -> ModelInfo {
    serde_json::from_value(json!({
            "slug": slug,
            "display_name": display,
            "description": format!("{display} desc"),
            "default_reasoning_level": "medium",
            "supported_reasoning_levels": [{"effort": "low", "description": "low"}, {"effort": "medium", "description": "medium"}],
            "shell_type": "shell_command",
            "visibility": visibility,
            "minimal_client_version": [0, 1, 0],
            "supported_in_api": true,
            "priority": priority,
            "upgrade": null,
            "base_instructions": "base instructions",
            "supports_reasoning_summaries": false,
            "support_verbosity": false,
            "default_verbosity": null,
            "apply_patch_tool_type": null,
            "truncation_policy": {"mode": "bytes", "limit": 10_000},
            "supports_parallel_tool_calls": false,
            "supports_image_detail_original": false,
            "context_window": 272_000,
            "max_context_window": 272_000,
            "experimental_supported_tools": [],
        }))
        .expect("valid model")
}

fn assert_models_contain(actual: &[ModelInfo], expected: &[ModelInfo]) {
    for model in expected {
        assert!(
            actual.iter().any(|candidate| candidate.slug == model.slug),
            "expected model {} in cached list",
            model.slug
        );
    }
}

#[derive(Debug)]
struct TestModelsEndpoint {
    has_command_auth: bool,
    uses_codex_backend: bool,
    responses: Mutex<VecDeque<Vec<ModelInfo>>>,
    fetch_count: AtomicUsize,
}

impl TestModelsEndpoint {
    fn new(responses: Vec<Vec<ModelInfo>>) -> Arc<Self> {
        Arc::new(Self {
            has_command_auth: false,
            uses_codex_backend: true,
            responses: Mutex::new(responses.into()),
            fetch_count: AtomicUsize::new(0),
        })
    }

    fn without_refresh(responses: Vec<Vec<ModelInfo>>) -> Arc<Self> {
        Arc::new(Self {
            has_command_auth: false,
            uses_codex_backend: false,
            responses: Mutex::new(responses.into()),
            fetch_count: AtomicUsize::new(0),
        })
    }

    fn fetch_count(&self) -> usize {
        self.fetch_count.load(Ordering::SeqCst)
    }
}

#[derive(Debug)]
struct TestExternalApiKeyAuth;

#[async_trait]
impl ExternalAuth for TestExternalApiKeyAuth {
    fn auth_mode(&self) -> AuthMode {
        AuthMode::ApiKey
    }

    async fn resolve(&self) -> std::io::Result<Option<ExternalAuthTokens>> {
        Ok(Some(ExternalAuthTokens::access_token_only(
            "test-external-api-key",
        )))
    }

    async fn refresh(
        &self,
        _context: ExternalAuthRefreshContext,
    ) -> std::io::Result<ExternalAuthTokens> {
        Ok(ExternalAuthTokens::access_token_only(
            "test-external-api-key",
        ))
    }
}

#[derive(Debug)]
struct TestUnresolvedExternalApiKeyAuth;

#[async_trait]
impl ExternalAuth for TestUnresolvedExternalApiKeyAuth {
    fn auth_mode(&self) -> AuthMode {
        AuthMode::ApiKey
    }

    async fn refresh(
        &self,
        _context: ExternalAuthRefreshContext,
    ) -> std::io::Result<ExternalAuthTokens> {
        Err(std::io::Error::other("unresolved test auth"))
    }
}

#[async_trait]
impl ModelsEndpointClient for TestModelsEndpoint {
    fn has_command_auth(&self) -> bool {
        self.has_command_auth
    }

    async fn uses_codex_backend(&self) -> bool {
        self.uses_codex_backend
    }

    async fn list_models(
        &self,
        _client_version: &str,
    ) -> CoreResult<(Vec<ModelInfo>, Option<String>)> {
        self.fetch_count.fetch_add(1, Ordering::SeqCst);
        let models = self
            .responses
            .lock()
            .expect("responses lock should not be poisoned")
            .pop_front()
            .unwrap_or_default();
        Ok((models, None))
    }
}

fn openai_manager_for_tests(
    codex_home: std::path::PathBuf,
    endpoint_client: Arc<dyn ModelsEndpointClient>,
) -> OpenAiModelsManager {
    openai_manager_for_tests_with_auth(
        codex_home,
        endpoint_client,
        Some(AuthManager::from_auth_for_testing(
            CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        )),
    )
}

fn openai_manager_for_tests_with_auth(
    codex_home: std::path::PathBuf,
    endpoint_client: Arc<dyn ModelsEndpointClient>,
    auth_manager: Option<Arc<AuthManager>>,
) -> OpenAiModelsManager {
    OpenAiModelsManager::new(
        codex_home,
        endpoint_client,
        auth_manager,
        CollaborationModesConfig::default(),
    )
}

fn static_manager_for_tests(model_catalog: ModelsResponse) -> StaticModelsManager {
    StaticModelsManager::new(
        /*auth_manager*/ None,
        model_catalog,
        CollaborationModesConfig::default(),
    )
}

async fn chatgpt_auth_tokens_for_tests(codex_home: &Path) -> CodexAuth {
    let auth_dot_json = codex_login::AuthDotJson {
        auth_mode: Some(AuthMode::ChatgptAuthTokens),
        openai_api_key: None,
        tokens: Some(TokenData {
            id_token: codex_login::token_data::parse_chatgpt_jwt_claims(
                "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.\
eyJlbWFpbCI6InVzZXJAZXhhbXBsZS5jb20iLCJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF9wbGFuX3R5cGUiOiJwcm8iLCJjaGF0Z3B0X3VzZXJfaWQiOiJ1c2VyLWlkIiwiY2hhdGdwdF9hY2NvdW50X2lkIjoiYWNjb3VudC1pZCJ9fQ.\
c2ln",
            )
            .expect("fake id token should parse"),
            access_token: "Access Token".to_string(),
            refresh_token: "test".to_string(),
            account_id: Some("account_id".to_string()),
        }),
        last_refresh: Some(Utc::now()),
        agent_identity: None,
    };
    std::fs::create_dir_all(codex_home).expect("codex home should be created");
    std::fs::write(
        codex_home.join("auth.json"),
        serde_json::to_string(&auth_dot_json).expect("auth should serialize"),
    )
    .expect("auth.json should be written");

    CodexAuth::from_auth_storage(
        codex_home,
        AuthCredentialsStoreMode::File,
        /*chatgpt_base_url*/ None,
    )
    .await
    .expect("auth should load")
    .expect("auth should be present")
}

#[tokio::test]
async fn get_model_info_tracks_fallback_usage() {
    let codex_home = tempdir().expect("temp dir");
    let config = ModelsManagerConfig::default();
    let manager = openai_manager_for_tests(
        codex_home.path().to_path_buf(),
        TestModelsEndpoint::new(Vec::new()),
    );
    let known_slug = manager
        .get_remote_models()
        .await
        .first()
        .expect("bundled models should include at least one model")
        .slug
        .clone();

    let known = manager.get_model_info(known_slug.as_str(), &config).await;
    assert!(!known.used_fallback_model_metadata);
    assert_eq!(known.slug, known_slug);

    let unknown = manager
        .get_model_info("model-that-does-not-exist", &config)
        .await;
    assert!(unknown.used_fallback_model_metadata);
    assert_eq!(unknown.slug, "model-that-does-not-exist");
}

#[tokio::test]
async fn get_model_info_uses_custom_catalog() {
    let config = ModelsManagerConfig::default();
    let mut overlay = remote_model("gpt-overlay", "Overlay", /*priority*/ 0);
    overlay.supports_image_detail_original = true;

    let manager = static_manager_for_tests(ModelsResponse {
        models: vec![overlay],
    });

    let model_info = manager
        .get_model_info("gpt-overlay-experiment", &config)
        .await;

    assert_eq!(model_info.slug, "gpt-overlay-experiment");
    assert_eq!(model_info.display_name, "Overlay");
    assert_eq!(model_info.context_window, Some(272_000));
    assert!(model_info.supports_image_detail_original);
    assert!(!model_info.supports_parallel_tool_calls);
    assert!(!model_info.used_fallback_model_metadata);
}

#[tokio::test]
async fn get_model_info_matches_namespaced_suffix() {
    let config = ModelsManagerConfig::default();
    let mut remote = remote_model("gpt-image", "Image", /*priority*/ 0);
    remote.supports_image_detail_original = true;
    let manager = static_manager_for_tests(ModelsResponse {
        models: vec![remote],
    });
    let namespaced_model = "custom/gpt-image".to_string();

    let model_info = manager.get_model_info(&namespaced_model, &config).await;

    assert_eq!(model_info.slug, namespaced_model);
    assert!(model_info.supports_image_detail_original);
    assert!(!model_info.used_fallback_model_metadata);
}

#[tokio::test]
async fn get_model_info_rejects_multi_segment_namespace_suffix_matching() {
    let codex_home = tempdir().expect("temp dir");
    let config = ModelsManagerConfig::default();
    let manager = openai_manager_for_tests(
        codex_home.path().to_path_buf(),
        TestModelsEndpoint::new(Vec::new()),
    );
    let known_slug = manager
        .get_remote_models()
        .await
        .first()
        .expect("bundled models should include at least one model")
        .slug
        .clone();
    let namespaced_model = format!("ns1/ns2/{known_slug}");

    let model_info = manager.get_model_info(&namespaced_model, &config).await;

    assert_eq!(model_info.slug, namespaced_model);
    assert!(model_info.used_fallback_model_metadata);
}

#[tokio::test]
async fn refresh_available_models_sorts_by_priority() {
    let remote_models = vec![
        remote_model("priority-low", "Low", /*priority*/ 1),
        remote_model("priority-high", "High", /*priority*/ 0),
    ];
    let codex_home = tempdir().expect("temp dir");
    let endpoint = TestModelsEndpoint::new(vec![remote_models.clone()]);
    let manager = openai_manager_for_tests(codex_home.path().to_path_buf(), endpoint.clone());

    manager
        .refresh_available_models(RefreshStrategy::OnlineIfUncached)
        .await
        .expect("refresh succeeds");
    let cached_remote = manager.get_remote_models().await;
    assert_models_contain(&cached_remote, &remote_models);

    let available = manager.list_models(RefreshStrategy::OnlineIfUncached).await;
    let high_idx = available
        .iter()
        .position(|model| model.model == "priority-high")
        .expect("priority-high should be listed");
    let low_idx = available
        .iter()
        .position(|model| model.model == "priority-low")
        .expect("priority-low should be listed");
    assert!(
        high_idx < low_idx,
        "higher priority should be listed before lower priority"
    );
    assert_eq!(endpoint.fetch_count(), 1, "expected a single model fetch");
}

#[tokio::test]
async fn refresh_available_models_uses_cache_when_fresh() {
    let remote_models = vec![remote_model("cached", "Cached", /*priority*/ 5)];
    let codex_home = tempdir().expect("temp dir");
    let endpoint = TestModelsEndpoint::new(vec![remote_models.clone()]);
    let manager = openai_manager_for_tests(codex_home.path().to_path_buf(), endpoint.clone());

    manager
        .refresh_available_models(RefreshStrategy::OnlineIfUncached)
        .await
        .expect("first refresh succeeds");
    assert_models_contain(&manager.get_remote_models().await, &remote_models);

    // Second call should read from cache and avoid the network.
    manager
        .refresh_available_models(RefreshStrategy::OnlineIfUncached)
        .await
        .expect("cached refresh succeeds");
    assert_models_contain(&manager.get_remote_models().await, &remote_models);
    assert_eq!(
        endpoint.fetch_count(),
        1,
        "cache hit should avoid a second model fetch"
    );
}

#[tokio::test]
async fn refresh_available_models_refetches_when_cache_stale() {
    let initial_models = vec![remote_model("stale", "Stale", /*priority*/ 1)];
    let codex_home = tempdir().expect("temp dir");
    let updated_models = vec![remote_model("fresh", "Fresh", /*priority*/ 9)];
    let endpoint = TestModelsEndpoint::new(vec![initial_models.clone(), updated_models.clone()]);
    let manager = openai_manager_for_tests(codex_home.path().to_path_buf(), endpoint.clone());

    manager
        .refresh_available_models(RefreshStrategy::OnlineIfUncached)
        .await
        .expect("initial refresh succeeds");

    // Rewrite cache with an old timestamp so it is treated as stale.
    manager
        .cache_manager
        .manipulate_cache_for_test(|fetched_at| {
            *fetched_at = Utc::now() - chrono::Duration::hours(1);
        })
        .await
        .expect("cache manipulation succeeds");

    manager
        .refresh_available_models(RefreshStrategy::OnlineIfUncached)
        .await
        .expect("second refresh succeeds");
    assert_models_contain(&manager.get_remote_models().await, &updated_models);
    assert_eq!(
        endpoint.fetch_count(),
        2,
        "stale cache refresh should fetch models again"
    );
}

#[tokio::test]
async fn refresh_available_models_refetches_when_version_mismatch() {
    let initial_models = vec![remote_model("old", "Old", /*priority*/ 1)];
    let codex_home = tempdir().expect("temp dir");
    let updated_models = vec![remote_model("new", "New", /*priority*/ 2)];
    let endpoint = TestModelsEndpoint::new(vec![initial_models.clone(), updated_models.clone()]);
    let manager = openai_manager_for_tests(codex_home.path().to_path_buf(), endpoint.clone());

    manager
        .refresh_available_models(RefreshStrategy::OnlineIfUncached)
        .await
        .expect("initial refresh succeeds");

    manager
        .cache_manager
        .mutate_cache_for_test(|cache| {
            let client_version = crate::client_version_to_whole();
            cache.client_version = Some(format!("{client_version}-mismatch"));
        })
        .await
        .expect("cache mutation succeeds");

    manager
        .refresh_available_models(RefreshStrategy::OnlineIfUncached)
        .await
        .expect("second refresh succeeds");
    assert_models_contain(&manager.get_remote_models().await, &updated_models);
    assert_eq!(
        endpoint.fetch_count(),
        2,
        "version mismatch should fetch models again"
    );
}

#[tokio::test]
async fn refresh_available_models_drops_removed_remote_models() {
    let initial_models = vec![remote_model(
        "remote-old",
        "Remote Old",
        /*priority*/ 1,
    )];
    let codex_home = tempdir().expect("temp dir");
    let refreshed_models = vec![remote_model(
        "remote-new",
        "Remote New",
        /*priority*/ 1,
    )];
    let endpoint = TestModelsEndpoint::new(vec![initial_models, refreshed_models]);
    let mut manager = openai_manager_for_tests(codex_home.path().to_path_buf(), endpoint.clone());
    manager.cache_manager.set_ttl(Duration::ZERO);

    manager
        .refresh_available_models(RefreshStrategy::OnlineIfUncached)
        .await
        .expect("initial refresh succeeds");

    manager
        .refresh_available_models(RefreshStrategy::OnlineIfUncached)
        .await
        .expect("second refresh succeeds");

    let available = manager
        .try_list_models()
        .expect("models should be available");
    assert!(
        available.iter().any(|preset| preset.model == "remote-new"),
        "new remote model should be listed"
    );
    assert!(
        !available.iter().any(|preset| preset.model == "remote-old"),
        "removed remote model should not be listed"
    );
    assert_eq!(
        endpoint.fetch_count(),
        2,
        "second refresh should fetch models again"
    );
}

#[tokio::test]
async fn refresh_available_models_skips_network_without_chatgpt_auth() {
    let dynamic_slug = "dynamic-model-only-for-test-noauth";
    let codex_home = tempdir().expect("temp dir");
    let endpoint = TestModelsEndpoint::without_refresh(vec![vec![remote_model(
        dynamic_slug,
        "No Auth",
        /*priority*/ 1,
    )]]);
    let manager = openai_manager_for_tests_with_auth(
        codex_home.path().to_path_buf(),
        endpoint.clone(),
        /*auth_manager*/ None,
    );

    manager
        .refresh_available_models(RefreshStrategy::Online)
        .await
        .expect("refresh should no-op without chatgpt auth");
    let cached_remote = manager.get_remote_models().await;
    assert!(
        !cached_remote
            .iter()
            .any(|candidate| candidate.slug == dynamic_slug),
        "remote refresh should be skipped without chatgpt auth"
    );
    assert_eq!(
        endpoint.fetch_count(),
        0,
        "endpoint that cannot refresh should avoid model fetches"
    );
}

#[derive(Debug)]
struct TestAuthAwareModelsEndpoint {
    auth_manager: Option<Arc<AuthManager>>,
    responses: Mutex<VecDeque<Vec<ModelInfo>>>,
    fetch_count: AtomicUsize,
}

impl TestAuthAwareModelsEndpoint {
    fn new(auth_manager: Option<Arc<AuthManager>>, responses: Vec<Vec<ModelInfo>>) -> Arc<Self> {
        Arc::new(Self {
            auth_manager,
            responses: Mutex::new(responses.into()),
            fetch_count: AtomicUsize::new(0),
        })
    }

    fn fetch_count(&self) -> usize {
        self.fetch_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ModelsEndpointClient for TestAuthAwareModelsEndpoint {
    fn has_command_auth(&self) -> bool {
        false
    }

    async fn uses_codex_backend(&self) -> bool {
        match self.auth_manager.as_ref() {
            Some(auth_manager) => auth_manager
                .auth()
                .await
                .as_ref()
                .is_some_and(CodexAuth::uses_codex_backend),
            None => false,
        }
    }

    async fn list_models(
        &self,
        _client_version: &str,
    ) -> CoreResult<(Vec<ModelInfo>, Option<String>)> {
        self.fetch_count.fetch_add(1, Ordering::SeqCst);
        let models = self
            .responses
            .lock()
            .expect("responses lock should not be poisoned")
            .pop_front()
            .unwrap_or_default();
        Ok((models, None))
    }
}

#[tokio::test]
async fn refresh_available_models_skips_network_when_external_api_key_overrides_chatgpt_auth() {
    let dynamic_slug = "dynamic-model-only-for-test-external-api-key";
    let codex_home = tempdir().expect("temp dir");
    let auth_manager =
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    auth_manager.set_external_auth(Arc::new(TestExternalApiKeyAuth));
    let endpoint = TestAuthAwareModelsEndpoint::new(
        Some(Arc::clone(&auth_manager)),
        vec![vec![remote_model(
            dynamic_slug,
            "External API Key",
            /*priority*/ 1,
        )]],
    );
    let manager = openai_manager_for_tests_with_auth(
        codex_home.path().to_path_buf(),
        endpoint.clone(),
        Some(auth_manager),
    );

    manager
        .refresh_available_models(RefreshStrategy::Online)
        .await
        .expect("refresh should no-op with API key auth");
    let cached_remote = manager.get_remote_models().await;

    assert!(
        !cached_remote
            .iter()
            .any(|candidate| candidate.slug == dynamic_slug),
        "remote refresh should be skipped when external API key auth is active"
    );
    assert_eq!(
        endpoint.fetch_count(),
        0,
        "endpoint should avoid model fetches when external API key auth is active"
    );
}

#[tokio::test]
async fn refresh_available_models_uses_cached_chatgpt_when_external_api_key_is_unresolved() {
    let dynamic_slug = "dynamic-model-only-for-test-unresolved-external-api-key";
    let codex_home = tempdir().expect("temp dir");
    let auth_manager =
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    auth_manager.set_external_auth(Arc::new(TestUnresolvedExternalApiKeyAuth));
    let endpoint = TestAuthAwareModelsEndpoint::new(
        Some(Arc::clone(&auth_manager)),
        vec![vec![remote_model(
            dynamic_slug,
            "Unresolved External API Key",
            /*priority*/ 1,
        )]],
    );
    let manager = openai_manager_for_tests_with_auth(
        codex_home.path().to_path_buf(),
        endpoint.clone(),
        Some(auth_manager),
    );

    manager
        .refresh_available_models(RefreshStrategy::Online)
        .await
        .expect("refresh should fall back to cached ChatGPT auth");

    assert!(
        manager
            .get_remote_models()
            .await
            .iter()
            .any(|candidate| candidate.slug == dynamic_slug),
        "remote refresh should include models fetched with cached ChatGPT auth"
    );
    assert_eq!(
        endpoint.fetch_count(),
        1,
        "endpoint should fetch models when unresolved external API key falls back to ChatGPT auth"
    );
}

#[tokio::test]
async fn refresh_available_models_fetches_with_chatgpt_auth_tokens() {
    let dynamic_slug = "dynamic-model-only-for-test-chatgpt-auth-tokens";
    let codex_home = tempdir().expect("temp dir");
    let endpoint = TestModelsEndpoint::new(vec![vec![remote_model(
        dynamic_slug,
        "ChatGPT Auth Tokens",
        /*priority*/ 1,
    )]]);
    let auth = chatgpt_auth_tokens_for_tests(codex_home.path()).await;
    let manager = openai_manager_for_tests_with_auth(
        codex_home.path().to_path_buf(),
        endpoint.clone(),
        Some(AuthManager::from_auth_for_testing(auth)),
    );

    manager
        .refresh_available_models(RefreshStrategy::Online)
        .await
        .expect("refresh should fetch with ChatGPT auth tokens");

    assert!(
        manager
            .get_remote_models()
            .await
            .iter()
            .any(|candidate| candidate.slug == dynamic_slug),
        "remote refresh should include models fetched with ChatGPT auth tokens"
    );
    assert_eq!(
        endpoint.fetch_count(),
        1,
        "endpoint should fetch models with ChatGPT auth tokens"
    );
}

#[test]
fn build_available_models_picks_default_after_hiding_hidden_models() {
    let manager = static_manager_for_tests(ModelsResponse { models: Vec::new() });

    let hidden_model =
        remote_model_with_visibility("hidden", "Hidden", /*priority*/ 0, "hide");
    let visible_model =
        remote_model_with_visibility("visible", "Visible", /*priority*/ 1, "list");

    let expected_hidden = ModelPreset::from(hidden_model.clone());
    let mut expected_visible = ModelPreset::from(visible_model.clone());
    expected_visible.is_default = true;

    let available = manager.build_available_models(vec![hidden_model, visible_model]);

    assert_eq!(available, vec![expected_hidden, expected_visible]);
}

#[tokio::test]
async fn static_manager_reads_latest_auth_mode() {
    let auth_manager =
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let chatgpt_only_model = {
        let mut model = remote_model("chatgpt-only", "ChatGPT Only", /*priority*/ 0);
        model.supported_in_api = false;
        model
    };
    let api_model = remote_model("api-model", "API Model", /*priority*/ 1);
    let manager = StaticModelsManager::new(
        Some(Arc::clone(&auth_manager)),
        ModelsResponse {
            models: vec![chatgpt_only_model, api_model],
        },
        CollaborationModesConfig::default(),
    );

    let chatgpt_models = manager.list_models(RefreshStrategy::Online).await;
    assert_eq!(
        chatgpt_models
            .iter()
            .map(|model| model.model.as_str())
            .collect::<Vec<_>>(),
        vec!["chatgpt-only", "api-model"]
    );

    auth_manager.set_external_auth(Arc::new(TestExternalApiKeyAuth));
    let api_models = manager.list_models(RefreshStrategy::Online).await;

    assert_eq!(
        api_models
            .iter()
            .map(|model| model.model.as_str())
            .collect::<Vec<_>>(),
        vec!["api-model"]
    );
}

#[test]
fn bundled_models_json_roundtrips() {
    let response = crate::bundled_models_response()
        .unwrap_or_else(|err| panic!("bundled models.json should parse: {err}"));

    let serialized =
        serde_json::to_string(&response).expect("bundled models.json should serialize");
    let roundtripped: ModelsResponse =
        serde_json::from_str(&serialized).expect("serialized models.json should deserialize");

    assert_eq!(
        response, roundtripped,
        "bundled models.json should round trip through serde"
    );
    assert!(
        !response.models.is_empty(),
        "bundled models.json should contain at least one model"
    );
}
