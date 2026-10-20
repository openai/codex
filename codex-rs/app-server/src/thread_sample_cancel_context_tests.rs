//! Checks that sample cancellation only affects matching requests on the same connection.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn thread_sample_cancel_only_signals_matching_request_contexts() {
    let (tx, _rx) = mpsc::channel(/*buffer*/ 1);
    let outgoing = OutgoingMessageSender::new(tx, AnalyticsEventsClient::disabled());
    for method in [
        "thread/sample",
        "thread/sample/cancel",
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
            outgoing.cancel_thread_sample_request(&wrong_target).await;
            assert!(!cancellation.is_cancelled());
        }
        outgoing.cancel_thread_sample_request(&target).await;
        outgoing.cancel_thread_sample_request(&target).await;
        assert_eq!(cancellation.is_cancelled(), method == "thread/sample");
        assert_eq!(outgoing.request_context_count().await, 1);
        outgoing.connection_closed(ConnectionId(1)).await;
        assert_eq!(outgoing.request_context_count().await, 0);
    }
}
