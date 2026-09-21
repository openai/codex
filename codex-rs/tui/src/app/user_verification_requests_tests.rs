use crate::app::app_server_requests::AppServerRequestResolution;
use crate::app::app_server_requests::PendingAppServerRequests;
use crate::app::app_server_requests::ResolvedAppServerRequest;
use crate::app_command::AppCommand;
use crate::app_command::UserVerificationResponse;
use codex_app_server_protocol::McpServerElicitationRequest;
use codex_app_server_protocol::McpServerElicitationRequestParams;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::UserVerificationProof;
use codex_protocol::ThreadId;
use pretty_assertions::assert_eq;

fn verification_request() -> ServerRequest {
    ServerRequest::McpServerElicitationRequest {
        request_id: RequestId::Integer(7),
        params: McpServerElicitationRequestParams {
            thread_id: ThreadId::new().to_string(),
            turn_id: None,
            server_name: "deployments".to_string(),
            request: McpServerElicitationRequest::UserVerification {
                meta: None,
                title: "Approve deployment?".to_string(),
                description: "Deploy the reviewed change.".to_string(),
                challenge: "AQID".to_string(),
            },
        },
    }
}

#[test]
fn user_verification_cancellation_invalidates_the_inflight_result() {
    let mut pending = PendingAppServerRequests::default();
    let request = verification_request();
    pending.note_server_request(&request);
    let attempt = pending
        .user_verification
        .begin("deployments", request.id())
        .expect("pending request");
    assert!(
        pending
            .user_verification
            .begin("deployments", request.id())
            .is_none()
    );
    assert!(
        pending
            .user_verification
            .is_pending("deployments", request.id(), attempt.id)
    );
    let resolution = pending
        .take_resolution(
            "unused",
            AppCommand::resolve_user_verification(
                "deployments".to_string(),
                request.id().clone(),
                UserVerificationResponse::Cancel,
            ),
        )
        .expect("cancel response");
    assert_eq!(
        resolution,
        Some(AppServerRequestResolution {
            request_id: request.id().clone(),
            result: serde_json::json!({ "action": "cancel", "content": null, "_meta": null }),
        })
    );
    assert!(
        !pending
            .user_verification
            .is_pending("deployments", request.id(), attempt.id)
    );
    assert!(attempt.cancelled.is_cancelled());
}

#[test]
fn discarded_thread_cancels_only_its_verification_attempts() {
    let mut pending = PendingAppServerRequests::default();
    let request = verification_request();
    let ServerRequest::McpServerElicitationRequest { params, .. } = &request else {
        unreachable!()
    };
    let discarded_thread_id = params.thread_id.clone();
    pending.note_server_request(&request);
    let discarded_attempt = pending
        .user_verification
        .begin("deployments", request.id())
        .expect("discarded thread attempt");

    let mut other_request = verification_request();
    let ServerRequest::McpServerElicitationRequest { request_id, params } = &mut other_request
    else {
        unreachable!()
    };
    *request_id = RequestId::Integer(8);
    params.thread_id = ThreadId::new().to_string();
    pending.note_server_request(&other_request);
    let other_attempt = pending
        .user_verification
        .begin("deployments", other_request.id())
        .expect("other thread attempt");

    pending.cancel_thread_verification(&discarded_thread_id);

    assert!(discarded_attempt.cancelled.is_cancelled());
    assert!(!pending.contains_server_request(&request));
    assert!(!pending.user_verification.is_pending(
        "deployments",
        request.id(),
        discarded_attempt.id
    ));
    assert!(!other_attempt.cancelled.is_cancelled());
    assert!(pending.contains_server_request(&other_request));
    assert!(pending.user_verification.is_pending(
        "deployments",
        other_request.id(),
        other_attempt.id
    ));
}

#[test]
fn user_verification_reconnect_cannot_accept_a_previous_connections_proof() {
    let mut pending = PendingAppServerRequests::default();
    let request = verification_request();
    pending.note_server_request(&request);
    let previous = pending
        .user_verification
        .begin("deployments", request.id())
        .expect("first request");
    pending.clear();
    assert!(previous.cancelled.is_cancelled());
    pending.note_server_request(&request);
    let current = pending
        .user_verification
        .begin("deployments", request.id())
        .expect("new request");
    assert!(
        !pending
            .user_verification
            .is_pending("deployments", request.id(), previous.id)
    );
    assert!(
        pending
            .user_verification
            .is_pending("deployments", request.id(), current.id)
    );
}

#[test]
fn user_verification_server_resolution_invalidates_the_inflight_result() {
    let mut pending = PendingAppServerRequests::default();
    let request = verification_request();
    pending.note_server_request(&request);
    let attempt = pending
        .user_verification
        .begin("deployments", request.id())
        .expect("pending request");
    assert_eq!(
        pending.resolve_notification("unused", request.id()),
        Some(ResolvedAppServerRequest::McpElicitation {
            server_name: "deployments".to_string(),
            request_id: request.id().clone()
        })
    );
    assert!(
        !pending
            .user_verification
            .is_pending("deployments", request.id(), attempt.id)
    );
    assert!(attempt.cancelled.is_cancelled());
}

#[test]
fn user_verification_proof_is_sent_to_the_server_but_not_session_recording() {
    let request = verification_request();
    let proof = UserVerificationProof {
        credential_id: "private-credential".to_string(),
        signature: "private-signature".to_string(),
    };
    let op = AppCommand::resolve_user_verification(
        "deployments".to_string(),
        request.id().clone(),
        UserVerificationResponse::Accept {
            proof: proof.clone(),
        },
    );
    assert_eq!(
        serde_json::to_value(&op).expect("recording payload"),
        serde_json::json!({
            "ResolveUserVerification": {
                "server_name": "deployments",
                "request_id": 7,
                "response": { "Accept": {} },
            },
        })
    );
    let mut pending = PendingAppServerRequests::default();
    pending.note_server_request(&request);
    let resolution = pending
        .take_resolution("unused", op)
        .expect("response should serialize")
        .expect("pending request");
    assert_eq!(
        resolution,
        AppServerRequestResolution {
            request_id: request.id().clone(),
            result: serde_json::json!({
                "action": "accept",
                "content": proof,
                "_meta": null,
            }),
        }
    );
}
