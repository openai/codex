//! Exercises ownership through real initialized JSON-RPC sessions and disconnect processing.

use super::test_support::Harness;
use super::test_support::write_auth;
use crate::message_processor::ConnectionSessionState;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::OutgoingEnvelope;
use crate::outgoing_message::OutgoingMessage;
use crate::transport::AppServerTransport;
use crate::transport::ConnectionOrigin;
use anyhow::Result;
use codex_app_server_protocol as rpc;
use codex_protocol::ThreadId;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::time::Duration;
use tokio::time::timeout;

fn request() -> rpc::ServerRequestPayload {
    rpc::ServerRequestPayload::McpServerElicitationRequest(rpc::McpServerElicitationRequestParams {
        thread_id: ThreadId::new().to_string(),
        turn_id: None,
        server_name: "codex_apps".into(),
        request: rpc::McpServerElicitationRequest::UserVerification {
            meta: None,
            title: "Approve".into(),
            description: String::new(),
            challenge: "AQ".into(),
        },
    })
}

async fn initialize_second(h: &mut Harness) -> Result<Arc<ConnectionSessionState>> {
    let session = Arc::new(ConnectionSessionState::new(ConnectionOrigin::InProcess));
    h.processor
        .process_request(
            ConnectionId(2),
            rpc::JSONRPCRequest {
                id: rpc::RequestId::Integer(0),
                method: "initialize".into(),
                params: Some(json!({"clientInfo":{"name":"codex-tui","version":"1"},"capabilities":{"experimentalApi":true}})),
                trace: None,
            },
            &AppServerTransport::Stdio,
            Arc::clone(&session),
        )
        .await;
    let message = timeout(Duration::from_secs(/*secs*/ 5), h.messages.recv()).await?;
    assert!(matches!(
        message,
        Some(OutgoingEnvelope::ToConnection {
            connection_id: ConnectionId(2),
            message: OutgoingMessage::Response(_),
            ..
        })
    ));
    Ok(session)
}

#[tokio::test]
async fn user_verification_disconnect_releases_ownership_before_rpc_drain() -> Result<()> {
    let mut h = Harness::new(ConnectionOrigin::InProcess, || true).await?;
    h.initialize("codex-tui", /*opt_in*/ true).await;
    let second = initialize_second(&mut h).await?;
    let connections = [ConnectionId(1), ConnectionId(2)];
    let (id, pending) = h
        .outgoing
        .send_request_to_connections(Some(&connections), request(), /*thread_id*/ None)
        .await;
    assert!(matches!(h.response().await, OutgoingMessage::Request(_)));
    let proof = json!({"action":"accept","content":{"credentialId":"AQ","signature":"Ag"}});
    h.processor
        .process_response(
            ConnectionId(2),
            rpc::JSONRPCResponse {
                id: id.clone(),
                result: proof.clone(),
            },
        )
        .await;
    let mut pending = std::pin::pin!(pending);
    assert!(matches!(
        futures::poll!(&mut pending),
        std::task::Poll::Pending
    ));

    // Keep an unrelated initialized handler active so production disconnect must wait at the gate.
    let gate = Arc::clone(&h.session.rpc_gate);
    let (entered, waiting) = oneshot::channel();
    let (release, blocked) = oneshot::channel();
    let running = tokio::spawn(async move {
        gate.run(async move {
            entered.send(()).expect("handler entry");
            blocked.await.expect("release handler");
        })
        .await;
    });
    waiting.await?;
    let processor = Arc::clone(&h.processor);
    let session = Arc::clone(&h.session);
    let mut disconnect = std::pin::pin!(processor.connection_closed(ConnectionId(1), &session));
    assert!(matches!(
        futures::poll!(&mut disconnect),
        std::task::Poll::Pending
    ));
    assert!(
        timeout(Duration::from_secs(/*secs*/ 5), pending)
            .await?
            .is_err()
    );

    // The live initialized UI must now own the request, although the disconnected RPC is draining.
    let (id, response) = h
        .outgoing
        .send_request_to_connections(Some(&connections), request(), /*thread_id*/ None)
        .await;
    let message = timeout(Duration::from_secs(/*secs*/ 5), h.messages.recv()).await?;
    assert!(matches!(
        message,
        Some(OutgoingEnvelope::ToConnection {
            connection_id: ConnectionId(2),
            message: OutgoingMessage::Request(_),
            ..
        })
    ));
    h.processor
        .process_response(
            ConnectionId(1),
            rpc::JSONRPCResponse {
                id: id.clone(),
                result: proof.clone(),
            },
        )
        .await;
    let mut response = std::pin::pin!(response);
    assert!(matches!(
        futures::poll!(&mut response),
        std::task::Poll::Pending
    ));
    h.processor
        .process_response(
            ConnectionId(2),
            rpc::JSONRPCResponse {
                id,
                result: proof.clone(),
            },
        )
        .await;
    assert_eq!(response.await?, Ok(proof));
    release.send(()).expect("release draining handler");
    running.await?;
    disconnect.await;
    h.processor
        .connection_closed(ConnectionId(2), &second)
        .await;
    h.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn user_verification_dispatcher_auth_watcher_cancels_pending_ownership() -> Result<()> {
    let mut h = Harness::new(ConnectionOrigin::InProcess, || true).await?;
    h.initialize("codex-tui", /*opt_in*/ true).await;
    let (_, pending) = h
        .outgoing
        .send_request_to_connections(Some(&[ConnectionId(1)]), request(), /*thread_id*/ None)
        .await;
    assert!(matches!(h.response().await, OutgoingMessage::Request(_)));
    write_auth(h.home.path(), "second")?;
    h.auth.reload().await;
    // No response triggers a revision recheck: MessageProcessor's installed watcher must cancel it.
    assert!(
        timeout(Duration::from_secs(/*secs*/ 5), pending)
            .await?
            .is_err()
    );
    h.shutdown().await;
    Ok(())
}
