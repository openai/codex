use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::write_models_cache;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use core_test_support::responses;
use serde_json::json;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

#[cfg(windows)]
const READ_TIMEOUT: Duration = Duration::from_secs(25);
#[cfg(not(windows))]
const READ_TIMEOUT: Duration = Duration::from_secs(10);

/// A roleless worker keeps inherited developer instructions across cold root resume and lazy reload.
#[tokio::test]
async fn cold_resume_preserves_inherited_developer_instructions_for_roleless_worker() -> Result<()>
{
    const PARENT_INSTRUCTIONS: &str = "parent-only developer instructions";
    const INITIAL_PROMPT: &str = "spawn a durable instruction worker";
    const INITIAL_TASK: &str = "perform the initial durable instruction task";
    const FOLLOWUP_PROMPT: &str = "continue the durable instruction worker";
    const FOLLOWUP_TASK: &str = "perform the resumed durable instruction task";
    const SPAWN_CALL_ID: &str = "spawn-durable-instruction-worker";
    const WAIT_CALL_ID: &str = "wait-for-durable-instruction-worker";
    const FOLLOWUP_CALL_ID: &str = "followup-durable-instruction-worker";
    const NAMESPACE: &str = "collaboration";

    let server = responses::start_mock_server().await;
    responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            String::from_utf8_lossy(&request.body).contains(INITIAL_PROMPT)
        },
        responses::sse(vec![
            responses::ev_response_created("initial-parent-spawn"),
            responses::ev_function_call_with_namespace(
                SPAWN_CALL_ID,
                NAMESPACE,
                "spawn_agent",
                &serde_json::to_string(&json!({
                    "message": INITIAL_TASK,
                    "task_name": "worker",
                    "fork_turns": "none",
                }))?,
            ),
            responses::ev_completed("initial-parent-spawn"),
        ]),
    )
    .await;
    let initial_child_request = responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            let body = String::from_utf8_lossy(&request.body);
            body.contains(INITIAL_TASK) && !body.contains(SPAWN_CALL_ID)
        },
        responses::sse(vec![
            responses::ev_response_created("initial-child-work"),
            responses::ev_assistant_message("initial-child-message", "initial child complete"),
            responses::ev_completed("initial-child-work"),
        ]),
    )
    .await;
    responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            let body = String::from_utf8_lossy(&request.body);
            body.contains(SPAWN_CALL_ID) && !body.contains(WAIT_CALL_ID)
        },
        responses::sse(vec![
            responses::ev_response_created("initial-parent-wait"),
            responses::ev_function_call_with_namespace(WAIT_CALL_ID, NAMESPACE, "wait_agent", "{}"),
            responses::ev_completed("initial-parent-wait"),
        ]),
    )
    .await;
    responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| String::from_utf8_lossy(&request.body).contains(WAIT_CALL_ID),
        responses::sse(vec![
            responses::ev_response_created("initial-parent-complete"),
            responses::ev_assistant_message("initial-parent-message", "initial parent complete"),
            responses::ev_completed("initial-parent-complete"),
        ]),
    )
    .await;

    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_model("gpt-5.4")
        .with_root_config(&format!("developer_instructions = {PARENT_INSTRUCTIONS:?}"))
        .with_extra_config("[features.multi_agent_v2]\nenabled = true")
        .write(codex_home.path())?;
    write_models_cache(codex_home.path())?;

    let thread_id = {
        let mut app_server = TestAppServer::builder()
            .with_codex_home(codex_home.path())
            .build_initialized()
            .await?;
        let ThreadStartResponse { thread, .. } = app_server
            .start_thread(ThreadStartParams {
                model: Some("gpt-5.4".to_string()),
                ..Default::default()
            })
            .await?;
        timeout(
            READ_TIMEOUT,
            app_server.start_turn_and_wait_for_completion(TurnStartParams {
                thread_id: thread.id.clone(),
                input: vec![UserInput::Text {
                    text: INITIAL_PROMPT.to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            }),
        )
        .await??;

        let initial_child_request = initial_child_request
            .requests()
            .into_iter()
            .find(|request| !request.inputs_of_type("agent_message").is_empty())
            .expect("initial worker model request");
        let developer_texts = initial_child_request.message_input_texts("developer");
        assert!(
            developer_texts
                .iter()
                .any(|text| text == PARENT_INSTRUCTIONS),
            "worker did not inherit parent developer instructions: {developer_texts:?}"
        );

        let shutdown = timeout(READ_TIMEOUT, app_server.shutdown_gracefully()).await??;
        assert!(
            shutdown.success(),
            "initial app-server shutdown failed: {shutdown}"
        );
        thread.id
    };

    responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            let body = String::from_utf8_lossy(&request.body);
            body.contains(FOLLOWUP_PROMPT) && !body.contains(FOLLOWUP_CALL_ID)
        },
        responses::sse(vec![
            responses::ev_response_created("resumed-parent-followup"),
            responses::ev_function_call_with_namespace(
                FOLLOWUP_CALL_ID,
                NAMESPACE,
                "followup_task",
                &serde_json::to_string(&json!({
                    "target": "worker",
                    "message": FOLLOWUP_TASK,
                }))?,
            ),
            responses::ev_completed("resumed-parent-followup"),
        ]),
    )
    .await;
    let resumed_child_request = responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            let body = String::from_utf8_lossy(&request.body);
            body.contains(FOLLOWUP_TASK) && !body.contains(FOLLOWUP_CALL_ID)
        },
        responses::sse(vec![
            responses::ev_response_created("resumed-child-work"),
            responses::ev_assistant_message("resumed-child-message", "resumed child complete"),
            responses::ev_completed("resumed-child-work"),
        ]),
    )
    .await;
    responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            String::from_utf8_lossy(&request.body).contains(FOLLOWUP_CALL_ID)
        },
        responses::sse(vec![
            responses::ev_response_created("resumed-parent-complete"),
            responses::ev_assistant_message("resumed-parent-message", "resumed parent complete"),
            responses::ev_completed("resumed-parent-complete"),
        ]),
    )
    .await;

    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let _: ThreadResumeResponse = app_server
        .request(|request_id| ClientRequest::ThreadResume {
            request_id,
            params: ThreadResumeParams {
                thread_id: thread_id.clone(),
                ..Default::default()
            },
        })
        .await?;
    let _: TurnStartResponse = app_server
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id,
                input: vec![UserInput::Text {
                    text: FOLLOWUP_PROMPT.to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?;
    let resumed_child_request = timeout(READ_TIMEOUT, async {
        loop {
            if let Some(request) = resumed_child_request
                .requests()
                .into_iter()
                .find(|request| {
                    request
                        .inputs_of_type("agent_message")
                        .iter()
                        .any(|message| {
                            message.get("recipient").and_then(serde_json::Value::as_str)
                                == Some("/root/worker")
                        })
                        && request.body_contains_text(FOLLOWUP_TASK)
                })
            {
                break request;
            }
            tokio::task::yield_now().await;
        }
    })
    .await?;

    let developer_texts = resumed_child_request.message_input_texts("developer");
    assert!(
        developer_texts
            .iter()
            .any(|text| text == PARENT_INSTRUCTIONS),
        "resumed worker lost its inherited developer instructions: {developer_texts:?}"
    );

    Ok(())
}
