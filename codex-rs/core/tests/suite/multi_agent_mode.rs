use anyhow::Result;
use codex_core::config::Config;
use codex_features::Feature;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::MULTI_AGENT_MODE_OPEN_TAG;
use codex_protocol::protocol::MultiAgentVersion;
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

const NO_SPAWN_TEXT: &str = "Do not spawn sub-agents unless the user explicitly asks for sub-agents, delegation, or parallel agent work.";
const PROACTIVE_TEXT: &str = "Proactive multi-agent delegation is active.";

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

async fn submit_turn_with_effort(
    codex: &codex_core::CodexThread,
    prompt: &str,
    effort: ReasoningEffort,
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
                effort: Some(Some(effort)),
                ..Default::default()
            },
        })
        .await?;
    wait_for_event(codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    Ok(())
}

fn configure_ultra_model(config: &mut Config, multi_agent_version: MultiAgentVersion) {
    let model = codex_core::test_support::get_model_offline(/*model*/ None);
    let mut catalog = codex_models_manager::bundled_models_response()
        .expect("bundled model catalog should parse");
    let model_info = catalog
        .models
        .iter_mut()
        .find(|model_info| model_info.slug == model)
        .expect("default model should exist in bundled catalog");
    model_info.supports_reasoning_summaries = true;
    model_info.multi_agent_version = Some(multi_agent_version);
    model_info
        .supported_reasoning_levels
        .push(ReasoningEffortPreset {
            effort: ReasoningEffort::Ultra,
            description: "Uses maximum reasoning with proactive delegation".to_string(),
        });

    config.model = Some(model);
    config.model_catalog = Some(catalog);
    config
        .features
        .enable(Feature::MultiAgentMode)
        .expect("test config should allow feature update");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ultra_uses_max_reasoning_and_proactive_mode_for_v2_turns() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        ],
    )
    .await;
    let test = test_codex()
        .with_config(|config| configure_ultra_model(config, MultiAgentVersion::V2))
        .build(&server)
        .await?;

    submit_turn_with_effort(&test.codex, "use ultra", ReasoningEffort::Ultra).await?;
    submit_turn_with_effort(&test.codex, "leave ultra", ReasoningEffort::High).await?;

    let requests = responses.requests();
    assert_eq!(requests[0].body_json()["reasoning"]["effort"], "max");
    assert_eq!(requests[1].body_json()["reasoning"]["effort"], "high");
    let first_input = requests[0].input();
    let first = developer_texts(&first_input);
    assert_eq!(
        (
            count_containing(&first, MULTI_AGENT_MODE_OPEN_TAG),
            count_containing(&first, NO_SPAWN_TEXT),
            count_containing(&first, PROACTIVE_TEXT),
        ),
        (1, 0, 1)
    );
    let second_input = requests[1].input();
    let second = developer_texts(&second_input);
    assert_eq!(
        (
            count_containing(&second, MULTI_AGENT_MODE_OPEN_TAG),
            count_containing(&second, NO_SPAWN_TEXT),
            count_containing(&second, PROACTIVE_TEXT),
        ),
        (2, 1, 1)
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ultra_uses_max_reasoning_without_mode_instructions_for_v1_turns() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let test = test_codex()
        .with_config(|config| configure_ultra_model(config, MultiAgentVersion::V1))
        .build(&server)
        .await?;

    submit_turn_with_effort(&test.codex, "use ultra", ReasoningEffort::Ultra).await?;

    let request = responses.single_request();
    assert_eq!(request.body_json()["reasoning"]["effort"], "max");
    let input = request.input();
    let texts = developer_texts(&input);
    assert_eq!(
        (
            count_containing(&texts, MULTI_AGENT_MODE_OPEN_TAG),
            count_containing(&texts, PROACTIVE_TEXT),
        ),
        (0, 0)
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cold_resume_uses_persisted_effective_mode_as_the_update_baseline() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        ],
    )
    .await;
    let initial = test_codex()
        .with_config(|config| configure_ultra_model(config, MultiAgentVersion::V2))
        .build(&server)
        .await?;
    let home = initial.home.clone();
    let rollout_path = initial
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path");

    submit_turn_with_effort(&initial.codex, "before resume", ReasoningEffort::Ultra).await?;
    drop(initial);

    let mut resume_builder =
        test_codex().with_config(|config| configure_ultra_model(config, MultiAgentVersion::V2));
    let resumed = resume_builder.resume(&server, home, rollout_path).await?;
    submit_turn_with_effort(&resumed.codex, "after resume", ReasoningEffort::High).await?;

    let requests = responses.requests();
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
