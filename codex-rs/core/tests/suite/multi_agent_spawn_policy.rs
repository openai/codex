use anyhow::Result;
use codex_features::Feature;
use codex_protocol::config_types::MultiAgentMode;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::MULTI_AGENT_SPAWN_POLICY_OPEN_TAG;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
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

async fn submit_turn(
    codex: &codex_core::CodexThread,
    prompt: &str,
    mode: Option<MultiAgentMode>,
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
                multi_agent_mode: mode,
                ..Default::default()
            },
        })
        .await?;
    wait_for_event(codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn multi_agent_mode_is_sticky_and_emits_only_on_change() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        (1..=3)
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
        .with_config(|config| {
            config
                .features
                .enable(Feature::MultiAgentV2)
                .expect("test config should allow feature update");
        })
        .build(&server)
        .await?;

    assert!(
        test.codex
            .config_snapshot()
            .await
            .multi_agent_mode_available
    );

    submit_turn(&test.codex, "turn one", None).await?;
    submit_turn(&test.codex, "turn two", Some(MultiAgentMode::Proactive)).await?;
    submit_turn(&test.codex, "turn three", None).await?;

    let requests = responses.requests();
    let inputs = requests
        .iter()
        .map(core_test_support::responses::ResponsesRequest::input)
        .collect::<Vec<_>>();
    let first = developer_texts(&inputs[0]);
    let second = developer_texts(&inputs[1]);
    let third = developer_texts(&inputs[2]);

    assert_eq!(
        (
            count_containing(&first, MULTI_AGENT_SPAWN_POLICY_OPEN_TAG),
            count_containing(&first, NO_SPAWN_TEXT),
            count_containing(&first, PROACTIVE_TEXT),
        ),
        (1, 1, 0)
    );
    assert_eq!(
        (
            count_containing(&second, MULTI_AGENT_SPAWN_POLICY_OPEN_TAG),
            count_containing(&second, NO_SPAWN_TEXT),
            count_containing(&second, PROACTIVE_TEXT),
        ),
        (2, 1, 1)
    );
    assert_eq!(
        (
            count_containing(&third, MULTI_AGENT_SPAWN_POLICY_OPEN_TAG),
            count_containing(&third, NO_SPAWN_TEXT),
            count_containing(&third, PROACTIVE_TEXT),
        ),
        (2, 1, 1)
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_multi_agent_mode_is_rejected_without_multi_agent_v2() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let test = test_codex().build(&server).await?;

    assert!(
        !test
            .codex
            .config_snapshot()
            .await
            .multi_agent_mode_available
    );

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: ThreadSettingsOverrides {
                multi_agent_mode: Some(MultiAgentMode::Proactive),
                ..Default::default()
            },
        })
        .await?;

    let message = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::Error(error) => Some(error.message.clone()),
        _ => None,
    })
    .await;
    assert!(message.contains("multi_agent_mode"), "{message}");

    Ok(())
}
