use super::*;
use crate::ModelsManagerConfig;
use base64::Engine as _;
use chrono::Utc;
use codex_api::TransportError;
use codex_login::AuthCredentialsStoreMode;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider_info::WireApi;
use codex_protocol::config_types::ModelProviderAuthInfo;
use codex_protocol::openai_models::ModelsResponse;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::responses::mount_models_once;
use http::HeaderMap;
use http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::BTreeMap;
use std::num::NonZeroU64;
use std::sync::Arc;
use std::sync::Mutex;
use tempfile::TempDir;
use tempfile::tempdir;
use tracing::Event;
use tracing::Subscriber;
use tracing::field::Visit;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header_regex;
use wiremock::matchers::method;
use wiremock::matchers::path;

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

fn provider_for(base_url: String) -> ModelProviderInfo {
    ModelProviderInfo {
        name: "mock".into(),
        base_url: Some(base_url),
        env_key: None,
        env_key_instructions: None,
        experimental_bearer_token: None,
        auth: None,
        wire_api: WireApi::Responses,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: Some(0),
        stream_max_retries: Some(0),
        stream_idle_timeout_ms: Some(5_000),
        websocket_connect_timeout_ms: None,
        requires_openai_auth: false,
        supports_websockets: false,
    }
}

struct ProviderAuthScript {
    tempdir: TempDir,
    command: String,
    args: Vec<String>,
}

impl ProviderAuthScript {
    fn new(tokens: &[&str]) -> std::io::Result<Self> {
        let tempdir = tempfile::tempdir()?;
        let tokens_file = tempdir.path().join("tokens.txt");
        // `cmd.exe`'s `set /p` treats LF-only input as one line, so use CRLF on Windows.
        let token_line_ending = if cfg!(windows) { "\r\n" } else { "\n" };
        let mut token_file_contents = String::new();
        for token in tokens {
            token_file_contents.push_str(token);
            token_file_contents.push_str(token_line_ending);
        }
        std::fs::write(&tokens_file, token_file_contents)?;

        #[cfg(unix)]
        let (command, args) = {
            let script_path = tempdir.path().join("print-token.sh");
            std::fs::write(
                &script_path,
                r#"#!/bin/sh
first_line=$(sed -n '1p' tokens.txt)
printf '%s\n' "$first_line"
tail -n +2 tokens.txt > tokens.next
mv tokens.next tokens.txt
"#,
            )?;
            let mut permissions = std::fs::metadata(&script_path)?.permissions();
            {
                use std::os::unix::fs::PermissionsExt;
                permissions.set_mode(0o755);
            }
            std::fs::set_permissions(&script_path, permissions)?;
            ("./print-token.sh".to_string(), Vec::new())
        };

        #[cfg(windows)]
        let (command, args) = {
            let script_path = tempdir.path().join("print-token.cmd");
            std::fs::write(
                &script_path,
                r#"@echo off
setlocal EnableExtensions DisableDelayedExpansion
set "first_line="
<tokens.txt set /p "first_line="
if not defined first_line exit /b 1
setlocal EnableDelayedExpansion
echo(!first_line!
endlocal
more +1 tokens.txt > tokens.next
move /y tokens.next tokens.txt >nul
"#,
            )?;
            (
                "cmd.exe".to_string(),
                vec![
                    "/d".to_string(),
                    "/s".to_string(),
                    "/c".to_string(),
                    ".\\print-token.cmd".to_string(),
                ],
            )
        };

        Ok(Self {
            tempdir,
            command,
            args,
        })
    }

    fn auth_config(&self) -> ModelProviderAuthInfo {
        let timeout_ms = if cfg!(windows) {
            // Process startup can be slow on loaded Windows CI workers.
            10_000
        } else {
            2_000
        };
        ModelProviderAuthInfo {
            command: self.command.clone(),
            args: self.args.clone(),
            timeout_ms: NonZeroU64::new(timeout_ms).unwrap(),
            refresh_interval_ms: 60_000,
            cwd: match AbsolutePathBuf::try_from(self.tempdir.path()) {
                Ok(cwd) => cwd,
                Err(err) => panic!("tempdir should be absolute: {err}"),
            },
        }
    }
}

#[derive(Default)]
struct TagCollectorVisitor {
    tags: BTreeMap<String, String>,
}

impl Visit for TagCollectorVisitor {
    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.tags
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.tags
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.tags
            .insert(field.name().to_string(), format!("{value:?}"));
    }
}

#[derive(Clone)]
struct TagCollectorLayer {
    tags: Arc<Mutex<BTreeMap<String, String>>>,
}

impl<S> Layer<S> for TagCollectorLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        if event.metadata().target() != "feedback_tags" {
            return;
        }
        let mut visitor = TagCollectorVisitor::default();
        event.record(&mut visitor);
        self.tags.lock().unwrap().extend(visitor.tags);
    }
}

#[tokio::test]
async fn get_model_info_tracks_fallback_usage() {
    let codex_home = tempdir().expect("temp dir");
    let config = ModelsManagerConfig::default();
    let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("Test API Key"));
    let manager = ModelsManager::new(
        codex_home.path().to_path_buf(),
        auth_manager,
        /*model_catalog*/ None,
        CollaborationModesConfig::default(),
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
    let codex_home = tempdir().expect("temp dir");
    let config = ModelsManagerConfig::default();
    let mut overlay = remote_model("gpt-overlay", "Overlay", /*priority*/ 0);
    overlay.supports_image_detail_original = true;

    let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("Test API Key"));
    let manager = ModelsManager::new(
        codex_home.path().to_path_buf(),
        auth_manager,
        Some(ModelsResponse {
            models: vec![overlay],
        }),
        CollaborationModesConfig::default(),
    );

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
    let codex_home = tempdir().expect("temp dir");
    let config = ModelsManagerConfig::default();
    let mut remote = remote_model("gpt-image", "Image", /*priority*/ 0);
    remote.supports_image_detail_original = true;
    let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("Test API Key"));
    let manager = ModelsManager::new(
        codex_home.path().to_path_buf(),
        auth_manager,
        Some(ModelsResponse {
            models: vec![remote],
        }),
        CollaborationModesConfig::default(),
    );
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
    let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("Test API Key"));
    let manager = ModelsManager::new(
        codex_home.path().to_path_buf(),
        auth_manager,
        /*model_catalog*/ None,
        CollaborationModesConfig::default(),
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
    let server = MockServer::start().await;
    let remote_models = vec![
        remote_model("priority-low", "Low", /*priority*/ 1),
        remote_model("priority-high", "High", /*priority*/ 0),
    ];
    let models_mock = mount_models_once(
        &server,
        ModelsResponse {
            models: remote_models.clone(),
        },
    )
    .await;

    let codex_home = tempdir().expect("temp dir");
    let auth_manager =
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let provider = provider_for(server.uri());
    let manager = ModelsManager::with_provider_for_tests(
        codex_home.path().to_path_buf(),
        auth_manager,
        provider,
    );

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
    assert_eq!(
        models_mock.requests().len(),
        1,
        "expected a single /models request"
    );
}

#[tokio::test]
async fn refresh_available_models_uses_provider_auth_token() {
    let server = MockServer::start().await;
    let auth_script = ProviderAuthScript::new(&["provider-token"]).unwrap();
    let remote_models = vec![remote_model(
        "provider-model",
        "Provider",
        /*priority*/ 0,
    )];

    Mock::given(method("GET"))
        .and(path("/models"))
        .and(header_regex("Authorization", "Bearer provider-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_json(ModelsResponse {
                    models: remote_models.clone(),
                }),
        )
        .expect(1)
        .mount(&server)
        .await;

    let codex_home = tempdir().expect("temp dir");
    let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("unused"));
    let provider = ModelProviderInfo {
        auth: Some(auth_script.auth_config()),
        ..provider_for(server.uri())
    };
    let manager = ModelsManager::with_provider_for_tests(
        codex_home.path().to_path_buf(),
        auth_manager,
        provider,
    );

    manager
        .refresh_available_models(RefreshStrategy::Online)
        .await
        .expect("refresh succeeds");

    assert_models_contain(&manager.get_remote_models().await, &remote_models);
}

#[tokio::test]
async fn refresh_available_models_uses_cache_when_fresh() {
    let server = MockServer::start().await;
    let remote_models = vec![remote_model("cached", "Cached", /*priority*/ 5)];
    let models_mock = mount_models_once(
        &server,
        ModelsResponse {
            models: remote_models.clone(),
        },
    )
    .await;

    let codex_home = tempdir().expect("temp dir");
    let auth_manager =
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let provider = provider_for(server.uri());
    let manager = ModelsManager::with_provider_for_tests(
        codex_home.path().to_path_buf(),
        auth_manager,
        provider,
    );

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
        models_mock.requests().len(),
        1,
        "cache hit should avoid a second /models request"
    );
}

#[tokio::test]
async fn refresh_available_models_refetches_when_cache_stale() {
    let server = MockServer::start().await;
    let initial_models = vec![remote_model("stale", "Stale", /*priority*/ 1)];
    let initial_mock = mount_models_once(
        &server,
        ModelsResponse {
            models: initial_models.clone(),
        },
    )
    .await;

    let codex_home = tempdir().expect("temp dir");
    let auth_manager =
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let provider = provider_for(server.uri());
    let manager = ModelsManager::with_provider_for_tests(
        codex_home.path().to_path_buf(),
        auth_manager,
        provider,
    );

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

    let updated_models = vec![remote_model("fresh", "Fresh", /*priority*/ 9)];
    server.reset().await;
    let refreshed_mock = mount_models_once(
        &server,
        ModelsResponse {
            models: updated_models.clone(),
        },
    )
    .await;

    manager
        .refresh_available_models(RefreshStrategy::OnlineIfUncached)
        .await
        .expect("second refresh succeeds");
    assert_models_contain(&manager.get_remote_models().await, &updated_models);
    assert_eq!(
        initial_mock.requests().len(),
        1,
        "initial refresh should only hit /models once"
    );
    assert_eq!(
        refreshed_mock.requests().len(),
        1,
        "stale cache refresh should fetch /models once"
    );
}

#[tokio::test]
async fn refresh_available_models_refetches_when_version_mismatch() {
    let server = MockServer::start().await;
    let initial_models = vec![remote_model("old", "Old", /*priority*/ 1)];
    let initial_mock = mount_models_once(
        &server,
        ModelsResponse {
            models: initial_models.clone(),
        },
    )
    .await;

    let codex_home = tempdir().expect("temp dir");
    let auth_manager =
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let provider = provider_for(server.uri());
    let manager = ModelsManager::with_provider_for_tests(
        codex_home.path().to_path_buf(),
        auth_manager,
        provider,
    );

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

    let updated_models = vec![remote_model("new", "New", /*priority*/ 2)];
    server.reset().await;
    let refreshed_mock = mount_models_once(
        &server,
        ModelsResponse {
            models: updated_models.clone(),
        },
    )
    .await;

    manager
        .refresh_available_models(RefreshStrategy::OnlineIfUncached)
        .await
        .expect("second refresh succeeds");
    assert_models_contain(&manager.get_remote_models().await, &updated_models);
    assert_eq!(
        initial_mock.requests().len(),
        1,
        "initial refresh should only hit /models once"
    );
    assert_eq!(
        refreshed_mock.requests().len(),
        1,
        "version mismatch should fetch /models once"
    );
}

#[tokio::test]
async fn refresh_available_models_drops_removed_remote_models() {
    let server = MockServer::start().await;
    let initial_models = vec![remote_model(
        "remote-old",
        "Remote Old",
        /*priority*/ 1,
    )];
    let initial_mock = mount_models_once(
        &server,
        ModelsResponse {
            models: initial_models,
        },
    )
    .await;

    let codex_home = tempdir().expect("temp dir");
    let auth_manager =
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let provider = provider_for(server.uri());
    let mut manager = ModelsManager::with_provider_for_tests(
        codex_home.path().to_path_buf(),
        auth_manager,
        provider,
    );
    manager.cache_manager.set_ttl(Duration::ZERO);

    manager
        .refresh_available_models(RefreshStrategy::OnlineIfUncached)
        .await
        .expect("initial refresh succeeds");

    server.reset().await;
    let refreshed_models = vec![remote_model(
        "remote-new",
        "Remote New",
        /*priority*/ 1,
    )];
    let refreshed_mock = mount_models_once(
        &server,
        ModelsResponse {
            models: refreshed_models,
        },
    )
    .await;

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
        initial_mock.requests().len(),
        1,
        "initial refresh should only hit /models once"
    );
    assert_eq!(
        refreshed_mock.requests().len(),
        1,
        "second refresh should only hit /models once"
    );
}

#[tokio::test]
async fn refresh_available_models_skips_network_without_chatgpt_auth() {
    let server = MockServer::start().await;
    let dynamic_slug = "dynamic-model-only-for-test-noauth";
    let models_mock = mount_models_once(
        &server,
        ModelsResponse {
            models: vec![remote_model(dynamic_slug, "No Auth", /*priority*/ 1)],
        },
    )
    .await;

    let codex_home = tempdir().expect("temp dir");
    let auth_manager = Arc::new(AuthManager::new(
        codex_home.path().to_path_buf(),
        /*enable_codex_api_key_env*/ false,
        AuthCredentialsStoreMode::File,
    ));
    let provider = provider_for(server.uri());
    let manager = ModelsManager::with_provider_for_tests(
        codex_home.path().to_path_buf(),
        auth_manager,
        provider,
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
        models_mock.requests().len(),
        0,
        "no auth should avoid /models requests"
    );
}

#[test]
fn models_request_telemetry_emits_auth_env_feedback_tags_on_failure() {
    let tags = Arc::new(Mutex::new(BTreeMap::new()));
    let _guard = tracing_subscriber::registry()
        .with(TagCollectorLayer { tags: tags.clone() })
        .set_default();

    let telemetry = ModelsRequestTelemetry {
        auth_mode: Some(TelemetryAuthMode::Chatgpt.to_string()),
        auth_header_attached: true,
        auth_header_name: Some("authorization"),
        auth_env: codex_login::AuthEnvTelemetry {
            openai_api_key_env_present: false,
            codex_api_key_env_present: false,
            codex_api_key_env_enabled: false,
            provider_env_key_name: Some("configured".to_string()),
            provider_env_key_present: Some(false),
            refresh_token_url_override_present: false,
        },
    };
    let mut headers = HeaderMap::new();
    headers.insert("x-request-id", "req-models-401".parse().unwrap());
    headers.insert("cf-ray", "ray-models-401".parse().unwrap());
    headers.insert(
        "x-openai-authorization-error",
        "missing_authorization_header".parse().unwrap(),
    );
    headers.insert(
        "x-error-json",
        base64::engine::general_purpose::STANDARD
            .encode(r#"{"error":{"code":"token_expired"}}"#)
            .parse()
            .unwrap(),
    );
    telemetry.on_request(
        /*attempt*/ 1,
        Some(StatusCode::UNAUTHORIZED),
        Some(&TransportError::Http {
            status: StatusCode::UNAUTHORIZED,
            url: Some("https://example.test/models".to_string()),
            headers: Some(headers),
            body: Some("plain text error".to_string()),
        }),
        Duration::from_millis(17),
    );

    let tags = tags.lock().unwrap().clone();
    assert_eq!(
        tags.get("endpoint").map(String::as_str),
        Some("\"/models\"")
    );
    assert_eq!(
        tags.get("auth_mode").map(String::as_str),
        Some("\"Chatgpt\"")
    );
    assert_eq!(
        tags.get("auth_request_id").map(String::as_str),
        Some("\"req-models-401\"")
    );
    assert_eq!(
        tags.get("auth_error").map(String::as_str),
        Some("\"missing_authorization_header\"")
    );
    assert_eq!(
        tags.get("auth_error_code").map(String::as_str),
        Some("\"token_expired\"")
    );
    assert_eq!(
        tags.get("auth_env_openai_api_key_present")
            .map(String::as_str),
        Some("false")
    );
    assert_eq!(
        tags.get("auth_env_codex_api_key_present")
            .map(String::as_str),
        Some("false")
    );
    assert_eq!(
        tags.get("auth_env_codex_api_key_enabled")
            .map(String::as_str),
        Some("false")
    );
    assert_eq!(
        tags.get("auth_env_provider_key_name").map(String::as_str),
        Some("\"configured\"")
    );
    assert_eq!(
        tags.get("auth_env_provider_key_present")
            .map(String::as_str),
        Some("\"false\"")
    );
    assert_eq!(
        tags.get("auth_env_refresh_token_url_override_present")
            .map(String::as_str),
        Some("false")
    );
}

#[test]
fn build_available_models_picks_default_after_hiding_hidden_models() {
    let codex_home = tempdir().expect("temp dir");
    let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("Test API Key"));
    let provider = provider_for("http://example.test".to_string());
    let manager = ModelsManager::with_provider_for_tests(
        codex_home.path().to_path_buf(),
        auth_manager,
        provider,
    );

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
