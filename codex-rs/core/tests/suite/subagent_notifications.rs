use anyhow::Context;
use anyhow::Result;
use codex_core::features::Feature;
use codex_protocol::ThreadId;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_response_once_match;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::sse;
use core_test_support::responses::sse_response;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use serde_json::Value;
use serde_json::json;
use std::time::Duration;
use wiremock::MockServer;

const SPAWN_CALL_ID: &str = "spawn-call-1";
const WAIT_CALL_ID: &str = "wait-call-1";
const TURN_1_PROMPT: &str = "spawn a child and continue";
const TURN_2_NO_WAIT_PROMPT: &str = "follow up without wait";
const TURN_2_WAIT_PROMPT: &str = "wait for child completion";
const TURN_2_WAIT_UNRELATED_PROMPT: &str = "wait on unrelated id";
const TURN_3_PROMPT: &str = "next turn after wait";
const CHILD_PROMPT: &str = "child: do work";

fn body_contains(req: &wiremock::Request, text: &str) -> bool {
    std::str::from_utf8(&req.body).is_ok_and(|body| body.contains(text))
}

fn has_subagent_notification(req: &ResponsesRequest) -> bool {
    req.message_input_texts("user")
        .iter()
        .any(|text| text.contains("<subagent_notification>"))
}

fn spawned_agent_id(req: &ResponsesRequest) -> Result<String> {
    let spawn_output = req.function_call_output(SPAWN_CALL_ID);
    let output = spawn_output
        .get("output")
        .and_then(Value::as_str)
        .context("spawn function_call_output.output should be present")?;
    let payload: Value =
        serde_json::from_str(output).context("spawn output should be valid JSON")?;
    let agent_id = payload
        .get("agent_id")
        .and_then(Value::as_str)
        .context("spawn output should contain agent_id")?;
    Ok(agent_id.to_string())
}

fn wait_call_args(agent_id: &str) -> Result<String> {
    serde_json::to_string(&json!({
        "ids": [agent_id],
    }))
    .context("serialize wait args")
}

async fn setup_turn_one_with_spawned_child(
    server: &MockServer,
    child_response_delay: Option<Duration>,
) -> Result<(TestCodex, String)> {
    let spawn_args = serde_json::to_string(&json!({
        "message": CHILD_PROMPT,
    }))?;

    mount_sse_once_match(
        server,
        |req: &wiremock::Request| body_contains(req, TURN_1_PROMPT),
        sse(vec![
            ev_response_created("resp-turn1-1"),
            ev_function_call(SPAWN_CALL_ID, "spawn_agent", &spawn_args),
            ev_completed("resp-turn1-1"),
        ]),
    )
    .await;

    let child_sse = sse(vec![
        ev_response_created("resp-child-1"),
        ev_assistant_message("msg-child-1", "child done"),
        ev_completed("resp-child-1"),
    ]);
    if let Some(delay) = child_response_delay {
        mount_response_once_match(
            server,
            |req: &wiremock::Request| body_contains(req, CHILD_PROMPT),
            sse_response(child_sse).set_delay(delay),
        )
        .await;
    } else {
        mount_sse_once_match(
            server,
            |req: &wiremock::Request| body_contains(req, CHILD_PROMPT),
            child_sse,
        )
        .await;
    }

    let turn1_followup = mount_sse_once_match(
        server,
        |req: &wiremock::Request| body_contains(req, SPAWN_CALL_ID),
        sse(vec![
            ev_response_created("resp-turn1-2"),
            ev_assistant_message("msg-turn1-2", "parent done"),
            ev_completed("resp-turn1-2"),
        ]),
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::Collab);
    });
    let test = builder.build(server).await?;
    test.submit_turn(TURN_1_PROMPT).await?;
    let spawned_id = spawned_agent_id(&turn1_followup.single_request())?;

    Ok((test, spawned_id))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn subagent_notification_is_included_without_wait() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let (test, _spawned_id) = setup_turn_one_with_spawned_child(&server, None).await?;

    let turn2 = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-turn2-1"),
            ev_assistant_message("msg-turn2-1", "no wait path"),
            ev_completed("resp-turn2-1"),
        ]),
    )
    .await;
    test.submit_turn(TURN_2_NO_WAIT_PROMPT).await?;

    assert!(has_subagent_notification(&turn2.single_request()));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn subagent_notification_is_deduped_after_matching_wait() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let (test, spawned_id) = setup_turn_one_with_spawned_child(&server, None).await?;

    let wait_args = wait_call_args(&spawned_id)?;
    mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, TURN_2_WAIT_PROMPT),
        sse(vec![
            ev_response_created("resp-turn2-1"),
            ev_function_call(WAIT_CALL_ID, "wait", &wait_args),
            ev_completed("resp-turn2-1"),
        ]),
    )
    .await;
    mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, WAIT_CALL_ID),
        sse(vec![
            ev_response_created("resp-turn2-2"),
            ev_assistant_message("msg-turn2-2", "waited"),
            ev_completed("resp-turn2-2"),
        ]),
    )
    .await;
    test.submit_turn(TURN_2_WAIT_PROMPT).await?;

    let turn3 = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-turn3-1"),
            ev_assistant_message("msg-turn3-1", "after wait"),
            ev_completed("resp-turn3-1"),
        ]),
    )
    .await;
    test.submit_turn(TURN_3_PROMPT).await?;

    assert!(!has_subagent_notification(&turn3.single_request()));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn subagent_notification_is_deduped_when_wait_finishes_child_in_flight() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let (test, spawned_id) =
        setup_turn_one_with_spawned_child(&server, Some(Duration::from_millis(500))).await?;

    let wait_args = wait_call_args(&spawned_id)?;
    mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, TURN_2_WAIT_PROMPT),
        sse(vec![
            ev_response_created("resp-turn2f-1"),
            ev_function_call(WAIT_CALL_ID, "wait", &wait_args),
            ev_completed("resp-turn2f-1"),
        ]),
    )
    .await;
    mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, WAIT_CALL_ID),
        sse(vec![
            ev_response_created("resp-turn2f-2"),
            ev_assistant_message("msg-turn2f-2", "waited in flight"),
            ev_completed("resp-turn2f-2"),
        ]),
    )
    .await;
    test.submit_turn(TURN_2_WAIT_PROMPT).await?;

    let turn3 = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-turn3f-1"),
            ev_assistant_message("msg-turn3f-1", "after in-flight wait"),
            ev_completed("resp-turn3f-1"),
        ]),
    )
    .await;
    test.submit_turn(TURN_3_PROMPT).await?;

    assert!(!has_subagent_notification(&turn3.single_request()));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn subagent_notification_is_kept_after_non_matching_wait() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let (test, _spawned_id) = setup_turn_one_with_spawned_child(&server, None).await?;

    let unrelated_agent_id = ThreadId::new().to_string();
    let wait_args = wait_call_args(&unrelated_agent_id)?;
    mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, TURN_2_WAIT_UNRELATED_PROMPT),
        sse(vec![
            ev_response_created("resp-turn2u-1"),
            ev_function_call(WAIT_CALL_ID, "wait", &wait_args),
            ev_completed("resp-turn2u-1"),
        ]),
    )
    .await;
    mount_sse_once_match(
        &server,
        |req: &wiremock::Request| body_contains(req, WAIT_CALL_ID),
        sse(vec![
            ev_response_created("resp-turn2u-2"),
            ev_assistant_message("msg-turn2u-2", "waited unrelated"),
            ev_completed("resp-turn2u-2"),
        ]),
    )
    .await;
    test.submit_turn(TURN_2_WAIT_UNRELATED_PROMPT).await?;

    let turn3 = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-turn3u-1"),
            ev_assistant_message("msg-turn3u-1", "after unrelated wait"),
            ev_completed("resp-turn3u-1"),
        ]),
    )
    .await;
    test.submit_turn(TURN_3_PROMPT).await?;

    assert!(has_subagent_notification(&turn3.single_request()));

    Ok(())
}
