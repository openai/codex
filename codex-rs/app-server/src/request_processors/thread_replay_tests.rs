#![cfg(test)]
//! Checks stalled thread resume replay against the bounded outbound router.

use super::super::thread_processor::ThreadResumeTarget;
use super::listener_shutdown_tests::test_processor;
use super::*;
use crate::outgoing_message::OutgoingMessage;
use crate::thread_state::ConnectionCapabilities;
use crate::transport::CHANNEL_CAPACITY;
use crate::transport::OutboundConnectionState;
use crate::transport::QueuedOutgoingMessage;
use crate::transport::route_outgoing_envelope;
use anyhow::Result;
use codex_app_server_protocol::DynamicToolCallParams;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerRequestPayload;
use codex_app_server_protocol::ThreadResumeParams;
use codex_thread_store::PersistContext;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashSet;
use std::sync::RwLock;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

async fn next_message(writer: &mut mpsc::Receiver<QueuedOutgoingMessage>) -> QueuedOutgoingMessage {
    timeout(Duration::from_secs(5), async {
        loop {
            let queued = writer.recv().await.expect("connection closed");
            if !matches!(queued.message, OutgoingMessage::AppServerNotification(_)) {
                return queued;
            }
        }
    })
    .await
    .expect("expected resume response or replayed request")
}

#[tokio::test]
async fn stalled_replay_can_be_replaced_or_unsubscribed() -> Result<()> {
    let server = wiremock::MockServer::start().await;
    let test = test_codex().build_with_auto_env(&server).await?;
    let thread_id = test.session_configured.thread_id;
    test.thread_store
        .persist_thread(thread_id, PersistContext::Standard)
        .await?;
    let (processor, mut outgoing_rx) = test_processor(&test).await?;
    let connection_id = ConnectionId(1);
    processor
        .connection_initialized(connection_id, ConnectionCapabilities::default())
        .await;
    let outgoing = &processor.outgoing;
    outgoing.connection_initialized(connection_id).await;
    let mut expected = Vec::new();
    for index in 0..CHANNEL_CAPACITY + 32 {
        let (id, _callback) = outgoing
            .send_request_to_connections(
                Some(&[]),
                ServerRequestPayload::DynamicToolCall(DynamicToolCallParams {
                    thread_id: thread_id.to_string(),
                    turn_id: "turn".into(),
                    call_id: index.to_string(),
                    namespace: None,
                    tool: "update_up_next".into(),
                    arguments: json!({}),
                }),
                Some(thread_id),
            )
            .await;
        expected.push(id);
    }
    let (writer_tx, mut writer_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let disconnect = CancellationToken::new();
    let router = tokio::spawn({
        let disconnect = disconnect.clone();
        async move {
            let mut connections = HashMap::from([(
                connection_id,
                OutboundConnectionState::new(
                    writer_tx,
                    Arc::new(AtomicBool::new(true)),
                    Arc::new(AtomicBool::new(true)),
                    Arc::new(RwLock::new(HashSet::new())),
                    Some(disconnect),
                ),
            )]);
            while let Some(envelope) = outgoing_rx.recv().await {
                route_outgoing_envelope(&mut connections, envelope).await;
            }
        }
    });
    let resume = async |id, writer: &mut mpsc::Receiver<QueuedOutgoingMessage>| {
        timeout(
            Duration::from_secs(10),
            processor.thread_resume(
                ThreadResumeTarget::Client(ConnectionRequestId {
                    connection_id,
                    request_id: RequestId::Integer(id),
                }),
                ThreadResumeParams {
                    thread_id: thread_id.to_string(),
                    exclude_turns: true,
                    ..Default::default()
                },
                /*app_server_client_name*/ None,
                /*app_server_client_version*/ None,
                /*client_mcp_extensions*/ Default::default(),
            ),
        )
        .await
        .unwrap()
        .unwrap();
        let response = next_message(writer).await;
        assert!(
            matches!(response.message, OutgoingMessage::Response(r) if r.id == RequestId::Integer(id))
        );
    };
    resume(1, &mut writer_rx).await;
    let first = next_message(&mut writer_rx).await;
    let OutgoingMessage::Request(request) = first.message else {
        panic!("expected replay")
    };
    assert_eq!(request.id(), &expected[0]);
    // Resume must succeed while the first replay is still waiting on this receipt.
    resume(2, &mut writer_rx).await;
    let stall = Duration::from_millis(50);
    assert!(timeout(stall, next_message(&mut writer_rx)).await.is_err());
    first.write_complete_tx.unwrap().send(()).unwrap();
    for id in &expected {
        let queued = next_message(&mut writer_rx).await;
        let OutgoingMessage::Request(request) = queued.message else {
            panic!("expected replay")
        };
        assert_eq!(request.id(), id);
        // Losing a socket write receipt during reconnect must also let replay advance.
        drop(queued.write_complete_tx.unwrap());
    }
    assert!(!disconnect.is_cancelled());

    resume(3, &mut writer_rx).await;
    let first = next_message(&mut writer_rx).await;
    assert!(
        processor
            .thread_state_manager
            .unsubscribe_connection_from_thread(thread_id, connection_id)
            .await
    );
    drop(first.write_complete_tx.unwrap());
    assert!(timeout(stall, next_message(&mut writer_rx)).await.is_err());

    let replay = tokio::spawn({
        let outgoing = Arc::clone(outgoing);
        async move {
            outgoing
                .replay_requests_to_connection(
                    connection_id,
                    outgoing.pending_requests_for_thread(thread_id).await,
                )
                .await
        }
    });
    let held = next_message(&mut writer_rx).await;
    outgoing.stop_connection_replays(connection_id).await;
    timeout(Duration::from_secs(5), replay)
        .await
        .unwrap()
        .unwrap();
    drop(held);
    router.abort();
    Ok(())
}
