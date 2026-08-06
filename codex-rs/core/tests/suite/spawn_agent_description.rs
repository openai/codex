#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used)]

use anyhow::Result;
use codex_core::config::AgentRoleConfig;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_models_manager::manager::RefreshStrategy;
use codex_models_manager::manager::SharedModelsManager;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelServiceTier;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::openai_models::default_input_modalities;
use codex_protocol::protocol::MULTI_AGENT_MODE_OPEN_TAG;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_models_once;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::namespace_child_tool;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use serde_json::Value;
use std::time::Duration;
use std::time::Instant;
use test_case::test_case;
use tokio::time::sleep;

const MULTI_AGENT_V1_NAMESPACE: &str = "multi_agent_v1";
const MULTI_AGENT_V2_NAMESPACE: &str = "collaboration";
const SPAWN_AGENT_TOOL_NAME: &str = "spawn_agent";

fn spawn_agent_description(body: &Value) -> Option<String> {
    namespace_child_tool(body, MULTI_AGENT_V1_NAMESPACE, SPAWN_AGENT_TOOL_NAME)
        .and_then(|tool| tool.get("description"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn spawn_agent_exposes_agent_type(body: &Value, namespace: &str) -> bool {
    namespace_child_tool(body, namespace, SPAWN_AGENT_TOOL_NAME)
        .and_then(|tool| tool.pointer("/parameters/properties/agent_type"))
        .is_some()
}

fn test_model_info(
    slug: &str,
    display_name: &str,
    description: &str,
    visibility: ModelVisibility,
    default_reasoning_level: ReasoningEffort,
    supported_reasoning_levels: Vec<ReasoningEffortPreset>,
    service_tiers: Vec<ModelServiceTier>,
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
        use_responses_lite: false,
        auto_review_model_override: None,
        model_specialty: None,
        tool_mode: None,
        multi_agent_version: None,
        priority: 1,
        additional_speed_tiers: Vec::new(),
        service_tiers,
        default_service_tier: None,
        upgrade: None,
        model_messages: None,
        include_skills_usage_instructions: false,
        include_plugin_usage_instructions: false,
        include_apps_usage_instructions: false,
        supports_reasoning_summary_parameter: true,
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
        comp_hash: None,
        effective_context_window_percent: 95,
        experimental_supported_tools: Vec::new(),
    }
}

async fn wait_for_model_available(manager: &SharedModelsManager, slug: &str) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let available_models = manager
            .list_models(
                RefreshStrategy::Online,
                codex_core::test_support::default_http_client_factory(),
            )
            .await;
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
                            effort: ReasoningEffort::Medium,
                            description: "Balanced".to_string(),
                        },
                        ReasoningEffortPreset {
                            effort: ReasoningEffort::High,
                            description: "Deep dive".to_string(),
                        },
                    ],
                    vec![ModelServiceTier {
                        id: "priority".to_string(),
                        name: "Fast".to_string(),
                        description: "1.5x speed, increased usage".to_string(),
                    }],
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
                    Vec::new(),
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
            config.multi_agent_v2.hide_spawn_agent_metadata = false;
        });
    let test = builder.build(&server).await?;
    wait_for_model_available(&test.thread_manager.get_models_manager(), "visible-model").await;

    test.submit_turn("hello").await?;

    let body = resp_mock.single_request().body_json();
    let description =
        spawn_agent_description(&body).expect("spawn_agent description should be present");

    assert!(
        description.contains("- `visible-model`: Fast and capable"),
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
        description.contains("Reasoning efforts: low, medium (default), high."),
        "expected default reasoning effort in spawn_agent description: {description:?}"
    );
    assert!(
        description.contains("Service tiers: priority."),
        "expected service tier guidance in spawn_agent description: {description:?}"
    );
    assert!(
        !description.contains("hidden-model"),
        "hidden picker model should be omitted from spawn_agent description: {description:?}"
    );
    assert!(
        description.contains(
            "Do not spawn sub-agents unless the user or applicable AGENTS.md/skill instructions explicitly ask for sub-agents, delegation, or parallel agent work."
        ),
        "expected explicit authorization rule in spawn_agent description: {description:?}"
    );
    assert!(
        description.contains(
            "Requests for depth, thoroughness, research, investigation, or detailed codebase analysis do not count as permission to spawn."
        ) && description.contains("### When to delegate vs. do the subtask yourself"),
        "expected delegation decision guidance in spawn_agent description: {description:?}"
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

#[test_case(false, false, MULTI_AGENT_V1_NAMESPACE; "v1 hides agent type without roles")]
#[test_case(true, true, "collaboration"; "v2 exposes agent type with a role")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn configured_agent_roles_control_spawn_agent_type(
    multi_agent_v2: bool,
    has_agent_role: bool,
    namespace: &str,
) -> Result<()> {
    let server = start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp1"), ev_completed("resp1")]),
    )
    .await;
    let test = test_codex()
        .with_config(move |config| {
            config
                .features
                .enable(Feature::Collab)
                .expect("test config should allow feature update");
            if multi_agent_v2 {
                config
                    .features
                    .enable(Feature::MultiAgentV2)
                    .expect("test config should allow feature update");
            } else {
                config
                    .features
                    .disable(Feature::MultiAgentV2)
                    .expect("test config should allow feature update");
            }
            if has_agent_role {
                config.agent_roles.insert(
                    "researcher".to_string(),
                    AgentRoleConfig {
                        description: Some("Research role".to_string()),
                        config_file: None,
                        nickname_candidates: None,
                    },
                );
            }
        })
        .build_with_auto_env(&server)
        .await?;

    test.submit_turn("hello").await?;

    assert_eq!(
        spawn_agent_exposes_agent_type(&response.single_request().body_json(), namespace),
        has_agent_role
    );
    Ok(())
}

/// Wait guidance belongs to overridable developer instructions, never the tool schema.
#[test_case(None, true; "default developer instructions include wait guidance")]
#[test_case(Some("Custom root instructions."), false; "custom developer instructions replace wait guidance")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn multi_agent_v2_wait_guidance_uses_overridable_developer_instructions(
    root_agent_usage_hint_text: Option<&str>,
    expected_wait_guidance: bool,
) -> Result<()> {
    let root_agent_usage_hint_text = root_agent_usage_hint_text.map(str::to_string);
    let server = start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp1"), ev_completed("resp1")]),
    )
    .await;
    let test = test_codex()
        .with_config(move |config| {
            config
                .features
                .enable(Feature::MultiAgentV2)
                .expect("test config should allow feature update");
            if let Some(root_agent_usage_hint_text) = root_agent_usage_hint_text {
                config.multi_agent_v2.root_agent_usage_hint_text = Some(root_agent_usage_hint_text);
            }
        })
        .build_with_auto_env(&server)
        .await?;

    test.submit_turn("hello").await?;

    let request = response.single_request();
    let developer_messages = request.message_input_texts("developer");
    let has_wait_guidance = developer_messages.iter().any(|message| {
        message.contains(
            "When calling `wait_agent`, prefer longer waits (minutes) to avoid busy polling.",
        )
    });
    assert_eq!(has_wait_guidance, expected_wait_guidance);

    let body = request.body_json();
    let wait_agent_tool = namespace_child_tool(&body, MULTI_AGENT_V2_NAMESPACE, "wait_agent")
        .expect("wait_agent should be exposed");
    assert_eq!(
        wait_agent_tool
            .pointer("/parameters/properties/timeout_ms/description")
            .and_then(Value::as_str),
        Some("Timeout in milliseconds. Defaults to 30000, min 10000, max 3600000.")
    );

    Ok(())
}

/// Resumed legacy threads receive current usage hints once, before their active mode.
/// Configured hint overrides and disabled wait tools must retain their existing semantics.
#[test_case(true, None, true; "legacy resume restores default wait guidance")]
#[test_case(true, Some("Custom root instructions."), false; "legacy resume preserves custom usage hints")]
#[test_case(true, Some("Legacy root instructions."), false; "legacy resume preserves unchanged custom usage hints")]
#[test_case(false, None, false; "legacy resume omits disabled wait guidance")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn multi_agent_v2_cold_resume_refreshes_legacy_usage_hints_once(
    wait_agent_enabled: bool,
    resumed_root_agent_usage_hint_text: Option<&str>,
    expected_wait_guidance: bool,
) -> Result<()> {
    let resumed_root_agent_usage_hint_text = resumed_root_agent_usage_hint_text.map(str::to_string);
    let legacy_root_agent_usage_hint_text = "Legacy root instructions.";
    let wait_guidance =
        "When calling `wait_agent`, prefer longer waits (minutes) to avoid busy polling.";
    let config_toml = format!(
        "[features.multi_agent_v2]\nenabled = true\nwait_agent_enabled = {wait_agent_enabled}\n"
    );
    let server = start_mock_server().await;
    let initial_response = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-initial"),
            ev_completed("resp-initial"),
        ]),
    )
    .await;
    let initial = test_codex()
        .with_pre_build_hook(move |home| {
            std::fs::write(home.join("config.toml"), &config_toml)
                .expect("write multi-agent configuration");
        })
        .with_config(move |config| {
            config.multi_agent_v2.root_agent_usage_hint_text =
                Some(legacy_root_agent_usage_hint_text.to_string());
        })
        .build_with_auto_env(&server)
        .await?;

    initial.submit_turn("before upgrading").await?;

    let initial_request = initial_response.single_request();
    assert!(
        initial_request
            .message_input_text_groups("developer")
            .iter()
            .any(|group| { group.len() == 1 && group[0] == legacy_root_agent_usage_hint_text }),
        "legacy usage hint should be its own developer message"
    );
    assert!(
        !initial_request
            .message_input_texts("developer")
            .iter()
            .any(|message| message.contains(wait_guidance)),
        "legacy rollout should not already contain wait guidance"
    );

    let home = initial.home.clone();
    let rollout_path = initial
        .session_configured
        .rollout_path
        .clone()
        .expect("initial session should have a rollout path");
    initial.codex.shutdown_and_wait().await?;

    let mut removed_recorded_usage_hint = false;
    let mut removed_usage_hint_presence = false;
    let legacy_rollout = std::fs::read_to_string(&rollout_path)?
        .lines()
        .map(serde_json::from_str::<RolloutLine>)
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .map(|mut line| {
            if let RolloutItem::WorldState(world_state) = &mut line.item
                && let Some(state) = world_state.state.as_object_mut()
            {
                removed_recorded_usage_hint |= state.remove("multi_agent_usage_hint").is_some();
                if let Some(mode) = state
                    .get_mut("multi_agent_mode")
                    .and_then(Value::as_object_mut)
                {
                    removed_usage_hint_presence |= mode.remove("usage_hint_hash").is_some();
                }
            }
            serde_json::to_string(&line)
        })
        .collect::<std::result::Result<Vec<_>, _>>()?
        .join("\n");
    assert!(
        removed_recorded_usage_hint,
        "initial rollout should contain a usage-hint world-state section"
    );
    assert!(
        removed_usage_hint_presence,
        "initial rollout should record usage-hint presence on its active mode"
    );
    std::fs::write(&rollout_path, format!("{legacy_rollout}\n"))?;

    let resumed_responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-resumed-first"),
                ev_completed("resp-resumed-first"),
            ]),
            sse(vec![
                ev_response_created("resp-resumed-second"),
                ev_completed("resp-resumed-second"),
            ]),
        ],
    )
    .await;
    let mut resumed_builder = test_codex().with_config(move |config| {
        if let Some(root_agent_usage_hint_text) = resumed_root_agent_usage_hint_text {
            config.multi_agent_v2.root_agent_usage_hint_text = Some(root_agent_usage_hint_text);
        }
    });
    let resumed = resumed_builder.resume(&server, home, rollout_path).await?;
    let current_usage_hint = resumed
        .config
        .multi_agent_v2
        .root_agent_usage_hint_text
        .clone()
        .expect("current root usage hint should be configured");

    resumed.submit_turn("first turn after upgrading").await?;
    resumed.submit_turn("second turn after upgrading").await?;

    let requests = resumed_responses.requests();
    assert_eq!(requests.len(), 2);
    for request in &requests {
        let developer_messages = request.message_input_text_groups("developer");
        let current_usage_hint_positions = developer_messages
            .iter()
            .enumerate()
            .filter_map(|(index, group)| {
                (group.len() == 1 && group[0] == current_usage_hint).then_some(index)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            current_usage_hint_positions.len(),
            1,
            "current usage hint should appear exactly once as a standalone developer message: \
             {developer_messages:?}"
        );
        let active_mode_position = developer_messages
            .iter()
            .rposition(|group| {
                group
                    .iter()
                    .any(|message| message.contains(MULTI_AGENT_MODE_OPEN_TAG))
            })
            .expect("resumed context should include an active multi-agent mode");
        assert!(
            current_usage_hint_positions[0] < active_mode_position,
            "active mode must follow the current usage hint: {developer_messages:?}"
        );
        let wait_guidance_count = developer_messages
            .iter()
            .flatten()
            .filter(|message| message.contains(wait_guidance))
            .count();
        assert_eq!(
            wait_guidance_count,
            usize::from(expected_wait_guidance),
            "wait guidance must follow configured tool availability and usage-hint overrides"
        );

        let body = request.body_json();
        let wait_agent_tool = namespace_child_tool(&body, MULTI_AGENT_V2_NAMESPACE, "wait_agent");
        assert_eq!(wait_agent_tool.is_some(), wait_agent_enabled);
        if let Some(wait_agent_tool) = wait_agent_tool {
            assert_eq!(
                wait_agent_tool
                    .pointer("/parameters/properties/timeout_ms/description")
                    .and_then(Value::as_str),
                Some("Timeout in milliseconds. Defaults to 30000, min 10000, max 3600000.")
            );
        }
    }

    Ok(())
}

/// Resuming after wait-tool availability changes must refresh stale usage guidance once.
/// The active multi-agent mode must remain after the updated instructions.
#[test_case(true, false; "disabled wait agent invalidates prior guidance")]
#[test_case(false, true; "enabled wait agent adds current guidance")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn multi_agent_v2_resume_refreshes_changed_wait_guidance(
    initial_wait_agent_enabled: bool,
    resumed_wait_agent_enabled: bool,
) -> Result<()> {
    let wait_guidance =
        "When calling `wait_agent`, prefer longer waits (minutes) to avoid busy polling.";
    let initial_config_toml = format!(
        "[features.multi_agent_v2]\nenabled = true\nwait_agent_enabled = {initial_wait_agent_enabled}\n"
    );
    let server = start_mock_server().await;
    let initial_response = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-initial"),
            ev_completed("resp-initial"),
        ]),
    )
    .await;
    let initial = test_codex()
        .with_pre_build_hook(move |home| {
            std::fs::write(home.join("config.toml"), &initial_config_toml)
                .expect("write initial multi-agent configuration");
        })
        .build_with_auto_env(&server)
        .await?;

    initial
        .submit_turn("before changing wait-agent availability")
        .await?;

    let initial_request = initial_response.single_request();
    assert_eq!(
        namespace_child_tool(
            &initial_request.body_json(),
            MULTI_AGENT_V2_NAMESPACE,
            "wait_agent"
        )
        .is_some(),
        initial_wait_agent_enabled
    );

    let home = initial.home.clone();
    let rollout_path = initial
        .session_configured
        .rollout_path
        .clone()
        .expect("initial session should have a rollout path");
    initial.codex.shutdown_and_wait().await?;
    std::fs::write(
        home.path().join("config.toml"),
        format!(
            "[features.multi_agent_v2]\nenabled = true\nwait_agent_enabled = {resumed_wait_agent_enabled}\n"
        ),
    )?;

    let resumed_responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-resumed-first"),
                ev_completed("resp-resumed-first"),
            ]),
            sse(vec![
                ev_response_created("resp-resumed-second"),
                ev_completed("resp-resumed-second"),
            ]),
        ],
    )
    .await;
    let resumed = test_codex().resume(&server, home, rollout_path).await?;
    let current_usage_hint = resumed
        .config
        .multi_agent_v2
        .root_agent_usage_hint_text
        .clone()
        .expect("current root usage hint should be configured");

    resumed
        .submit_turn("first turn with updated wait-agent availability")
        .await?;
    resumed
        .submit_turn("second turn with updated wait-agent availability")
        .await?;

    let requests = resumed_responses.requests();
    assert_eq!(requests.len(), 2);
    for request in &requests {
        let developer_messages = request.message_input_text_groups("developer");
        let current_usage_hint_positions = developer_messages
            .iter()
            .enumerate()
            .filter_map(|(index, group)| {
                (group.len() == 1 && group[0] == current_usage_hint).then_some(index)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            current_usage_hint_positions.len(),
            1,
            "current usage hint should refresh earlier guidance exactly once: \
             {developer_messages:?}"
        );
        let current_usage_hint_position = current_usage_hint_positions[0];
        let current_usage_message = &developer_messages[current_usage_hint_position][0];
        assert_eq!(
            current_usage_message.contains(wait_guidance),
            resumed_wait_agent_enabled
        );
        let active_mode_position = developer_messages
            .iter()
            .rposition(|group| {
                group
                    .iter()
                    .any(|message| message.contains(MULTI_AGENT_MODE_OPEN_TAG))
            })
            .expect("resumed context should include an active multi-agent mode");
        assert!(
            current_usage_hint_position < active_mode_position,
            "active mode must follow updated usage instructions: {developer_messages:?}"
        );

        assert_eq!(
            namespace_child_tool(&request.body_json(), MULTI_AGENT_V2_NAMESPACE, "wait_agent")
                .is_some(),
            resumed_wait_agent_enabled
        );
    }

    Ok(())
}

#[test_case(true, false; "wait agent remains available without clock sleep")]
#[test_case(true, true; "wait agent remains available with clock sleep")]
#[test_case(false, false; "wait agent can be disabled without clock sleep")]
#[test_case(false, true; "wait agent can be disabled with clock sleep")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn multi_agent_v2_wait_agent_tool_follows_configuration(
    wait_agent_enabled: bool,
    sleep_tool_enabled: bool,
) -> Result<()> {
    let current_time_reminder = if sleep_tool_enabled {
        r#"
[features.current_time_reminder]
enabled = true
sleep_tool = true
"#
    } else {
        ""
    };
    let config_toml = format!(
        r#"
[features.multi_agent_v2]
enabled = true
wait_agent_enabled = {wait_agent_enabled}
{current_time_reminder}"#
    );
    let server = start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp1"), ev_completed("resp1")]),
    )
    .await;
    let test = test_codex()
        .with_pre_build_hook(move |home| {
            std::fs::write(home.join("config.toml"), &config_toml)
                .expect("write multi-agent configuration");
        })
        .build_with_auto_env(&server)
        .await?;

    test.submit_turn("hello").await?;

    let request = response.single_request();
    let body = request.body_json();
    assert!(namespace_child_tool(&body, MULTI_AGENT_V2_NAMESPACE, SPAWN_AGENT_TOOL_NAME).is_some());
    assert_eq!(
        namespace_child_tool(&body, MULTI_AGENT_V2_NAMESPACE, "wait_agent").is_some(),
        wait_agent_enabled
    );
    assert_eq!(
        namespace_child_tool(&body, "clock", "sleep").is_some(),
        sleep_tool_enabled
    );
    assert_eq!(
        request
            .message_input_texts("developer")
            .iter()
            .any(|message| {
                message.contains(
                "When calling `wait_agent`, prefer longer waits (minutes) to avoid busy polling.",
            )
            }),
        wait_agent_enabled
    );

    Ok(())
}
