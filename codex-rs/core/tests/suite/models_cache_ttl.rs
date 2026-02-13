use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use chrono::DateTime;
use chrono::TimeZone;
use chrono::Utc;
use codex_core::CodexAuth;
use codex_core::models_manager::manager::RefreshStrategy;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::openai_models::default_input_modalities;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::responses::sse_response;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde::Deserialize;
use serde::Serialize;
use wiremock::MockServer;

const ETAG: &str = "\"models-etag-ttl\"";
const CACHE_FILE: &str = "models_cache.json";
const REMOTE_MODEL: &str = "codex-test-ttl";
const VERSIONED_MODEL: &str = "codex-test-versioned";
const MISSING_VERSION_MODEL: &str = "codex-test-missing-version";
const DIFFERENT_VERSION_MODEL: &str = "codex-test-different-version";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn renews_cache_ttl_on_matching_models_etag() -> Result<()> {
    let server = MockServer::start().await;

    let remote_model = test_remote_model(REMOTE_MODEL, 1);
    let models_mock = responses::mount_models_once_with_etag(
        &server,
        ModelsResponse {
            models: vec![remote_model.clone()],
        },
        ETAG,
    )
    .await;

    let mut builder = test_codex().with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    builder = builder.with_config(|config| {
        config.model = Some("gpt-5".to_string());
        config.model_provider.request_max_retries = Some(0);
        config.model_provider.stream_max_retries = Some(1);
    });

    let test = builder.build(&server).await?;
    let codex = Arc::clone(&test.codex);
    let config = test.config.clone();

    // Populate cache via initial refresh.
    let models_manager = test.thread_manager.get_models_manager();
    let _ = models_manager
        .list_models(RefreshStrategy::OnlineIfUncached)
        .await;

    let cache_path = config.codex_home.join(CACHE_FILE);
    let stale_time = Utc.timestamp_opt(0, 0).single().expect("valid epoch");
    rewrite_cache_timestamp(&cache_path, stale_time).await?;

    // Trigger responses with matching ETag, which should renew the cache TTL without another /models.
    let response_body = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-1"),
    ]);
    let _responses_mock = responses::mount_response_once(
        &server,
        sse_response(response_body).insert_header("X-Models-Etag", ETAG),
    )
    .await;

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "hi".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd_path().to_path_buf(),
            approval_policy: codex_core::protocol::AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: test.session_configured.model.clone(),
            effort: None,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    let _ = wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let refreshed_cache = read_cache(&cache_path).await?;
    assert!(
        refreshed_cache.fetched_at > stale_time,
        "cache TTL should be renewed"
    );
    assert_eq!(
        models_mock.requests().len(),
        1,
        "/models should not refetch on matching etag"
    );

    // Cached models remain usable offline.
    let offline_models = test
        .thread_manager
        .list_models(RefreshStrategy::Offline)
        .await;
    assert!(
        offline_models
            .iter()
            .any(|preset| preset.model == REMOTE_MODEL),
        "offline listing should use renewed cache"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn uses_cache_when_version_matches() -> Result<()> {
    let server = MockServer::start().await;
    let cached_model = test_remote_model(VERSIONED_MODEL, 1);
    let models_mock = responses::mount_models_once(
        &server,
        ModelsResponse {
            models: vec![test_remote_model("remote", 2)],
        },
    )
    .await;

    let mut builder = test_codex().with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    builder = builder
        .with_pre_build_hook(move |home| {
            let cache = ModelsCache {
                fetched_at: Utc::now(),
                etag: None,
                client_version: Some(codex_core::models_manager::client_version_to_whole()),
                models: vec![cached_model],
            };
            let cache_path = home.join(CACHE_FILE);
            write_cache_sync(&cache_path, &cache).expect("write cache");
        })
        .with_config(|config| {
            config.model_provider.request_max_retries = Some(0);
        });

    let test = builder.build(&server).await?;
    let models_manager = test.thread_manager.get_models_manager();
    let models = models_manager
        .list_models(RefreshStrategy::OnlineIfUncached)
        .await;

    assert!(
        models.iter().any(|preset| preset.model == VERSIONED_MODEL),
        "expected cached model"
    );
    assert_eq!(
        models_mock.requests().len(),
        0,
        "/models should not be called when cache version matches"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refreshes_when_cache_version_missing() -> Result<()> {
    let server = MockServer::start().await;
    let cached_model = test_remote_model(MISSING_VERSION_MODEL, 1);
    let models_mock = responses::mount_models_once(
        &server,
        ModelsResponse {
            models: vec![test_remote_model("remote-missing", 2)],
        },
    )
    .await;

    let mut builder = test_codex().with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    builder = builder
        .with_pre_build_hook(move |home| {
            let cache = ModelsCache {
                fetched_at: Utc::now(),
                etag: None,
                client_version: None,
                models: vec![cached_model],
            };
            let cache_path = home.join(CACHE_FILE);
            write_cache_sync(&cache_path, &cache).expect("write cache");
        })
        .with_config(|config| {
            config.model_provider.request_max_retries = Some(0);
        });

    let test = builder.build(&server).await?;
    let models_manager = test.thread_manager.get_models_manager();
    let models = models_manager
        .list_models(RefreshStrategy::OnlineIfUncached)
        .await;

    assert!(
        models.iter().any(|preset| preset.model == "remote-missing"),
        "expected refreshed models"
    );
    assert_eq!(
        models_mock.requests().len(),
        1,
        "/models should be called when cache version is missing"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refreshes_when_cache_version_differs() -> Result<()> {
    let server = MockServer::start().await;
    let cached_model = test_remote_model(DIFFERENT_VERSION_MODEL, 1);
    let models_response = ModelsResponse {
        models: vec![test_remote_model("remote-different", 2)],
    };
    let mut models_mocks = Vec::new();
    for _ in 0..3 {
        models_mocks.push(responses::mount_models_once(&server, models_response.clone()).await);
    }

    let mut builder = test_codex().with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    builder = builder
        .with_pre_build_hook(move |home| {
            let client_version = codex_core::models_manager::client_version_to_whole();
            let cache = ModelsCache {
                fetched_at: Utc::now(),
                etag: None,
                client_version: Some(format!("{client_version}-diff")),
                models: vec![cached_model],
            };
            let cache_path = home.join(CACHE_FILE);
            write_cache_sync(&cache_path, &cache).expect("write cache");
        })
        .with_config(|config| {
            config.model_provider.request_max_retries = Some(0);
        });

    let test = builder.build(&server).await?;
    let models_manager = test.thread_manager.get_models_manager();
    let models = models_manager
        .list_models(RefreshStrategy::OnlineIfUncached)
        .await;

    assert!(
        models
            .iter()
            .any(|preset| preset.model == "remote-different"),
        "expected refreshed models"
    );
    let models_request_count: usize = models_mocks.iter().map(|mock| mock.requests().len()).sum();
    assert!(
        models_request_count >= 1,
        "/models should be called when cache version differs"
    );

    Ok(())
}

async fn rewrite_cache_timestamp(path: &Path, fetched_at: DateTime<Utc>) -> Result<()> {
    let mut cache = read_cache(path).await?;
    cache.fetched_at = fetched_at;
    write_cache(path, &cache).await?;
    Ok(())
}

async fn read_cache(path: &Path) -> Result<ModelsCache> {
    let contents = tokio::fs::read(path).await?;
    let cache = serde_json::from_slice(&contents)?;
    Ok(cache)
}

async fn write_cache(path: &Path, cache: &ModelsCache) -> Result<()> {
    let contents = serde_json::to_vec_pretty(cache)?;
    tokio::fs::write(path, contents).await?;
    Ok(())
}

fn write_cache_sync(path: &Path, cache: &ModelsCache) -> Result<()> {
    let contents = serde_json::to_vec_pretty(cache)?;
    std::fs::write(path, contents)?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelsCache {
    fetched_at: DateTime<Utc>,
    #[serde(default)]
    etag: Option<String>,
    #[serde(default)]
    client_version: Option<String>,
    models: Vec<ModelInfo>,
}

fn test_remote_model(slug: &str, priority: i32) -> ModelInfo {
    ModelInfo {
        slug: slug.to_string(),
        display_name: "Remote Test".to_string(),
        description: Some("remote model".to_string()),
        default_reasoning_level: Some(ReasoningEffort::Medium),
        supported_reasoning_levels: vec![
            ReasoningEffortPreset {
                effort: ReasoningEffort::Low,
                description: "low".to_string(),
            },
            ReasoningEffortPreset {
                effort: ReasoningEffort::Medium,
                description: "medium".to_string(),
            },
        ],
        shell_type: ConfigShellToolType::ShellCommand,
        visibility: ModelVisibility::List,
        supported_in_api: true,
        priority,
        upgrade: None,
        base_instructions: "base instructions".to_string(),
        model_messages: None,
        supports_reasoning_summaries: false,
        support_verbosity: false,
        default_verbosity: None,
        apply_patch_tool_type: None,
        truncation_policy: TruncationPolicyConfig::bytes(10_000),
        supports_parallel_tool_calls: false,
        context_window: Some(272_000),
        auto_compact_token_limit: None,
        effective_context_window_percent: 95,
        experimental_supported_tools: Vec::new(),
        input_modalities: default_input_modalities(),
        prefer_websockets: false,
        used_fallback_model_metadata: false,
    }
}
