#![cfg(test)]
//! Negotiated callback budgets protect older clients on a shared host.

use codex_code_mode_protocol::grpc as proto;
use codex_code_mode_protocol::grpc::code_mode_host_server::CodeModeHost;
use codex_code_mode_protocol::host::LEGACY_MAX_PENDING_DELEGATE_CALLS;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use tonic::Code;
use tonic::Request;

use super::GrpcCodeModeHost;
use super::tests::open_session;

#[tokio::test]
async fn client_callback_capacity_is_isolated_and_reusable() {
    let host = GrpcCodeModeHost::new();
    let (legacy_id, _legacy_lease) = open_session(&host).await;
    let legacy = host.state.session(&legacy_id).expect("legacy session");
    let legacy_permits = (0..LEGACY_MAX_PENDING_DELEGATE_CALLS)
        .map(|_| legacy.delegate_permit().expect("legacy callback"))
        .collect::<Vec<_>>();
    assert_eq!(
        legacy.delegate_permit().unwrap_err(),
        "code-mode session has too many pending delegate calls"
    );

    let mut lease = host
        .open_session(Request::new(proto::OpenSessionRequest {
            max_pending_delegate_calls: Some(2),
            ..Default::default()
        }))
        .await
        .expect("open capacity-limited session")
        .into_inner();
    let event = lease.next().await.expect("opened event").expect("event");
    let Some(proto::session_event::Event::Opened(opened)) = event.event else {
        panic!("expected session opening");
    };
    let session = host.state.session(&opened.session_id).expect("session");
    let first = session.delegate_permit().expect("first callback");
    let second = session.delegate_permit().expect("second callback");
    assert_eq!(
        session.delegate_permit().unwrap_err(),
        "code-mode session has too many pending delegate calls"
    );

    drop(first);
    let replacement = session.delegate_permit().expect("reused callback capacity");
    drop(legacy_permits);
    let legacy_replacement = legacy.delegate_permit().expect("reused legacy capacity");
    drop((second, replacement, legacy_replacement));
    host.close_session(Request::new(proto::CloseSessionRequest {
        session_id: opened.session_id,
    }))
    .await
    .expect("close new session");
    host.close_session(Request::new(proto::CloseSessionRequest {
        session_id: legacy_id,
    }))
    .await
    .expect("close legacy session");
}

#[tokio::test]
async fn zero_callback_capacity_is_rejected_before_opening_a_session() {
    let host = GrpcCodeModeHost::new();
    let result = host
        .open_session(Request::new(proto::OpenSessionRequest {
            max_pending_delegate_calls: Some(0),
            ..Default::default()
        }))
        .await;
    let Err(error) = result else {
        panic!("zero callback capacity must be rejected");
    };
    assert_eq!(error.code(), Code::InvalidArgument);
}
