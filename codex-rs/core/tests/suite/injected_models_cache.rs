//! Integration coverage for model catalogs supplied through the public cache interface.

use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use anyhow::Result;
use base64::Engine;
use chrono::Utc;
use codex_http_client::HttpClientFactory;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider_info::built_in_model_providers;
use codex_models_manager::cache::ModelsCache;
use codex_models_manager::cache::ModelsCacheEntry;
use codex_models_manager::cache::ModelsCacheError;
use codex_models_manager::cache::ModelsCacheFuture;
use codex_models_manager::manager::ModelCatalogSettings;
use codex_models_manager::manager::ModelsEndpointClient;
use codex_models_manager::manager::ModelsEndpointFuture;
use codex_models_manager::manager::ModelsEndpointResponse;
use codex_models_manager::manager::OpenAiModelsManager;
use codex_models_manager::manager::RefreshStrategy;
use codex_models_manager::manager::SharedModelsManager;
use codex_models_manager::model_info::model_info_from_slug;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::error::Result as CoreResult;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::submit_thread_settings;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use test_case::test_case;
use tokio::sync::oneshot;
use tokio::time::Duration;
use tokio::time::timeout;

#[derive(Debug)]
struct StoreGate {
    entered: oneshot::Sender<()>,
    release: oneshot::Receiver<()>,
}

#[derive(Debug)]
struct TestModelsCache {
    entry: Option<ModelsCacheEntry>,
    load_error: bool,
    stored_entries: Mutex<Vec<ModelsCacheEntry>>,
    store_gate: Mutex<Option<StoreGate>>,
}

impl ModelsCache for TestModelsCache {
    fn load<'a>(
        &'a self,
        _client_version: &'a str,
    ) -> ModelsCacheFuture<'a, Result<Option<ModelsCacheEntry>, ModelsCacheError>> {
        Box::pin(async move {
            if self.load_error {
                Err(ModelsCacheError::new("test load failure"))
            } else {
                Ok(self
                    .stored_entries
                    .lock()
                    .expect("stored entries lock should not be poisoned")
                    .last()
                    .cloned()
                    .or_else(|| self.entry.clone()))
            }
        })
    }

    fn store<'a>(
        &'a self,
        entry: &'a ModelsCacheEntry,
    ) -> ModelsCacheFuture<'a, Result<(), ModelsCacheError>> {
        Box::pin(async move {
            self.stored_entries
                .lock()
                .expect("stored entries lock should not be poisoned")
                .push(entry.clone());
            let gate = self
                .store_gate
                .lock()
                .expect("store gate lock should not be poisoned")
                .take();
            if let Some(StoreGate { entered, release }) = gate {
                let _ = entered.send(());
                let _ = release.await;
            }
            Ok(())
        })
    }

    fn refresh_ttl<'a>(
        &'a self,
        _client_version: &'a str,
        _identity: &'a str,
        _etag: &'a str,
    ) -> ModelsCacheFuture<'a, Result<(), ModelsCacheError>> {
        Box::pin(async move {
            let mut entry = self
                .entry
                .clone()
                .ok_or_else(|| ModelsCacheError::new("cache not found"))?;
            entry.fetched_at = Utc::now();
            self.stored_entries
                .lock()
                .expect("stored entries lock should not be poisoned")
                .push(entry);
            Ok(())
        })
    }
}

#[derive(Debug)]
struct TestModelsEndpoint {
    models: Mutex<Vec<ModelInfo>>,
    fetch_count: AtomicUsize,
}

impl TestModelsEndpoint {
    fn new(models: Vec<ModelInfo>) -> Arc<Self> {
        Arc::new(Self {
            models: Mutex::new(models),
            fetch_count: AtomicUsize::new(0),
        })
    }
}

impl ModelsEndpointClient for TestModelsEndpoint {
    fn identity(&self) -> Option<String> {
        Some("test-provider".to_string())
    }

    fn has_command_auth(&self) -> bool {
        false
    }

    fn uses_codex_backend(&self) -> ModelsEndpointFuture<'_, bool> {
        Box::pin(async { true })
    }

    fn list_models<'a>(
        &'a self,
        _client_version: &'a str,
        _http_client_factory: HttpClientFactory,
    ) -> ModelsEndpointFuture<'a, CoreResult<ModelsEndpointResponse>> {
        Box::pin(async move {
            self.fetch_count.fetch_add(1, Ordering::SeqCst);
            Ok(ModelsEndpointResponse {
                models: self.models.lock().expect("model response lock").clone(),
                etag: None,
                identity: self.identity().expect("test endpoint identity"),
            })
        })
    }
}

fn remote_model(slug: &str) -> ModelInfo {
    ModelInfo {
        visibility: ModelVisibility::List,
        ..model_info_from_slug(slug)
    }
}

fn models_manager(
    cache: Arc<dyn ModelsCache>,
    endpoint: Arc<TestModelsEndpoint>,
) -> SharedModelsManager {
    Arc::new(OpenAiModelsManager::new_with_cache(
        cache,
        endpoint,
        Some(AuthManager::from_auth_for_testing(
            CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        )),
    ))
}

async fn run_agent_with_model(models_manager: SharedModelsManager, model_slug: &str) -> Result<()> {
    let server = responses::start_mock_server().await;
    let response_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_models_manager(models_manager)
        .with_config(|config| {
            config
                .features
                .enable(codex_features::Feature::RuntimeAgentInstances)
                .expect("test config should allow enabling runtime auth");
            // Model inference uses a third-party fixture; a custom ChatGPT base must not block startup.
            config.chatgpt_base_url = "https://custom.example.test/backend-api".to_string();
        });
    let test = builder.build(&server).await?;
    let available_models = test
        .thread_manager
        .get_models_manager()
        .list_models(
            RefreshStrategy::OnlineIfUncached,
            codex_core::test_support::default_http_client_factory(),
        )
        .await;
    assert!(
        available_models
            .iter()
            .any(|model| model.model == model_slug)
    );

    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            model: Some(model_slug.to_string()),
            ..Default::default()
        },
    )
    .await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "hello".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    loop {
        if matches!(
            wait_for_event(&test.codex, |_| true).await,
            EventMsg::TurnComplete(_)
        ) {
            break;
        }
    }

    assert_eq!(
        response_mock.single_request().body_json()["model"],
        model_slug
    );
    assert!(!test.codex_home_path().join("models_cache.json").exists());
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn injected_cache_hit_drives_agent_model_selection() -> Result<()> {
    let model_slug = "injected-cache-model";
    let cache = Arc::new(TestModelsCache {
        entry: Some(ModelsCacheEntry {
            identity: Some("test-provider".to_string()),
            fetched_at: Utc::now(),
            etag: None,
            client_version: Some(codex_models_manager::client_version_to_whole()),
            models: vec![remote_model(model_slug)],
        }),
        load_error: false,
        stored_entries: Mutex::new(Vec::new()),
        store_gate: Mutex::new(None),
    });
    let endpoint = TestModelsEndpoint::new(Vec::new());

    run_agent_with_model(models_manager(cache, endpoint.clone()), model_slug).await?;

    assert_eq!(endpoint.fetch_count.load(Ordering::SeqCst), 0);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn injected_cache_error_falls_back_for_agent_model_selection() -> Result<()> {
    let model_slug = "injected-cache-fallback-model";
    let cache = Arc::new(TestModelsCache {
        entry: None,
        load_error: true,
        stored_entries: Mutex::new(Vec::new()),
        store_gate: Mutex::new(None),
    });
    let endpoint = TestModelsEndpoint::new(vec![remote_model(model_slug)]);

    run_agent_with_model(models_manager(cache.clone(), endpoint.clone()), model_slug).await?;

    assert!(endpoint.fetch_count.load(Ordering::SeqCst) >= 1);
    assert!(
        !cache
            .stored_entries
            .lock()
            .expect("stored entries lock should not be poisoned")
            .is_empty()
    );
    Ok(())
}

#[test_case(false; "without_cache")]
#[test_case(true; "with_host_cache")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn threads_follow_provider_changes_and_preserve_host_cache_policy(
    use_cache: bool,
) -> Result<()> {
    let host = responses::start_mock_server().await;
    let inference = responses::start_mock_server().await;
    let model = "host-catalog-model";
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/host-models"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(ModelsResponse {
                models: vec![remote_model(model)],
            }),
        )
        .expect(1..)
        .mount(&host)
        .await;
    let auth = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("host-catalog-key"));
    let mut info =
        built_in_model_providers(Some(format!("{}/v1", inference.uri())))["openai"].clone();
    info.model_catalog_url = Some(format!("{}/host-models", host.uri()).into());
    let cache = Arc::new(TestModelsCache {
        entry: None,
        load_error: false,
        stored_entries: Mutex::default(),
        store_gate: Mutex::default(),
    });
    let settings = if use_cache {
        ModelCatalogSettings::with_cache(cache.clone())
    } else {
        ModelCatalogSettings::default()
    };
    settings.set_api_key_model_discovery_enabled(/*enabled*/ true);
    let test = test_codex()
        .with_auth_manager(auth)
        .with_model_catalog_settings(settings)
        .with_config(move |config| {
            config.model_provider = info;
            config
                .features
                .enable(codex_features::Feature::RuntimeAgentInstances)
                .expect("runtime identity feature should be available");
            config.model = None;
        })
        .build_with_auto_env(&inference)
        .await?;
    assert_eq!(test.session_configured.model, model);
    let mut disabled = test.config.clone();
    disabled
        .features
        .disable(codex_features::Feature::RuntimeAgentInstances)?;
    let sibling = test
        .thread_manager
        .start_thread(StartThreadOptions::new(disabled))
        .await?;
    assert_eq!(sibling.session_configured.model, model);
    let second_host = responses::start_mock_server().await;
    let second_inference = responses::start_mock_server().await;
    let second_model = "second-provider-model";
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/second-models"))
        .and(wiremock::matchers::header(
            "authorization",
            "Bearer host-catalog-key",
        ))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(ModelsResponse {
                models: vec![remote_model(second_model)],
            }),
        )
        .expect(1..)
        .mount(&second_host)
        .await;
    let mut changed = test.config.clone();
    changed.model_provider.base_url = Some(format!("{}/v1", second_inference.uri()));
    changed.model_provider.model_catalog_url =
        Some(format!("{}/second-models", second_host.uri()).into());
    let changed_thread = test
        .thread_manager
        .start_thread(StartThreadOptions::new(changed))
        .await?;
    assert_eq!(changed_thread.session_configured.model, second_model);
    if use_cache {
        assert_eq!(
            cache
                .stored_entries
                .lock()
                .expect("stored entries lock should not be poisoned")
                .last()
                .expect("provider catalog should be cached")
                .models,
            vec![remote_model(second_model)],
        );
    }
    for (server, path) in [(&host, "/host-models"), (&second_host, "/second-models")] {
        for request in server
            .received_requests()
            .await
            .expect("mock requests should be recorded")
        {
            assert_eq!(request.url.path(), path);
            assert_eq!(request.headers["authorization"], "Bearer host-catalog-key");
        }
    }
    assert_eq!(
        !cache
            .stored_entries
            .lock()
            .expect("stored entries lock should not be poisoned")
            .is_empty(),
        use_cache
    );
    assert!(!test.codex_home_path().join("models_cache.json").exists());
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn initialization_preserves_a_concurrent_refresh() -> Result<()> {
    let (store_entered, entered) = oneshot::channel();
    let (release_store, release) = oneshot::channel();
    let cache = Arc::new(TestModelsCache {
        entry: None,
        load_error: false,
        stored_entries: Mutex::new(Vec::new()),
        store_gate: Mutex::new(Some(StoreGate {
            entered: store_entered,
            release,
        })),
    });
    let mut model = remote_model("gpt-5.4");
    model.default_reasoning_summary = ReasoningSummary::Concise;
    let endpoint = TestModelsEndpoint::new(vec![model.clone()]);
    let manager = models_manager(cache, endpoint.clone());
    let initialization = tokio::spawn({
        let manager = manager.clone();
        async move {
            manager
                .ensure_initialized(codex_core::test_support::default_http_client_factory())
                .await;
        }
    });
    timeout(Duration::from_secs(/*secs*/ 10), entered).await??;

    // A normal refresh must proceed while the initial load is waiting on storage.
    model.default_reasoning_summary = ReasoningSummary::Detailed;
    *endpoint.models.lock().expect("model response lock") = vec![model.clone()];
    timeout(
        Duration::from_secs(/*secs*/ 10),
        manager.list_models(
            RefreshStrategy::Online,
            codex_core::test_support::default_http_client_factory(),
        ),
    )
    .await
    .expect("refresh must not wait for an initialization cache write");
    assert_eq!(manager.get_remote_models().await, vec![model.clone()]);

    release_store.send(()).expect("release initial cache write");
    timeout(Duration::from_secs(/*secs*/ 10), initialization).await??;
    assert_eq!(manager.get_remote_models().await, vec![model.clone()]);
    assert_eq!(
        manager.refreshed_model_catalog().as_deref(),
        Some(std::slice::from_ref(&model))
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn account_switch_during_cache_store_preserves_new_catalog_for_next_turn() -> Result<()> {
    let server = wiremock::MockServer::start().await;
    let home = Arc::new(TempDir::new()?);
    let initial_auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let auth = AuthManager::from_auth_for_testing_with_home(
        initial_auth.clone(),
        home.path().to_path_buf(),
    );
    let mut provider_info = built_in_model_providers(/*openai_base_url*/ None)["openai"].clone();
    provider_info.base_url = Some(format!("{}/v1", server.uri()));
    provider_info.supports_websockets = false;
    let cache = Arc::new(TestModelsCache {
        entry: None,
        load_error: false,
        stored_entries: Mutex::new(Vec::new()),
        store_gate: Mutex::new(None),
    });
    let mut old_model = remote_model("gpt-5.4");
    old_model.default_reasoning_summary = ReasoningSummary::Concise;
    let initial = responses::mount_models_once(
        &server,
        ModelsResponse {
            models: vec![old_model.clone()],
        },
    )
    .await;
    let test = test_codex()
        .with_home(home.clone())
        .with_auth_manager(Arc::clone(&auth))
        .with_model("gpt-5.4")
        .with_model_catalog_settings(ModelCatalogSettings::with_cache(cache.clone()))
        .with_config(move |config| {
            config.model_provider = provider_info;
            config.model_reasoning_summary = None;
        })
        .build_with_auto_env(&server)
        .await?;
    let manager = test.thread_manager.get_models_manager();
    assert_eq!(initial.requests().len(), 1);

    // Pause after the old catalog has been stored, before its refresh can publish it.
    let (store_entered, entered) = oneshot::channel();
    let (release_store, release) = oneshot::channel();
    *cache.store_gate.lock().unwrap() = Some(StoreGate {
        entered: store_entered,
        release,
    });
    let old_request = responses::mount_models_once(
        &server,
        ModelsResponse {
            models: vec![old_model],
        },
    )
    .await;
    let old_refresh = tokio::spawn({
        let manager = manager.clone();
        async move {
            manager
                .list_models(
                    RefreshStrategy::Online,
                    codex_core::test_support::default_http_client_factory(),
                )
                .await
        }
    });
    timeout(Duration::from_secs(10), entered).await??;

    let mut tokens = initial_auth.get_token_data()?;
    tokens.id_token.raw_jwt = format!(
        "e30.{}.signature",
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(br#"{"email":"second@example.com"}"#)
    );
    tokens.account_id = Some("second-account".into());
    tokens.access_token = "second-account-token".into();
    std::fs::write(
        home.path().join("auth.json"),
        serde_json::to_vec(&serde_json::json!({
            "auth_mode": "chatgpt", "tokens": tokens, "last_refresh": Utc::now(),
        }))?,
    )?;
    auth.reload().await;

    let mut new_model = remote_model("gpt-5.4");
    new_model.default_reasoning_summary = ReasoningSummary::Detailed;
    let new_request = responses::mount_models_once(
        &server,
        ModelsResponse {
            models: vec![new_model.clone()],
        },
    )
    .await;
    timeout(
        Duration::from_secs(/*secs*/ 10),
        manager.list_models(
            RefreshStrategy::Online,
            codex_core::test_support::default_http_client_factory(),
        ),
    )
    .await
    .expect("new account catalog must load while the old account's cache write is paused");
    assert_eq!(manager.get_remote_models().await, vec![new_model.clone()]);
    release_store.send(()).unwrap();
    timeout(Duration::from_secs(10), old_refresh).await??;
    assert_eq!(manager.get_remote_models().await, vec![new_model]);
    for (mock, account) in [(old_request, "account_id"), (new_request, "second-account")] {
        let requests = mock.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].headers["chatgpt-account-id"], account);
    }

    let response = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("next-turn"),
            ev_completed("next-turn"),
        ]),
    )
    .await;
    test.submit_turn("use the new account's model defaults")
        .await?;
    let request = response.single_request();
    assert_eq!(
        request.header("chatgpt-account-id").as_deref(),
        Some("second-account")
    );
    assert_eq!(request.body_json()["reasoning"]["summary"], "detailed");
    Ok(())
}
