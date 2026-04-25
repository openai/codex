#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Result;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_models_manager::manager::RefreshStrategy;
use codex_models_manager::manager::SharedModelsManager;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::openai_models::default_input_modalities;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_models_once;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use serde_json::Value;
use std::time::Duration;
use std::time::Instant;
use tokio::time::sleep;

const SPAWN_AGENT_TOOL_NAME: &str = "spawn_agent";

fn spawn_agent_description(body: &Value) -> Option<String> {
    body.get("tools")
        .and_then(Value::as_array)
        .and_then(|tools| {
            tools.iter().find_map(|tool| {
                if tool.get("name").and_then(Value::as_str) == Some(SPAWN_AGENT_TOOL_NAME) {
                    tool.get("description")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                } else {
                    None
                }
            })
        })
}

fn test_model_info(
    slug: &str,
    display_name: &str,
    description: &str,
    visibility: ModelVisibility,
    default_reasoning_level: ReasoningEffort,
    supported_reasoning_levels: Vec<ReasoningEffortPreset>,
) -> ModelInfo {
    ModelInfo {
        slug: slug.to_string(),
        display_name: display_name.to_string(),
        description: Some(description.to_string()),
        default_reasoning_level: Some(default_reasoning_level),
        supported_reasoning_levels,
        shell_type: ConfigShellToolType::ShellCommand,
        visibility,
        supported_in_api: true,
        input_modalities: default_input_modalities(),
        used_fallback_model_metadata: false,
        supports_search_tool: false,
        priority: 1,
        additional_speed_tiers: Vec::new(),
        upgrade: None,
        base_instructions: "base instructions".to_string(),
        model_messages: None,
        supports_reasoning_summaries: false,
        default_reasoning_summary: ReasoningSummary::Auto,
        support_verbosity: false,
        default_verbosity: None,
        availability_nux: None,
        apply_patch_tool_type: None,
        web_search_tool_type: Default::default(),
        truncation_policy: TruncationPolicyConfig::bytes(/*limit*/ 10_000),
        supports_parallel_tool_calls: false,
        supports_image_detail_original: false,
        context_window: Some(272_000),
        max_context_window: None,
        auto_compact_token_limit: None,
        effective_context_window_percent: 95,
        experimental_supported_tools: Vec::new(),
    }
}

async fn wait_for_model_available(manager: &SharedModelsManager, slug: &str) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let available_models = manager.list_models(RefreshStrategy::Online).await;
        if available_models.iter().any(|model| model.model == slug) {
            return;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for remote model {slug} to appear");
        }
        sleep(Duration::from_millis(25)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawn_agent_description_lists_visible_models_and_reasoning_efforts() -> Result<()> {
    let server = start_mock_server().await;
    mount_models_once(
        &server,
        ModelsResponse {
            models: vec![
                test_model_info(
                    "visible-model",
                    "Visible Model",
                    "Fast and capable",
                    ModelVisibility::List,
                    ReasoningEffort::Medium,
                    vec![
                        ReasoningEffortPreset {
                            effort: ReasoningEffort::Low,
                            description: "Quick scan".to_string(),
                        },
                        ReasoningEffortPreset {
                            effort: ReasoningEffort::High,
                            description: "Deep dive".to_string(),
                        },
                    ],
                ),
                test_model_info(
                    "hidden-model",
                    "Hidden Model",
                    "Should not be shown",
                    ModelVisibility::Hide,
                    ReasoningEffort::Low,
                    vec![ReasoningEffortPreset {
                        effort: ReasoningEffort::Low,
                        description: "Not visible".to_string(),
                    }],
                ),
            ],
        },
    )
    .await;
    let resp_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp1"), ev_completed("resp1")]),
    )
    .await;

    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model("visible-model")
        .with_config(|config| {
            config
                .features
                .enable(Feature::Collab)
                .expect("test config should allow feature update");
        });
    let test = builder.build(&server).await?;
    wait_for_model_available(&test.thread_manager.get_models_manager(), "visible-model").await;

    test.submit_turn("hello").await?;

    let body = resp_mock.single_request().body_json();
    let description =
        spawn_agent_description(&body).expect("spawn_agent description should be present");

    assert!(
        description.contains("- Visible Model (`visible-model`): Fast and capable"),
        "expected visible model summary in spawn_agent description: {description:?}"
    );
    assert!(
        description
            .contains("Available model overrides (optional; inherited parent model is preferred):"),
        "expected model choices to be framed as overrides in spawn_agent description: {description:?}"
    );
    assert!(
        description.contains(
            "Spawned agents inherit your current model by default. Omit `model` to use that preferred default; set `model` only when an explicit override is needed."
        ),
        "expected inherited-model guidance in spawn_agent description: {description:?}"
    );
    assert!(
        description.contains(
            "Do not set the `model` field unless the user explicitly asks for a different model or there is a clear task-specific reason."
        ),
        "expected model override usage guidance in spawn_agent description: {description:?}"
    );
    assert!(
        description.contains("Default reasoning effort: medium."),
        "expected default reasoning effort in spawn_agent description: {description:?}"
    );
    assert!(
        description.contains("low (Quick scan), high (Deep dive)."),
        "expected reasoning efforts in spawn_agent description: {description:?}"
    );
    assert!(
        !description.contains("Hidden Model"),
        "hidden picker model should be omitted from spawn_agent description: {description:?}"
    );
    assert!(
        description.contains(
            "Only use `spawn_agent` if and only if the user explicitly asks for sub-agents, delegation, or parallel agent work."
        ),
        "expected explicit authorization rule in spawn_agent description: {description:?}"
    );
    assert!(
        description.contains(
            "Requests for depth, thoroughness, research, investigation, or detailed codebase analysis do not count as permission to spawn."
        ),
        "expected non-authorization clarification in spawn_agent description: {description:?}"
    );
    assert!(
        description.contains(
            "Agent-role guidance below only helps choose which agent to use after spawning is already authorized; it never authorizes spawning by itself."
        ),
        "expected agent-role clarification in spawn_agent description: {description:?}"
    );
    assert!(
        !description.contains("A mini model can solve many tasks faster than the main model."),
        "spawn_agent description should not encourage choosing a smaller model by default: {description:?}"
    );

    Ok(())
}
