use anyhow::Result;
use codex_core::config::Config;
use codex_features::Feature;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::MULTI_AGENT_MODE_OPEN_TAG;
use codex_protocol::protocol::Op;
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

const NO_SPAWN_TEXT: &str = "Do not spawn sub-agents unless the user or applicable AGENTS.md/skill instructions explicitly ask for sub-agents, delegation, or parallel agent work.";
const PROACTIVE_TEXT: &str = "Proactive multi-agent delegation is active.";

fn add_ultra_reasoning(model_info: &mut ModelInfo) {
    model_info.supports_reasoning_summaries = true;
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
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: prompt.to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: ThreadSettingsOverrides {
                effort: effort.map(Some),
                ..Default::default()
            },
        })
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
    let home = initial.home.clone();
    let rollout_path = initial
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path");

    submit_turn(&initial.codex, "before resume", /*effort*/ None).await?;
    drop(initial);

    let mut resume_builder = test_codex()
        .with_model_info_override("gpt-5.4", add_ultra_reasoning)
        .with_config(configure_ultra);
    let resumed = resume_builder.resume(&server, home, rollout_path).await?;
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
