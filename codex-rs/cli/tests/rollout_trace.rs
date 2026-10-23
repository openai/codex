//! Exercise trace acceptance through the CLI app-server and its offline reducer.

use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_sequence;
use codex_app_server_protocol::AgentMessageDeltaNotification;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnInterruptParams;
use codex_app_server_protocol::TurnInterruptResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_rollout_trace::ConversationRole;
use codex_rollout_trace::ExecutionStatus;
use codex_rollout_trace::ProducerRef;
use codex_rollout_trace::RolloutTrace;
use core_test_support::responses;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::sync::oneshot;

#[tokio::test]
async fn accepted_output_survives_success_and_stream_failure() -> Result<()> {
    let codex = codex_utils_cargo_bin::cargo_bin("codex")?;
    for (source_id, terminal, status) in [
        (
            Some("msg-original"),
            "response.completed",
            ExecutionStatus::Completed,
        ),
        (None, "response.completed", ExecutionStatus::Completed),
        (
            Some("msg-original"),
            "response.failed",
            ExecutionStatus::Failed,
        ),
        (None, "response.failed", ExecutionStatus::Failed),
    ] {
        let home = TempDir::new()?;
        let traces = TempDir::new()?;
        let item = json!({
            "type": "message", "id": source_id, "role": "assistant",
            "content": [{"type": "output_text", "text": "accepted answer"}]
        });
        let mut terminal_response = json!({"id": "resp-1"});
        if status == ExecutionStatus::Failed {
            terminal_response["error"] =
                json!({"code": "server_error", "message": "stream failed"});
        }
        let response = [
            json!({"type": "response.created", "response": {"id": "resp-1"}}),
            json!({"type": "response.output_item.added", "item": item}),
            json!({"type": "response.output_item.done", "item": item}),
            json!({"type": terminal, "response": terminal_response}),
        ]
        .into_iter()
        .map(|event| format!("data: {event}\n\n"))
        .collect();
        let server = create_mock_responses_server_sequence(vec![response]).await;
        MockResponsesConfig::new(&server.uri()).write(home.path())?;
        let mut app = TestAppServer::builder()
            .with_program(&codex)
            .with_plugin_startup_tasks()
            .with_args(&["app-server"])
            .with_codex_home(home.path())
            .with_env_overrides(&[("CODEX_ROLLOUT_TRACE_ROOT", traces.path().to_str())])
            .build_initialized()
            .await?;
        let ThreadStartResponse { thread, .. } =
            app.start_thread(ThreadStartParams::default()).await?;
        let TurnStartResponse { turn } = app
            .request(|request_id| ClientRequest::TurnStart {
                request_id,
                params: TurnStartParams {
                    thread_id: thread.id.clone(),
                    input: vec![UserInput::Text {
                        text: "answer once".into(),
                        text_elements: Vec::new(),
                    }],
                    ..Default::default()
                },
            })
            .await?;
        let completed: TurnCompletedNotification = app.read_notification("turn/completed").await?;
        assert_eq!(
            completed.turn.status,
            if status == ExecutionStatus::Completed {
                TurnStatus::Completed
            } else {
                TurnStatus::Failed
            }
        );
        assert!(app.shutdown_gracefully().await?.success());

        // Exercise the exported artifact, not just an in-process reducer call.
        // There is no following request to prove acceptance of this final item.
        let bundle = std::fs::read_dir(traces.path())?.next().unwrap()?.path();
        let reduced = tokio::process::Command::new(&codex)
            .env("CODEX_HOME", home.path())
            .args(["debug", "trace-reduce"])
            .arg(&bundle)
            .output()
            .await?;
        assert!(
            reduced.status.success(),
            "{}",
            String::from_utf8_lossy(&reduced.stderr)
        );
        let rollout: RolloutTrace =
            serde_json::from_reader(std::fs::File::open(bundle.join("state.json"))?)?;
        let call = rollout.inference_calls.values().next().unwrap();
        let output_id = &call.response_item_ids[0];
        let output = &rollout.conversation_items[output_id];
        let mut expected = call.request_item_ids.clone();
        expected.push(output_id.clone());
        assert_eq!(rollout.threads[&thread.id].conversation_item_ids, expected);
        assert_eq!(rollout.codex_turns[&turn.id].execution.status, status);
        assert_eq!(call.execution.status, status);
        assert_eq!(
            output.produced_by,
            vec![ProducerRef::Inference {
                inference_call_id: call.inference_call_id.clone()
            }]
        );
        assert!(
            output
                .responses_item_id
                .as_ref()
                .is_some_and(|id| !id.is_empty())
        );
        if let Some(source_id) = source_id {
            assert_eq!(output.responses_item_id.as_deref(), Some(source_id));
        }
        let requests = server.received_requests().await.unwrap();
        let request = requests
            .iter()
            .find(|request| request.url.path().ends_with("/responses"))
            .unwrap();
        assert_eq!(
            request.headers["x-codex-inference-call-id"].to_str()?,
            call.inference_call_id
        );
    }
    Ok(())
}

#[tokio::test]
async fn accepted_output_survives_interruption_and_shutdown() -> Result<()> {
    let codex = codex_utils_cargo_bin::cargo_bin("codex")?;
    let home = TempDir::new()?;
    let traces = TempDir::new()?;
    let (release_response, response_gate) = oneshot::channel();
    let (server, _) = start_streaming_sse_server(vec![vec![
        StreamingSseChunk {
            gate: None,
            body: responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_assistant_message("accepted", "Keep this answer."),
                responses::ev_message_item_added("unfinished", ""),
                responses::ev_output_text_delta("Discard this unfinished text"),
            ]),
        },
        // Keep the provider stream open so only the client interrupt ends it.
        StreamingSseChunk {
            gate: Some(response_gate),
            body: String::new(),
        },
    ]])
    .await;
    MockResponsesConfig::new(server.uri()).write(home.path())?;
    let mut app = TestAppServer::builder()
        .with_program(&codex)
        .with_plugin_startup_tasks()
        .with_args(&["app-server"])
        .with_codex_home(home.path())
        .with_env_overrides(&[("CODEX_ROLLOUT_TRACE_ROOT", traces.path().to_str())])
        .build_initialized()
        .await?;
    let ThreadStartResponse { thread, .. } = app.start_thread(ThreadStartParams::default()).await?;
    let TurnStartResponse { turn } = app
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread.id.clone(),
                input: vec![UserInput::Text {
                    text: "answer once".into(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?;

    // Reaching the second item proves the session processed the first one.
    let delta: AgentMessageDeltaNotification =
        app.read_notification("item/agentMessage/delta").await?;
    assert_eq!(delta.item_id, "unfinished");
    let _: TurnInterruptResponse = app
        .request(|request_id| ClientRequest::TurnInterrupt {
            request_id,
            params: TurnInterruptParams {
                thread_id: thread.id.clone(),
                turn_id: turn.id.clone(),
            },
        })
        .await?;
    let completed: TurnCompletedNotification = app.read_notification("turn/completed").await?;
    assert_eq!(completed.turn.status, TurnStatus::Interrupted);
    assert!(app.shutdown_gracefully().await?.success());
    drop(release_response);
    server.shutdown().await;

    // Reduce immediately after shutdown: the background stream recorder must
    // have preserved the accepted output even without response.completed.
    let bundle = std::fs::read_dir(traces.path())?.next().unwrap()?.path();
    let reduced = tokio::process::Command::new(&codex)
        .env("CODEX_HOME", home.path())
        .args(["debug", "trace-reduce"])
        .arg(&bundle)
        .output()
        .await?;
    assert!(
        reduced.status.success(),
        "{}",
        String::from_utf8_lossy(&reduced.stderr)
    );
    let rollout: RolloutTrace =
        serde_json::from_reader(std::fs::File::open(bundle.join("state.json"))?)?;
    assert_eq!(rollout.inference_calls.len(), 1);
    let call = rollout.inference_calls.values().next().unwrap();
    assert_eq!(call.execution.status, ExecutionStatus::Cancelled);
    assert_eq!(
        rollout.codex_turns[&turn.id].execution.status,
        ExecutionStatus::Cancelled
    );
    assert_eq!(call.response_item_ids.len(), 1);
    let output_id = &call.response_item_ids[0];
    let output = &rollout.conversation_items[output_id];
    assert_eq!(output.responses_item_id.as_deref(), Some("accepted"));
    assert_eq!(
        output.produced_by,
        vec![ProducerRef::Inference {
            inference_call_id: call.inference_call_id.clone(),
        }]
    );
    let accepted_messages: Vec<_> = rollout.threads[&thread.id]
        .conversation_item_ids
        .iter()
        .filter(|id| rollout.conversation_items[*id].role == ConversationRole::Assistant)
        .collect();
    assert_eq!(accepted_messages, vec![output_id]);
    Ok(())
}
