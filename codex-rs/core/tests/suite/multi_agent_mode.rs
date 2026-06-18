use anyhow::Result;
use codex_features::Feature;
use codex_protocol::config_types::MultiAgentMode;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::MULTI_AGENT_MODE_OPEN_TAG;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::time::Duration;

const SPAWN_CALL_ID: &str = "spawn-call-1";

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

fn body_contains(req: &wiremock::Request, text: &str) -> bool {
    String::from_utf8_lossy(&req.body).contains(text)
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
            config
                .features
                .enable(Feature::MultiAgentMode)
                .expect("test config should allow feature update");
        })
        .build(&server)
        .await?;

    submit_turn(&test.codex, "turn one", /*mode*/ None).await?;
    assert_eq!(test.codex.config_snapshot().await.multi_agent_mode, None);
    submit_turn(&test.codex, "turn two", Some(MultiAgentMode::Proactive)).await?;
    submit_turn(&test.codex, "turn three", /*mode*/ None).await?;

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
            count_containing(&first, MULTI_AGENT_MODE_OPEN_TAG),
            count_containing(&first, NO_SPAWN_TEXT),
            count_containing(&first, PROACTIVE_TEXT),
        ),
        (1, 1, 0)
    );
    assert_eq!(
        (
            count_containing(&second, MULTI_AGENT_MODE_OPEN_TAG),
            count_containing(&second, NO_SPAWN_TEXT),
            count_containing(&second, PROACTIVE_TEXT),
        ),
        (2, 1, 1)
    );
    assert_eq!(
        (
            count_containing(&third, MULTI_AGENT_MODE_OPEN_TAG),
            count_containing(&third, NO_SPAWN_TEXT),
            count_containing(&third, PROACTIVE_TEXT),
        ),
        (2, 1, 1)
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn existing_subagent_uses_root_multi_agent_mode_on_its_next_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let root_prompt = "spawn a child";
    let child_prompt = "child initial task";
    let child_followup = "child followup task";
    let spawn_args = serde_json::to_string(&json!({
        "message": child_prompt,
        "task_name": "worker",
    }))?;
    mount_sse_once_match(
        &server,
        move |req: &wiremock::Request| body_contains(req, root_prompt),
        sse(vec![
            ev_response_created("root-spawn-response"),
            ev_function_call(SPAWN_CALL_ID, "spawn_agent", &spawn_args),
            ev_completed("root-spawn-response"),
        ]),
    )
    .await;
    mount_sse_once_match(
        &server,
        move |req: &wiremock::Request| {
            body_contains(req, child_prompt) && !body_contains(req, SPAWN_CALL_ID)
        },
        sse(vec![
            ev_response_created("child-initial-response"),
            ev_completed("child-initial-response"),
        ]),
    )
    .await;
    mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, SPAWN_CALL_ID),
        sse(vec![
            ev_response_created("root-followup-response"),
            ev_completed("root-followup-response"),
        ]),
    )
    .await;
    let child_followup_response = mount_sse_once_match(
        &server,
        move |req: &wiremock::Request| body_contains(req, child_followup),
        sse(vec![
            ev_response_created("child-followup-response"),
            ev_completed("child-followup-response"),
        ]),
    )
    .await;

    let test = test_codex()
        .with_config(|config| {
            config
                .features
                .enable(Feature::Collab)
                .expect("test config should allow feature update");
            config
                .features
                .enable(Feature::MultiAgentV2)
                .expect("test config should allow feature update");
            config
                .features
                .enable(Feature::MultiAgentMode)
                .expect("test config should allow feature update");
            config
                .features
                .disable(Feature::EnableRequestCompression)
                .expect("test config should allow feature update");
        })
        .build(&server)
        .await?;
    let mut created_threads = test.thread_manager.subscribe_thread_created();

    submit_turn(&test.codex, root_prompt, /*mode*/ None).await?;
    let child_thread_id =
        tokio::time::timeout(Duration::from_secs(10), created_threads.recv()).await??;
    let child_thread = test.thread_manager.get_thread(child_thread_id).await?;
    wait_for_event(&child_thread, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.codex
        .submit(Op::ThreadSettings {
            thread_settings: ThreadSettingsOverrides {
                multi_agent_mode: Some(MultiAgentMode::Proactive),
                ..Default::default()
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::ThreadSettingsApplied(_))
    })
    .await;

    submit_turn(&child_thread, child_followup, /*mode*/ None).await?;

    let input = child_followup_response.single_request().input();
    let texts = developer_texts(&input);
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
async fn multi_agent_mode_feature_uses_explicit_mode_when_disabled() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
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

    submit_turn(&test.codex, "hello", /*mode*/ None).await?;

    let input = responses.single_request().input();
    let texts = developer_texts(&input);
    assert_eq!(
        (
            count_containing(&texts, MULTI_AGENT_MODE_OPEN_TAG),
            count_containing(&texts, NO_SPAWN_TEXT),
            count_containing(&texts, PROACTIVE_TEXT),
        ),
        (1, 1, 0)
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resume_compares_against_previous_effective_multi_agent_mode() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        (1..=4)
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
        .with_config(|config| {
            config
                .features
                .enable(Feature::MultiAgentV2)
                .expect("test config should allow feature update");
        })
        .build(&server)
        .await?;
    let home = initial.home.clone();
    let rollout_path = initial
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path");

    submit_turn(
        &initial.codex,
        "before resume",
        Some(MultiAgentMode::Proactive),
    )
    .await?;
    drop(initial);

    let mut resume_builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::MultiAgentV2)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::MultiAgentMode)
            .expect("test config should allow feature update");
    });
    let resumed = resume_builder.resume(&server, home, rollout_path).await?;
    submit_turn(
        &resumed.codex,
        "after resume",
        Some(MultiAgentMode::Proactive),
    )
    .await?;

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

    let resumed_rollout_path = resumed
        .session_configured
        .rollout_path
        .clone()
        .expect("resumed rollout path");
    let resumed_home = resumed.home.clone();
    drop(resumed);
    let mut same_mode_resume_builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::MultiAgentV2)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::MultiAgentMode)
            .expect("test config should allow feature update");
    });
    let resumed_same_mode = same_mode_resume_builder
        .resume(&server, resumed_home, resumed_rollout_path)
        .await?;
    submit_turn(
        &resumed_same_mode.codex,
        "after same-mode resume",
        /*mode*/ None,
    )
    .await?;

    assert_eq!(
        resumed_same_mode
            .codex
            .config_snapshot()
            .await
            .multi_agent_mode,
        Some(MultiAgentMode::Proactive)
    );
    let requests = responses.requests();
    let resumed_same_mode_input = requests[2].input();
    let texts = developer_texts(&resumed_same_mode_input);
    assert_eq!(
        (
            count_containing(&texts, MULTI_AGENT_MODE_OPEN_TAG),
            count_containing(&texts, NO_SPAWN_TEXT),
            count_containing(&texts, PROACTIVE_TEXT),
        ),
        (2, 1, 1)
    );

    let resumed_same_mode_rollout_path = resumed_same_mode
        .session_configured
        .rollout_path
        .clone()
        .expect("same-mode resumed rollout path");
    let resumed_same_mode_home = resumed_same_mode.home.clone();
    drop(resumed_same_mode);
    let mut disabled_mode_resume_builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::MultiAgentV2)
            .expect("test config should allow feature update");
    });
    let resumed_disabled_mode = disabled_mode_resume_builder
        .resume(
            &server,
            resumed_same_mode_home,
            resumed_same_mode_rollout_path,
        )
        .await?;
    submit_turn(
        &resumed_disabled_mode.codex,
        "after disabled-mode resume",
        /*mode*/ None,
    )
    .await?;

    assert_eq!(
        resumed_disabled_mode
            .codex
            .config_snapshot()
            .await
            .multi_agent_mode,
        Some(MultiAgentMode::Proactive)
    );
    let requests = responses.requests();
    let resumed_disabled_mode_input = requests[3].input();
    let texts = developer_texts(&resumed_disabled_mode_input);
    assert_eq!(
        (
            count_containing(&texts, MULTI_AGENT_MODE_OPEN_TAG),
            count_containing(&texts, NO_SPAWN_TEXT),
            count_containing(&texts, PROACTIVE_TEXT),
        ),
        (3, 2, 1)
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_multi_agent_mode_is_retained_without_multi_agent_v2() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let test = test_codex()
        .with_config(|config| {
            config
                .features
                .enable(Feature::MultiAgentMode)
                .expect("test config should allow feature update");
        })
        .build(&server)
        .await?;

    submit_turn(&test.codex, "hello", Some(MultiAgentMode::Proactive)).await?;

    assert_eq!(
        test.codex.config_snapshot().await.multi_agent_mode,
        Some(MultiAgentMode::Proactive)
    );
    let input = responses.single_request().input();
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
