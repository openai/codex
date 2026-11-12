//! Checks cancellation ownership and the lifetime of proofs waiting for outbound capacity.

use super::*;
use codex_app_server_protocol::UserVerificationDeleteResponse;
use codex_app_server_protocol::UserVerificationProof;
use codex_app_server_protocol::UserVerificationVerifyResponse;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn user_verification_cancel_only_signals_matching_native_request_contexts() {
    let (tx, _rx) = mpsc::channel(/*buffer*/ 1);
    let outgoing = OutgoingMessageSender::new(tx, AnalyticsEventsClient::disabled());
    for method in [
        "userVerification/status",
        "userVerification/enroll",
        "userVerification/delete",
        "userVerification/verify",
        "userVerification/cancel",
        "turn/start",
    ] {
        let target = ConnectionRequestId {
            connection_id: ConnectionId(1),
            request_id: RequestId::Integer(7),
        };
        let context = RequestContext::new(
            target.clone(),
            method,
            Span::none(),
            /*parent_trace*/ None,
        );
        let cancellation = context.cancellation.clone();
        outgoing.register_request_context(context).await;
        for wrong_target in [
            ConnectionRequestId {
                connection_id: ConnectionId(2),
                request_id: RequestId::Integer(7),
            },
            ConnectionRequestId {
                connection_id: ConnectionId(1),
                request_id: RequestId::String("7".into()),
            },
            ConnectionRequestId {
                connection_id: ConnectionId(1),
                request_id: RequestId::Integer(8),
            },
        ] {
            outgoing
                .cancel_user_verification_request(&wrong_target)
                .await;
            assert!(!cancellation.is_cancelled());
        }
        outgoing.cancel_user_verification_request(&target).await;
        outgoing.cancel_user_verification_request(&target).await;
        assert_eq!(
            cancellation.is_cancelled(),
            method != "turn/start" && method != "userVerification/cancel"
        );
        assert_eq!(outgoing.request_context_count().await, 1);
        outgoing.connection_closed(ConnectionId(1)).await;
        assert_eq!(outgoing.request_context_count().await, 0);
    }
}

#[tokio::test]
async fn user_verification_cancel_remains_effective_until_proof_enqueue_and_cleans_up() {
    for close_receiver in [false, true] {
        let (tx, mut rx) = mpsc::channel(/*buffer*/ 1);
        let outgoing = OutgoingMessageSender::new(tx, AnalyticsEventsClient::disabled());
        let target = ConnectionRequestId {
            connection_id: ConnectionId(1),
            request_id: RequestId::String("verify".into()),
        };
        outgoing
            .send_response(target.clone(), UserVerificationDeleteResponse {})
            .await;
        let context = RequestContext::new(
            target.clone(),
            "userVerification/verify",
            Span::none(),
            /*parent_trace*/ None,
        );
        let cancellation = context.cancellation.clone();
        outgoing.register_request_context(context).await;
        let response = UserVerificationVerifyResponse {
            biometric_type: None,
            proof: UserVerificationProof {
                credential_id: "AQ".into(),
                signature: "Ag".into(),
            },
        };
        let error = internal_error("canceled proof");
        let expected_error = error.clone();
        let sending =
            outgoing.send_response_as_checked(target.clone(), response.into(), move || {
                if cancellation.is_cancelled() {
                    Err(error)
                } else {
                    Ok(())
                }
            });
        let mut sending = std::pin::pin!(sending);
        assert!(matches!(
            futures::poll!(&mut sending),
            std::task::Poll::Pending
        ));
        assert_eq!(outgoing.request_context_count().await, 1);
        outgoing.cancel_user_verification_request(&target).await;
        if close_receiver {
            rx.close();
        } else {
            rx.recv().await.unwrap();
        }
        sending.await;
        assert_eq!(outgoing.request_context_count().await, 0);
        if !close_receiver {
            let OutgoingEnvelope::ToConnection {
                message: OutgoingMessage::Error(error),
                ..
            } = rx.recv().await.unwrap()
            else {
                panic!("a proof canceled while queued must not be sent")
            };
            assert_eq!(error.id, target.request_id);
            assert_eq!(error.error, expected_error);
        }
    }
}
