//! Guardian circuit breakers notify the parent without changing the child's interrupted state.

use super::*;
use codex_config::config_toml::CircuitBreakAction;
use codex_core::config::Constrained;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::ErrorEvent;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::TurnAbortReason;
use core_test_support::ThreadIdle;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use test_case::test_case;
use tokio::sync::oneshot;

#[test_case(CircuitBreakAction::Strict; "strict_notifies_parent")]
#[test_case(CircuitBreakAction::Default; "default_stays_silent")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_circuit_breaker_notifies_parent(action: CircuitBreakAction) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.thread_lifecycle_contributor(Arc::new(ThreadIdle));
    let test = test_codex()
        .with_model_info_override("test-gpt-5.1-codex", |model| {
            model.multi_agent_version = Some(MultiAgentVersion::V2);
            model.auto_review_model_override = Some(model.slug.clone());
        })
        .with_extensions(Arc::new(extensions.build()))
        .with_config(move |config| {
            for feature in [
                Feature::Collab,
                Feature::MultiAgentV2,
                Feature::GuardianApproval,
            ] {
                config
                    .features
                    .enable(feature)
                    .expect("enable test feature");
            }
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config.guardian_circuit_break_action = action;
            config.model_provider.stream_max_retries = Some(0);
        })
        .build_with_auto_env(&server)
        .await?;
    let root = test.session_configured.thread_id;
    let is_root = move |request: &wiremock::Request| {
        decoded_body(request)
            .and_then(|body| serde_json::from_slice::<Value>(&body).ok())
            .is_some_and(|body| body["client_metadata"]["thread_id"] == json!(root))
    };
    let is_worker = move |request: &wiremock::Request| {
        decoded_body(request)
            .and_then(|body| serde_json::from_slice::<Value>(&body).ok())
            .is_some_and(|body| {
                body["client_metadata"]["x-codex-parent-thread-id"] == json!(root)
                    && body["client_metadata"]["x-openai-subagent"] != "guardian"
            })
    };
    let spawn = mount_sse_once_match(
        &server,
        is_root,
        sse(vec![
            ev_function_call_with_namespace(
                SPAWN_CALL_ID,
                MULTI_AGENT_V2_NAMESPACE,
                "spawn_agent",
                &json!({
                    "task_name": "worker", "fork_turns": "none", "message": "Run the checks."
                })
                .to_string(),
            ),
            ev_completed("spawn-worker"),
        ]),
    )
    .await;
    let parent_idle =
        mount_sse_once_match(&server, is_root, sse(vec![ev_completed("parent-idle")])).await;
    let mut calls = Vec::new();
    let mut reviews = Vec::new();
    for call in ["first", "second", "third"] {
        calls.push(
            mount_sse_once_match(
                &server,
                is_worker,
                sse(vec![
                    ev_function_call(
                        call,
                        "exec_command",
                        &json!({
                            "cmd": format!("echo {call}"),
                            "sandbox_permissions": "require_escalated",
                            "justification": "Exercise Guardian review."
                        })
                        .to_string(),
                    ),
                    ev_completed(call),
                ]),
            )
            .await,
        );
        reviews.push(mount_sse_once_match(
            &server,
            |request: &wiremock::Request| {
                decoded_body(request)
                    .and_then(|body| serde_json::from_slice::<Value>(&body).ok())
                    .is_some_and(|body| body["client_metadata"]["x-openai-subagent"] == "guardian")
            },
            sse(vec![
                ev_assistant_message(
                    "denied",
                    r#"{"risk_level":"high","user_authorization":"low","outcome":"deny","rationale":"Action is not authorized."}"#,
                ),
                ev_completed("review-denied"),
            ]),
        ).await);
    }
    // A continuation can race the third denial's interruption. Keep it pending so
    // only the actual Guardian circuit breaker can end the child's turn.
    let (_release, gate) = oneshot::channel();
    let (pending, _) = start_streaming_sse_server(vec![vec![StreamingSseChunk {
        gate: Some(gate),
        body: sse(vec![ev_completed("unexpected-worker-completion")]),
    }]])
    .await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(is_worker)
        .respond_with(
            wiremock::ResponseTemplate::new(/*s*/ 307)
                .insert_header("location", format!("{}/v1/responses", pending.uri())),
        )
        .with_priority(/*priority*/ 20)
        .mount(&server)
        .await;

    let mut created = test.thread_manager.subscribe_thread_created();
    test.submit_text_turn("Spawn a worker to run the checks.")
        .await?;
    ThreadIdle::wait(&test.codex).await;
    let worker = test
        .thread_manager
        .get_thread(created.recv().await?)
        .await?;
    let warning = wait_for_event_match(&worker, |event| match event {
        EventMsg::GuardianWarning(warning) if warning.message.contains("3 consecutive") => {
            Some(warning.message.clone())
        }
        _ => None,
    })
    .await;
    let aborted = wait_for_event_match(&worker, |event| match event {
        EventMsg::TurnAborted(aborted) => Some(aborted.clone()),
        _ => None,
    })
    .await;
    ThreadIdle::wait(&worker).await;
    assert_eq!(
        (aborted.reason, aborted.error, worker.agent_status().await),
        (
            TurnAbortReason::Interrupted,
            (action == CircuitBreakAction::Strict).then_some(ErrorEvent {
                message: warning.clone(),
                codex_error_info: Some(CodexErrorInfo::TooManyDenials),
                misalignment: None,
            }),
            AgentStatus::Interrupted,
        ),
    );

    // Delivery is queued. If the parent samples before draining its mailbox,
    // wait_agent must observe the notification before the next model request.
    let _wait = if action == CircuitBreakAction::Strict {
        Some(
            mount_sse_once_match(
                &server,
                move |request: &wiremock::Request| {
                    is_root(request) && !body_contains(request, "Agent interrupted by Guardian:")
                },
                sse(vec![
                    ev_function_call_with_namespace(
                        "wait-worker",
                        MULTI_AGENT_V2_NAMESPACE,
                        "wait_agent",
                        "{}",
                    ),
                    ev_completed("parent-wait"),
                ]),
            )
            .await,
        )
    } else {
        None
    };
    let continued = mount_sse_once_match(
        &server,
        move |request: &wiremock::Request| {
            is_root(request)
                && (action == CircuitBreakAction::Default
                    || body_contains(request, "Agent interrupted by Guardian:"))
        },
        sse(vec![ev_completed("parent-continued")]),
    )
    .await;
    test.submit_text_turn("Report the worker's status.").await?;
    let continued_request = continued.single_request();
    let messages = continued_request.inputs_of_type("agent_message");
    let expected = if action == CircuitBreakAction::Strict {
        vec![json!({
            "type": "agent_message",
            "author": "/root/worker",
            "recipient": "/root",
            "content": [{
                "type": "input_text",
                "text": format!(
                    "Message Type: FINAL_ANSWER\nTask name: /root\nSender: /root/worker\nPayload:\nAgent interrupted by Guardian: {warning}\n\nTell the user this agent stopped after repeated Guardian denials. Do not resume this agent or retry its blocked work until the user explicitly confirms that it should continue."
                ),
            }],
        })]
    } else {
        Vec::new()
    };
    assert_eq!(
        strip_response_item_ids_from_json(strip_metadata_from_json(json!(messages))),
        json!(expected),
    );
    assert_eq!(
        spawn.single_request().body_json()["client_metadata"]["thread_id"],
        json!(root)
    );
    assert!(parent_idle.requests().iter().any(|request| {
        request.body_json()["client_metadata"]["thread_id"] == json!(root)
            && request.function_call_output(SPAWN_CALL_ID)["output"].is_string()
    }));
    assert_eq!(
        reviews
            .iter()
            .map(|review| review
                .requests()
                .into_iter()
                .filter(|request| {
                    request.body_json()["client_metadata"]["x-openai-subagent"] == "guardian"
                })
                .count())
            .collect::<Vec<_>>(),
        vec![1, 1, 1],
    );
    assert_eq!(
        calls
            .iter()
            .map(|call| call
                .requests()
                .into_iter()
                .filter(|request| {
                    request.body_json()["client_metadata"]["x-codex-parent-thread-id"]
                        == json!(root)
                        && request.body_json()["client_metadata"]["x-openai-subagent"] != "guardian"
                })
                .count())
            .collect::<Vec<_>>(),
        vec![1, 1, 1]
    );
    pending.shutdown().await;
    Ok(())
}
