use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_core::config::Config;
use codex_features::Feature;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::MultiAgentMessages;
use codex_protocol::openai_models::MultiAgentModeMessages;
use codex_protocol::openai_models::MultiAgentRoleMessages;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::MULTI_AGENT_MODE_OPEN_TAG;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use test_case::test_case;

const NO_SPAWN_TEXT: &str = "Any earlier instruction enabling proactive multi-agent delegation no longer applies. Do not spawn sub-agents unless the user or applicable AGENTS.md/skill instructions explicitly ask for sub-agents, delegation, or parallel agent work.";
const PROACTIVE_TEXT: &str = "Proactive multi-agent delegation is active.";
const CUSTOM_MODE_HINT_TEXT: &str = "Use the configured delegation policy.";
const CATALOG_MODE_HINT_TEXT: &str = "Use the model catalog delegation policy.";
const CATALOG_EXPLICIT_TEXT: &str = "Use explicit delegation from the model catalog.";
const SECOND_MODEL_EXPLICIT_TEXT: &str = "Second model explicit mode.";
const FIRST_MODEL_ROOT_ROLE_TEXT: &str = "First model root role.";
const SECOND_MODEL_ROOT_ROLE_TEXT: &str = "Second model root role.";
const ROOT_USAGE_HINT_TEXT: &str = "Root usage hint.";

#[derive(Clone, Copy, Debug)]
enum ModeHintSource {
    ConfiguredHint,
    CatalogHint,
}

fn set_multi_agent_mode(
    model_info: &mut ModelInfo,
    explicit: &str,
    hint_text: Option<&str>,
    root_role: Option<&str>,
) {
    if let Some(model_messages) = model_info.model_messages.as_mut() {
        model_messages.multi_agent = Some(MultiAgentMessages {
            role: root_role.map(|root| MultiAgentRoleMessages {
                root: Some(root.to_string()),
                subagent: None,
            }),
            mode: Some(MultiAgentModeMessages {
                explicit: Some(explicit.to_string()),
                hint_text: hint_text.map(str::to_string),
            }),
        });
    }
}

fn add_ultra_reasoning(model_info: &mut ModelInfo) {
    model_info
        .supported_reasoning_levels
        .push(ReasoningEffortPreset {
            effort: ReasoningEffort::Ultra,
            description: "Ultra".to_string(),
        });
}

fn configure_multi_agent_v2(config: &mut Config) {
    config
        .features
        .enable(Feature::MultiAgentV2)
        .expect("test config should allow feature update");
}

// Configuring a custom mode hint also enables multi-agent V2 for the test.
fn configure_custom_mode_hint(config: &mut Config) {
    configure_multi_agent_v2(config);
    config.multi_agent_v2.multi_agent_mode_hint_text = Some(CUSTOM_MODE_HINT_TEXT.to_string());
}

fn configure_ultra(config: &mut Config) {
    configure_multi_agent_v2(config);
    config.model_reasoning_effort = Some(ReasoningEffort::Ultra);
}

fn developer_texts(input: &[Value]) -> Vec<&str> {
    input
        .iter()
        .filter(|item| item.get("role").and_then(Value::as_str) == Some("developer"))
        .filter_map(|item| item.get("content")?.as_array())
        .flatten()
        .filter_map(|content| content.get("text")?.as_str())
        .collect()
}

fn count_containing(texts: &[&str], target: &str) -> usize {
    texts.iter().filter(|text| text.contains(target)).count()
}

async fn submit_turn(
    codex: &codex_core::CodexThread,
    prompt: &str,
    effort: Option<ReasoningEffort>,
) -> Result<()> {
    codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: prompt.to_string(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                effort: effort.map(Some),
                ..Default::default()
            }),
        )
        .await?;
    wait_for_event(codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ultra_reasoning_uses_max_and_proactive_mode() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let test = test_codex()
        .with_model_info_override("gpt-5.4", add_ultra_reasoning)
        .with_config(configure_ultra)
        .build(&server)
        .await?;

    submit_turn(&test.codex, "hello", /*effort*/ None).await?;

    let request = response.single_request();
    assert_eq!(
        request.body_json()["reasoning"]["effort"].as_str(),
        Some("max")
    );
    let input = request.input();
    let texts = developer_texts(&input);
    assert_eq!(
        (
            count_containing(&texts, NO_SPAWN_TEXT),
            count_containing(&texts, PROACTIVE_TEXT),
        ),
        (0, 1)
    );

    Ok(())
}

#[test_case(ModeHintSource::ConfiguredHint; "configured hint overrides catalog")]
#[test_case(ModeHintSource::CatalogHint; "catalog hint overrides reasoning effort")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mode_hints_override_reasoning_effort(source: ModeHintSource) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        (1..=2)
            .map(|index| {
                sse(vec![
                    ev_response_created(&format!("resp-{index}")),
                    ev_completed(&format!("resp-{index}")),
                ])
            })
            .collect(),
    )
    .await;
    let test = test_codex()
        .with_model_info_override("gpt-5.4", |model_info| {
            add_ultra_reasoning(model_info);
            set_multi_agent_mode(
                model_info,
                CATALOG_EXPLICIT_TEXT,
                Some(CATALOG_MODE_HINT_TEXT),
                /*root_role*/ None,
            );
        })
        .with_config(move |config| match source {
            ModeHintSource::ConfiguredHint => configure_custom_mode_hint(config),
            ModeHintSource::CatalogHint => configure_multi_agent_v2(config),
        })
        .build(&server)
        .await?;
    submit_turn(&test.codex, "explicit", Some(ReasoningEffort::High)).await?;
    submit_turn(&test.codex, "proactive", Some(ReasoningEffort::Ultra)).await?;

    let requests = responses.requests();
    let first_input = requests[0].input();
    let first_texts = developer_texts(&first_input);
    let second_input = requests[1].input();
    let second_texts = developer_texts(&second_input);
    let (expected_hint, suppressed_hint) = match source {
        ModeHintSource::ConfiguredHint => (CUSTOM_MODE_HINT_TEXT, CATALOG_MODE_HINT_TEXT),
        ModeHintSource::CatalogHint => (CATALOG_MODE_HINT_TEXT, CATALOG_EXPLICIT_TEXT),
    };
    for texts in [&first_texts, &second_texts] {
        assert_eq!(
            (
                count_containing(texts, expected_hint),
                count_containing(texts, NO_SPAWN_TEXT),
                count_containing(texts, PROACTIVE_TEXT),
            ),
            (1, 0, 0)
        );
        assert_eq!(count_containing(texts, suppressed_hint), 0);
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_switch_refreshes_catalog_role_and_explicit_mode() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        (1..=2)
            .map(|index| {
                sse(vec![
                    ev_response_created(&format!("resp-{index}")),
                    ev_completed(&format!("resp-{index}")),
                ])
            })
            .collect(),
    )
    .await;
    let test = test_codex()
        .with_model_info_override("gpt-5.2", |model_info| {
            set_multi_agent_mode(
                model_info,
                SECOND_MODEL_EXPLICIT_TEXT,
                /*hint_text*/ None,
                Some(SECOND_MODEL_ROOT_ROLE_TEXT),
            );
        })
        .with_model_info_override("gpt-5.4", |model_info| {
            set_multi_agent_mode(
                model_info,
                CATALOG_EXPLICIT_TEXT,
                /*hint_text*/ None,
                Some(FIRST_MODEL_ROOT_ROLE_TEXT),
            );
        })
        .with_config(configure_multi_agent_v2)
        .build_with_auto_env(&server)
        .await?;
    submit_turn(&test.codex, "first model", Some(ReasoningEffort::High)).await?;
    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            model: Some("gpt-5.2".to_string()),
            ..Default::default()
        },
    )
    .await?;
    submit_turn(&test.codex, "second model", Some(ReasoningEffort::High)).await?;

    let requests = responses.requests();
    for (index, request) in requests.iter().enumerate() {
        let input = request.input();
        let texts = developer_texts(&input);
        assert_eq!(
            (
                count_containing(&texts, CATALOG_EXPLICIT_TEXT),
                count_containing(&texts, SECOND_MODEL_EXPLICIT_TEXT),
                count_containing(&texts, NO_SPAWN_TEXT),
                count_containing(&texts, PROACTIVE_TEXT),
            ),
            (1, usize::from(index == 1), 0, 0)
        );
    }

    let developer_messages = requests[1].message_input_text_groups("developer");
    let [first_role_position, second_role_position] =
        [FIRST_MODEL_ROOT_ROLE_TEXT, SECOND_MODEL_ROOT_ROLE_TEXT].map(|role| {
            developer_messages
                .iter()
                .position(|group| {
                    matches!(
                        group.as_slice(),
                        [text] if text.starts_with("<multi_agent_role>")
                            && text.ends_with("</multi_agent_role>")
                            && text.contains(role)
                    )
                })
                .expect("catalog root role should be a standalone marked developer message")
        });
    let active_mode_position = developer_messages
        .iter()
        .rposition(|group| {
            group
                .iter()
                .any(|text| text.contains(MULTI_AGENT_MODE_OPEN_TAG))
        })
        .expect("active multi-agent mode should be present");
    assert!(
        first_role_position < second_role_position && second_role_position < active_mode_position,
        "catalog root roles should remain ordered before the active mode: {developer_messages:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn empty_configured_mode_hint_emits_no_mode_message() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let test = test_codex()
        .with_config(|config| {
            configure_multi_agent_v2(config);
            config.multi_agent_v2.multi_agent_mode_hint_text = Some(String::new());
        })
        .build(&server)
        .await?;

    submit_turn(&test.codex, "hello", Some(ReasoningEffort::High)).await?;

    let input = response.single_request().input();
    let texts = developer_texts(&input);
    assert_eq!(
        (
            count_containing(&texts, MULTI_AGENT_MODE_OPEN_TAG),
            count_containing(&texts, NO_SPAWN_TEXT),
            count_containing(&texts, PROACTIVE_TEXT),
        ),
        (0, 0, 0)
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn changing_configured_mode_hint_to_empty_emits_no_update() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        (1..=2)
            .map(|index| {
                sse(vec![
                    ev_response_created(&format!("resp-{index}")),
                    ev_completed(&format!("resp-{index}")),
                ])
            })
            .collect(),
    )
    .await;
    let initial = test_codex()
        .with_config(configure_custom_mode_hint)
        .build(&server)
        .await?;

    submit_turn(&initial.codex, "before resume", /*effort*/ None).await?;

    let mut resume_builder = test_codex().with_config(|config| {
        configure_multi_agent_v2(config);
        config.multi_agent_v2.multi_agent_mode_hint_text = Some(String::new());
    });
    let resumed = resume_builder.restart(&server, &initial).await?;
    drop(initial);
    submit_turn(&resumed.codex, "after resume", /*effort*/ None).await?;

    let requests = responses.requests();
    let first_input = requests[0].input();
    let first_texts = developer_texts(&first_input);
    let resumed_input = requests[1].input();
    let resumed_texts = developer_texts(&resumed_input);
    assert_eq!(
        (
            count_containing(&first_texts, MULTI_AGENT_MODE_OPEN_TAG),
            count_containing(&resumed_texts, MULTI_AGENT_MODE_OPEN_TAG),
            count_containing(&resumed_texts, CUSTOM_MODE_HINT_TEXT),
        ),
        (1, 1, 1)
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_mode_change_appends_mode_without_reappending_usage_hint() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        (1..=2)
            .map(|index| {
                sse(vec![
                    ev_response_created(&format!("resp-{index}")),
                    ev_completed(&format!("resp-{index}")),
                ])
            })
            .collect(),
    )
    .await;
    let test = test_codex()
        .with_model_info_override("gpt-5.4", add_ultra_reasoning)
        .with_config(|config| {
            configure_ultra(config);
            config.multi_agent_v2.root_agent_usage_hint_text =
                Some(ROOT_USAGE_HINT_TEXT.to_string());
        })
        .build_with_auto_env(&server)
        .await?;
    let rollout_path = test
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path");

    submit_turn(&test.codex, "proactive", /*effort*/ None).await?;
    submit_turn(&test.codex, "explicit", Some(ReasoningEffort::High)).await?;

    let requests = responses.requests();
    let first_input = requests[0].input();
    let first_texts = developer_texts(&first_input);
    let hint_index = first_texts
        .iter()
        .position(|text| text.contains(ROOT_USAGE_HINT_TEXT))
        .expect("initial usage hint");
    let mode_index = first_texts
        .iter()
        .position(|text| text.contains(PROACTIVE_TEXT))
        .expect("initial proactive mode");
    assert!(hint_index < mode_index);

    let second_input = requests[1].input();
    let second_texts = developer_texts(&second_input);
    assert_eq!(
        (
            count_containing(&second_texts, ROOT_USAGE_HINT_TEXT),
            count_containing(&second_texts, PROACTIVE_TEXT),
            count_containing(&second_texts, NO_SPAWN_TEXT),
        ),
        (1, 1, 1),
    );
    test.codex.ensure_rollout_materialized().await;
    test.codex.flush_rollout().await?;
    let rollout_values = std::fs::read_to_string(rollout_path)?
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<serde_json::Result<Vec<_>>>()?;
    let recorded_modes = rollout_values
        .iter()
        .filter(|value| value.get("type").and_then(Value::as_str) == Some("world_state"))
        .filter_map(|value| {
            value
                .pointer("/payload/state/multi_agent_mode/mode")
                .cloned()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        recorded_modes,
        [json!("proactive"), json!("explicitRequestOnly")]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn leaving_ultra_after_cold_resume_emits_explicit_mode() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        (1..=2)
            .map(|index| {
                sse(vec![
                    ev_response_created(&format!("resp-{index}")),
                    ev_completed(&format!("resp-{index}")),
                ])
            })
            .collect(),
    )
    .await;
    let initial = test_codex()
        .with_model_info_override("gpt-5.4", add_ultra_reasoning)
        .with_config(configure_ultra)
        .build(&server)
        .await?;

    submit_turn(&initial.codex, "before resume", /*effort*/ None).await?;

    let mut resume_builder = test_codex()
        .with_model_info_override("gpt-5.4", add_ultra_reasoning)
        .with_config(configure_ultra);
    let resumed = resume_builder.restart(&server, &initial).await?;
    drop(initial);
    submit_turn(&resumed.codex, "after resume", Some(ReasoningEffort::High)).await?;

    let requests = responses.requests();
    assert_eq!(
        (
            requests[0].body_json()["reasoning"]["effort"]
                .as_str()
                .map(str::to_string),
            requests[1].body_json()["reasoning"]["effort"]
                .as_str()
                .map(str::to_string),
        ),
        (Some("max".to_string()), Some("high".to_string()))
    );
    let resumed_input = requests[1].input();
    let texts = developer_texts(&resumed_input);
    assert_eq!(
        (
            count_containing(&texts, MULTI_AGENT_MODE_OPEN_TAG),
            count_containing(&texts, NO_SPAWN_TEXT),
            count_containing(&texts, PROACTIVE_TEXT),
        ),
        (2, 1, 1)
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ultra_on_multi_agent_v1_uses_max_without_mode_instructions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let test = test_codex()
        .with_model_info_override("gpt-5.4", add_ultra_reasoning)
        .with_config(|config| {
            config.model_reasoning_effort = Some(ReasoningEffort::Ultra);
        })
        .build(&server)
        .await?;

    submit_turn(&test.codex, "hello", /*effort*/ None).await?;

    let request = response.single_request();
    assert_eq!(
        request.body_json()["reasoning"]["effort"].as_str(),
        Some("max")
    );
    let input = request.input();
    let texts = developer_texts(&input);
    assert_eq!(count_containing(&texts, MULTI_AGENT_MODE_OPEN_TAG), 0);

    Ok(())
}
