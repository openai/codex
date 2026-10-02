//! Exercises thread cleanup when required MCP startup outlives its requesting connection.

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
