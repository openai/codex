//! Exercises cancellation and local-device security through the client JSON-RPC dispatcher.

use super::test_support::Harness;
use super::test_support::write_auth;
use super::*;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::OutgoingMessage;
use anyhow::Result;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::atomic::Ordering;
use tokio::time::timeout;

#[tokio::test]
async fn user_verification_rpc_enroll_returns_public_registration_metadata() -> Result<()> {
    let mut h = Harness::new(ConnectionOrigin::Stdio, || true).await?;
    h.initialize("test-local-ui", /*opt_in*/ true).await;
    h.send(/*id*/ 1, "userVerification/enroll", json!({})).await;
    let OutgoingMessage::Response(response) = h.response().await else {
        panic!("local enrollment must return public metadata")
    };
    assert_eq!(
        serde_json::to_value(response.result)?,
        json!({
            "credentialId": "credential",
            "algorithm": "ecdsaP256Sha256X962",
            "publicKey": "public-key"
        })
    );
    h.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn user_verification_rpc_auth_revision_discards_native_proof() -> Result<()> {
    let mut h = Harness::new(ConnectionOrigin::Stdio, || true).await?;
    h.initialize("test-local-ui", /*opt_in*/ true).await;
    h.start_verify().await?;
    write_auth(h.home.path(), "second")?;
    h.auth.reload().await;
    h.provider.released.store(/*val*/ true, Ordering::Release);
    let OutgoingMessage::Error(error) = h.response().await else {
        panic!("auth change must reject proof")
    };
    assert_eq!(
        error.error.data,
        Some(json!({"type": "cancelled", "reason": "interrupted"}))
    );
    h.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn user_verification_rpc_disconnect_cancels_worker_and_releases_slot() -> Result<()> {
    let mut h = Harness::new(ConnectionOrigin::Stdio, || true).await?;
    h.initialize("test-local-ui", /*opt_in*/ true).await;
    h.start_verify().await?;
    timeout(
        Duration::from_secs(/*secs*/ 5),
        h.processor.connection_closed(ConnectionId(1), &h.session),
    )
    .await?;
    assert!(matches!(h.response().await, OutgoingMessage::Error(_)));
    assert_eq!(h.service.worker.available_permits(), 1);
    h.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn user_verification_rpc_network_peers_cannot_use_server_keys() -> Result<()> {
    for origin in [ConnectionOrigin::WebSocket, ConnectionOrigin::RemoteControl] {
        let mut h = Harness::new(origin, || true).await?;
        h.initialize("codex-tui", /*opt_in*/ true).await;
        for method in [
            "userVerification/enroll",
            "userVerification/delete",
            "userVerification/verify",
        ] {
            let params = if method.ends_with("verify") {
                json!({"challenge":"AQ", "title":"Approve", "description":""})
            } else {
                json!({})
            };
            h.send(/*id*/ 1, method, params).await;
            let OutgoingMessage::Error(error) = h.response().await else {
                panic!("remote native operation must fail")
            };
            assert_eq!(
                error.error.data,
                Some(json!({"type":"unavailable", "reason":"providerUnavailable"}))
            );
        }
        assert_eq!(h.provider.calls.load(Ordering::SeqCst), 0);
        h.shutdown().await;
    }
    Ok(())
}

#[tokio::test]
async fn user_verification_rpc_dropped_caller_cancels_native_work() -> Result<()> {
    let h = Harness::new(ConnectionOrigin::InProcess, || true).await?;
    let (entered, waiting) = oneshot::channel();
    *h.provider.entered.lock().unwrap() = Some(entered);
    let future = h.service.handle(
        Operation::Verify(rpc::UserVerificationVerifyParams {
            challenge: "AQ".into(),
            title: "Approve".into(),
            description: String::new(),
        }),
        Arc::clone(&h.session.rpc_gate),
        CancellationToken::new(),
        ConnectionOrigin::InProcess,
    );
    {
        let future = std::pin::pin!(future);
        tokio::select! { result = future => panic!("native operation should block: {}", result.is_ok()), entered = waiting => entered? }
    }
    let permit = timeout(Duration::from_secs(/*secs*/ 5), h.service.worker.acquire()).await??;
    drop(permit);
    assert_eq!(h.provider.calls.load(Ordering::SeqCst), 1);
    h.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn user_verification_rechecks_auth_after_response_queue_wait() -> Result<()> {
    let mut h = Harness::new(ConnectionOrigin::Stdio, || true).await?;
    h.provider.released.store(/*val*/ true, Ordering::Release);
    let response = h
        .service
        .handle(
            Operation::Verify(rpc::UserVerificationVerifyParams {
                challenge: "AQ".into(),
                title: "Approve".into(),
                description: String::new(),
            }),
            Arc::clone(&h.session.rpc_gate),
            CancellationToken::new(),
            ConnectionOrigin::Stdio,
        )
        .await
        .map_err(|error| anyhow::anyhow!("{error:?}"))?;
    for id in 0..16 {
        h.outgoing
            .send_response(
                crate::outgoing_message::ConnectionRequestId {
                    connection_id: ConnectionId(1),
                    request_id: rpc::RequestId::Integer(id),
                },
                rpc::UserVerificationDeleteResponse {},
            )
            .await;
    }
    let outgoing = Arc::clone(&h.outgoing);
    let (payload, check) = response.into_parts();
    let sending = outgoing.send_response_as_checked(
        crate::outgoing_message::ConnectionRequestId {
            connection_id: ConnectionId(1),
            request_id: rpc::RequestId::Integer(16),
        },
        payload,
        check,
    );
    let mut sending = std::pin::pin!(sending);
    assert!(matches!(
        futures::poll!(&mut sending),
        std::task::Poll::Pending
    ));
    write_auth(h.home.path(), "second")?;
    h.auth.reload().await;
    h.response().await;
    sending.await;
    for _ in 1..16 {
        h.response().await;
    }
    let OutgoingMessage::Error(error) = h.response().await else {
        panic!("queued response must recheck auth")
    };
    assert_eq!(
        error.error.data,
        Some(json!({"type":"cancelled", "reason":"interrupted"}))
    );
    h.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn user_verification_rejects_concurrent_native_workers() -> Result<()> {
    let h = Harness::new(ConnectionOrigin::Stdio, || true).await?;
    let permit = h.service.worker.clone().acquire_owned().await?;
    let result = h
        .service
        .handle(
            Operation::Status,
            Arc::clone(&h.session.rpc_gate),
            CancellationToken::new(),
            ConnectionOrigin::Stdio,
        )
        .await;
    assert_eq!(
        result.err().unwrap().message,
        "Another user verification operation is still running."
    );
    assert_eq!(h.provider.calls.load(Ordering::SeqCst), 0);
    drop(permit);
    h.shutdown().await;
    Ok(())
}
