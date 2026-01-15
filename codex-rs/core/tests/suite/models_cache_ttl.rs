use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use chrono::DateTime;
use chrono::TimeZone;
use chrono::Utc;
use codex_core::CodexAuth;
use codex_core::features::Feature;
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
        config.features.enable(Feature::RemoteModels);
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
        .list_models(&config, RefreshStrategy::OnlineIfUncached)
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
        .list_models(&config, RefreshStrategy::Offline)
        .await;
    assert!(
        offline_models
            .iter()
            .any(|preset| preset.model == REMOTE_MODEL),
        "offline listing should use renewed cache"
    );

    Ok(())
}

async fn rewrite_cache_timestamp(path: &Path, fetched_at: DateTime<Utc>) -> Result<()> {
    let mut cache = read_cache(path).await?;
    cache.fetched_at = fetched_at;
    let contents = serde_json::to_vec_pretty(&cache)?;
    tokio::fs::write(path, contents).await?;
    Ok(())
}

async fn read_cache(path: &Path) -> Result<ModelsCache> {
    let contents = tokio::fs::read(path).await?;
    let cache = serde_json::from_slice(&contents)?;
    Ok(cache)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelsCache {
    fetched_at: DateTime<Utc>,
    #[serde(default)]
    etag: Option<String>,
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
    }
}
