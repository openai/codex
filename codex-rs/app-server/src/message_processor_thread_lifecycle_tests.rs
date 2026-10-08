//! Exercises thread cleanup when startup or child creation outlives the client connection.

use codex_app_server_protocol::TurnStartResponse;
use codex_features::Feature;
use codex_protocol::protocol::AgentStatus;
use core_test_support::responses;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use pretty_assertions::assert_eq;
use test_case::test_case;
use tokio::sync::oneshot;

use super::ConnectionSessionState;
use super::MessageProcessor;
use super::message_processor_tracing_tests::build_test_processor;
use super::message_processor_tracing_tests::read_response_from;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::OutgoingEnvelope;
use crate::outgoing_message::OutgoingMessage;
use crate::transport::AppServerTransport;
use crate::transport::ConnectionOrigin;
use anyhow::Context;
use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use codex_app_server_protocol::InitializeResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadClosedNotification;
use codex_app_server_protocol::ThreadLoadedListResponse;
use codex_app_server_protocol::ThreadStartResponse;
use codex_core::config::ConfigBuilder;
use codex_login::AuthManager;
use core_test_support::fs_wait::wait_for_path_exists;
use core_test_support::process::wait_for_pid_file;
use core_test_support::stdio_server_bin;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

#[tokio::test]
async fn thread_start_unloads_after_requesting_connection_closes_during_mcp_startup() -> Result<()>
{
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let home = TempDir::new()?;
    let barrier_file = home.path().join("allow-initialize");
    let pid_file = home.path().join("mcp.pid");
    MockResponsesConfig::new(&server.uri())
        .with_root_config("thread_unload_delay_secs = 1")
        .with_extra_config(&format!(
            r#"[mcp_servers.blocked]
command = {}
required = true
startup_timeout_sec = 120

[mcp_servers.blocked.env]
MCP_TEST_INITIALIZE_BARRIER_FILE = {}
MCP_TEST_PID_FILE = {}
"#,
            toml::Value::String(stdio_server_bin()?),
            toml::Value::String(barrier_file.to_string_lossy().into_owned()),
            toml::Value::String(pid_file.to_string_lossy().into_owned()),
        ))
        .write(home.path())?;
    let config = Arc::new(
        ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .build()
            .await?,
    );
    let auth =
        AuthManager::shared_from_config(config.as_ref(), /*enable_codex_api_key_env*/ false)
            .await?;
    let (processor, mut outgoing) = build_test_processor(config, auth).await;
    let owner_id = ConnectionId(1);
    let owner = Arc::new(ConnectionSessionState::new(ConnectionOrigin::WebSocket));
    let result: Result<()> = async {
        send_request(
            &processor,
            owner_id,
            &owner,
            json!({"id": 1, "method": "initialize", "params": {
                "clientInfo": {"name": "startup-owner", "version": "1"},
                "capabilities": {"experimentalApi": true}
            }}),
        )
        .await;
        let _: InitializeResponse =
            read_response_from(&mut outgoing, owner_id, /*request_id*/ 1).await;
        send_request(
            &processor,
            owner_id,
            &owner,
            json!({"id": 2, "method": "thread/start", "params": {"ephemeral": true}}),
        )
        .await;
        wait_for_path_exists(&pid_file, Duration::from_secs(/*secs*/ 10)).await?;
        #[cfg(unix)]
        let pid = wait_for_pid_file(&pid_file).await?;
        #[cfg(not(unix))]
        wait_for_pid_file(&pid_file).await?;
        #[cfg(unix)]
        anyhow::ensure!(
            core_test_support::process::process_is_alive(&pid)?,
            "MCP process should be running before releasing startup"
        );

        // Advance only connection cleanup's clock. Startup and idle unloading keep
        // using the original runtime's real clock while the MCP barrier stays closed.
        let cleanup = tokio::task::spawn_blocking({
            let processor = Arc::clone(&processor);
            let owner = Arc::clone(&owner);
            move || -> Result<()> {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .start_paused(/*start_paused*/ true)
                    .build()?;
                runtime.block_on(async {
                    timeout(
                        Duration::from_secs(/*secs*/ 45),
                        processor.connection_closed(owner_id, &owner),
                    )
                    .await
                    .context("connection cleanup did not finish while MCP startup was blocked")
                })
            }
        });
        cleanup
            .await
            .context("connection cleanup task panicked")??;
        std::fs::write(&barrier_file, "ready")?;

        // The harness receives the completed handler's output even though its transport is gone.
        // Retain both messages because idle unloading can finish before the response is consumed.
        let (started, closed) = timeout(Duration::from_secs(/*secs*/ 10), async {
            let mut started: Option<ThreadStartResponse> = None;
            let mut closed = None;
            while started.is_none() || closed.is_none() {
                let envelope = outgoing.recv().await.context("outgoing channel closed")?;
                match envelope {
                    OutgoingEnvelope::ToConnection {
                        connection_id,
                        message: OutgoingMessage::Response(response),
                        ..
                    } if connection_id == owner_id && response.id == RequestId::Integer(2) => {
                        started = Some(serde_json::from_value(serde_json::to_value(
                            response.result,
                        )?)?);
                    }
                    OutgoingEnvelope::Broadcast {
                        message: OutgoingMessage::AppServerNotification(notification),
                    } => {
                        if let ServerNotification::ThreadClosed(notification) =
                            notification.notification
                        {
                            closed = Some(notification);
                        }
                    }
                    _ => {}
                }
            }
            anyhow::Ok((
                started.context("missing thread/start response")?,
                closed.context("missing thread/closed notification")?,
            ))
        })
        .await
        .context("thread created after disconnect did not unload")??;
        anyhow::ensure!(
            closed
                == (ThreadClosedNotification {
                    thread_id: started.thread.id
                }),
            "unexpected thread/closed notification: {closed:?}"
        );

        let observer_id = ConnectionId(2);
        let observer = Arc::new(ConnectionSessionState::new(ConnectionOrigin::WebSocket));
        send_request(
            &processor,
            observer_id,
            &observer,
            json!({"id": 3, "method": "initialize", "params": {
                "clientInfo": {"name": "loaded-thread-observer", "version": "1"}
            }}),
        )
        .await;
        let _: InitializeResponse =
            read_response_from(&mut outgoing, observer_id, /*request_id*/ 3).await;
        send_request(
            &processor,
            observer_id,
            &observer,
            json!({"id": 4, "method": "thread/loaded/list", "params": {}}),
        )
        .await;
        let loaded: ThreadLoadedListResponse =
            read_response_from(&mut outgoing, observer_id, /*request_id*/ 4).await;
        anyhow::ensure!(
            loaded
                == (ThreadLoadedListResponse {
                    data: Vec::new(),
                    next_cursor: None,
                }),
            "disconnected thread remained loaded: {loaded:?}"
        );
        #[cfg(unix)]
        core_test_support::process::wait_for_process_exit(&pid).await?;
        Ok(())
    }
    .await;

    // Release the startup fixture and reap its runtime even when the regression assertion fails.
    std::fs::write(&barrier_file, "ready")?;
    drop(outgoing);
    let drained = timeout(Duration::from_secs(/*secs*/ 10), owner.rpc_gate.shutdown()).await;
    processor.clear_runtime_references();
    processor.shutdown_threads().await;
    drained.context("thread/start did not finish during test cleanup")?;
    result?;
    Ok(())
}

#[test_case(0, false; "v2_zero_delay")]
#[test_case(1, false; "v2_one_second_delay")]
#[test_case(1, true; "ephemeral_v2_followup_after_idle_deadline")]
#[tokio::test]
async fn disconnected_v2_child_lifecycle(unload_delay_secs: u64, ephemeral: bool) -> Result<()> {
    let (release_spawn, spawn_gate) = oneshot::channel();
    let (release_spawn_completion, spawn_completion_gate) = oneshot::channel();
    let (release_first_completion, first_completion_gate) = oneshot::channel();
    let (release_second_completion, second_completion_gate) = oneshot::channel();
    let done = responses::sse(vec![
        responses::ev_response_created("done"),
        responses::ev_assistant_message("done", "Done"),
        responses::ev_completed("done"),
    ]);
    let (server, _completions) = start_streaming_sse_server(vec![
        vec![
            StreamingSseChunk {
                gate: Some(spawn_gate),
                body: responses::sse(vec![
                    responses::ev_response_created("spawn"),
                    responses::ev_function_call_with_namespace(
                        "spawn",
                        "collaboration",
                        "spawn_agent",
                        &json!({"task_name": "child", "message": "Reply done", "fork_turns": "none"})
                            .to_string(),
                    ),
                ]),
            },
            StreamingSseChunk {
                gate: Some(spawn_completion_gate),
                body: responses::sse(vec![responses::ev_completed("spawn")]),
            },
        ],
        vec![StreamingSseChunk {
            gate: Some(first_completion_gate),
            body: done.clone(),
        }],
        vec![StreamingSseChunk {
            gate: Some(second_completion_gate),
            body: if ephemeral {
                responses::sse(vec![
                    responses::ev_response_created("followup"),
                    responses::ev_function_call_with_namespace(
                        "followup",
                        "collaboration",
                        "followup_task",
                        &json!({"target": "child", "message": "Recall your previous reply"})
                            .to_string(),
                    ),
                    responses::ev_completed("followup"),
                ])
            } else {
                done.clone()
            },
        }],
        // Follow-up responses and the child's completion notification.
        vec![StreamingSseChunk {
            gate: None,
            body: done.clone(),
        }],
        vec![StreamingSseChunk {
            gate: None,
            body: done.clone(),
        }],
        vec![StreamingSseChunk {
            gate: None,
            body: done,
        }],
    ])
    .await;
    let home = TempDir::new()?;
    let config = MockResponsesConfig::new(server.uri())
        .with_root_config(&format!(
            "thread_unload_delay_secs = {unload_delay_secs}\napprovals_reviewer = \"user\""
        ))
        .enable_feature(Feature::Collab)
        .enable_feature(Feature::MultiAgentV2);
    config.write(home.path())?;
    let config = Arc::new(
        ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .build()
            .await?,
    );
    let auth =
        AuthManager::shared_from_config(config.as_ref(), /*enable_codex_api_key_env*/ false)
            .await?;
    let (processor, mut outgoing) = build_test_processor(config, auth).await;
    let mut created = processor.thread_created_receiver();
    let owner_id = ConnectionId(1);
    let owner = Arc::new(ConnectionSessionState::new(ConnectionOrigin::WebSocket));
    let result: Result<()> = async {
        send_request(
            &processor,
            owner_id,
            &owner,
            json!({"id": 1, "method": "initialize", "params": {
                "clientInfo": {"name": "parent-owner", "version": "1"},
                "capabilities": {"experimentalApi": true}
            }}),
        )
        .await;
        let _: InitializeResponse =
            read_response_from(&mut outgoing, owner_id, /*request_id*/ 1).await;
        processor
            .connection_initialized(owner_id, owner.request_attestation())
            .await;
        send_request(
            &processor,
            owner_id,
            &owner,
            json!({"id": 2, "method": "thread/start", "params": {"ephemeral": ephemeral}}),
        )
        .await;
        let parent: ThreadStartResponse =
            read_response_from(&mut outgoing, owner_id, /*request_id*/ 2).await;
        send_request(
            &processor,
            owner_id,
            &owner,
            json!({"id": 3, "method": "turn/start", "params": {
                "threadId": parent.thread.id,
                "input": [{"type": "text", "text": "Spawn a child and finish"}]
            }}),
        )
        .await;
        let _: TurnStartResponse =
            read_response_from(&mut outgoing, owner_id, /*request_id*/ 3).await;
        timeout(
            Duration::from_secs(/*secs*/ 10),
            server.wait_for_request_count(/*count*/ 1),
        )
        .await?;

        // Finish connection cleanup before the model can create a child. Dispatch the
        // core creation event exactly as the app-server does with no initialized clients.
        processor.connection_closed(owner_id, &owner).await;
        release_spawn.send(()).expect("parent response is waiting");
        let child_id = timeout(Duration::from_secs(/*secs*/ 10), created.recv()).await??;
        let child_runtime = Arc::downgrade(
            &processor
                .thread_processor
                .thread_manager
                .get_thread(child_id)
                .await?,
        );
        let attach = processor.try_attach_thread_listener(child_id, Vec::new());
        tokio::pin!(attach);
        // The old runtime has left core, but its app-server cleanup still owns the ID.
        let pending = &processor.thread_processor.pending_thread_unloads;
        pending
            .lock()
            .await
            .insert(child_id, tokio::sync::watch::channel(()).0);
        assert!(
            futures::poll!(&mut attach).is_pending(),
            "attachment must wait for old cleanup"
        );
        pending.lock().await.remove(&child_id);
        attach.await;
        // Keep the spawn response open until the child's model request arrives, so
        // the next gated response belongs to the parent regardless of scheduling.
        timeout(
            Duration::from_secs(/*secs*/ 10),
            server.wait_for_request_count(/*count*/ 2),
        )
        .await?;
        release_spawn_completion
            .send(())
            .expect("spawn completion is waiting");
        timeout(
            Duration::from_secs(/*secs*/ 10),
            server.wait_for_request_count(/*count*/ 3),
        )
        .await?;

        // Installing an unsubscribed listener must not unload a still-running child.
        let premature_close = timeout(Duration::from_millis(/*millis*/ 1200), async {
            loop {
                if let OutgoingEnvelope::Broadcast {
                    message: OutgoingMessage::AppServerNotification(notification),
                } = outgoing.recv().await.expect("outgoing channel stays open")
                    && matches!(
                        notification.notification,
                        ServerNotification::ThreadClosed(_)
                    )
                {
                    break;
                }
            }
        })
        .await;
        assert!(premature_close.is_err(), "running threads must stay loaded");
        if ephemeral {
            // Complete the child while the parent's follow-up response remains gated.
            let child = processor
                .thread_processor
                .thread_manager
                .get_thread(child_id)
                .await?;
            release_first_completion
                .send(())
                .expect("child completion is waiting");
            timeout(Duration::from_secs(/*secs*/ 10), async {
                while !matches!(child.agent_status().await, AgentStatus::Completed(_)) {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .context("child did not complete")?;
            tokio::time::sleep(Duration::from_secs(unload_delay_secs + 1)).await;
            release_second_completion
                .send(())
                .expect("parent completion is waiting");

            // Exercise the parent's real followup_task call and inspect the child's next
            // model request: accepting a follow-up must preserve its earlier model context.
            timeout(
                Duration::from_secs(/*secs*/ 10),
                server.wait_for_request_count(/*count*/ 5),
            )
            .await
            .context("ephemeral child did not receive its follow-up")?;
            let requests = server.requests().await;
            let followup = requests[3..]
                .iter()
                .map(|request| serde_json::from_slice::<serde_json::Value>(request))
                .collect::<std::result::Result<Vec<_>, _>>()?
                .into_iter()
                .find(|request| {
                    request["input"]
                        .to_string()
                        .contains("Recall your previous reply")
                        && !request["input"].as_array().is_some_and(|input| {
                            input.iter().any(|item| {
                                item["type"] == "function_call_output" && item["call_id"] == "spawn"
                            })
                        })
                })
                .context("missing child follow-up request")?;
            let previous_reply = followup["input"]
                .as_array()
                .context("follow-up input")?
                .iter()
                .find(|item| item["role"] == "assistant")
                .context("child's previous reply must survive the idle deadline")?;
            assert_eq!(
                previous_reply["content"],
                json!([{"type": "output_text", "text": "Done"}])
            );
            return Ok(());
        }
        release_first_completion
            .send(())
            .expect("first completion is waiting");
        release_second_completion
            .send(())
            .expect("second completion is waiting");

        // Check both registry cleanup and runtime release, regardless of the order
        // in which the parent and child finish unloading.
        timeout(Duration::from_secs(/*secs*/ 10), async {
            loop {
                let loaded = processor
                    .thread_processor
                    .thread_manager
                    .list_thread_ids()
                    .await;
                if loaded.is_empty() && child_runtime.upgrade().is_none() {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .context("parent and disconnected child still retain their runtimes")?;
        Ok(())
    }
    .await;
    drop(outgoing);
    processor.clear_runtime_references();
    processor.shutdown_threads().await;
    server.shutdown().await;
    result
}

async fn send_request(
    processor: &Arc<MessageProcessor>,
    connection_id: ConnectionId,
    session: &Arc<ConnectionSessionState>,
    request: serde_json::Value,
) {
    processor
        .process_request(
            connection_id,
            serde_json::from_value(request).expect("JSON-RPC request"),
            &AppServerTransport::WebSocket {
                bind_address: "127.0.0.1:0".parse().expect("loopback address"),
            },
            Arc::clone(session),
        )
        .await;
}
