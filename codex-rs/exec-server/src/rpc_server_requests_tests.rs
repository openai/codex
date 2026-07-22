use std::sync::Arc;
use std::time::Duration;

use codex_exec_server_protocol::JSONRPCRequest;
use codex_exec_server_protocol::RequestId;
use pretty_assertions::assert_eq;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio::time::timeout;

use super::MAX_IN_FLIGHT_SERVER_CALLS;
use super::RpcServerRequestSender;
use crate::rpc::RpcCallError;
use crate::rpc::RpcServerOutboundMessage;

async fn receive_server_request(
    outgoing_rx: &mut mpsc::Receiver<RpcServerOutboundMessage>,
) -> JSONRPCRequest {
    let message = timeout(Duration::from_secs(1), outgoing_rx.recv())
        .await
        .expect("server request should arrive")
        .expect("server request");
    match message {
        RpcServerOutboundMessage::Request(request) => request,
        other => panic!("expected server request, got {other:?}"),
    }
}

impl RpcServerRequestSender {
    pub(crate) fn pending_request_count(&self) -> usize {
        self.inner
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }
}

#[tokio::test]
async fn rpc_server_sender_matches_out_of_order_responses_by_request_id() {
    let (outgoing_tx, mut outgoing_rx) = mpsc::channel(/*buffer*/ 8);
    let requests = RpcServerRequestSender::new(outgoing_tx);
    let slow_requests = requests.clone();
    let slow = tokio::spawn(async move {
        slow_requests
            .call_with_timeout::<_, serde_json::Value>(
                "slow",
                &serde_json::json!({ "n": 1 }),
                Duration::from_secs(1),
            )
            .await
    });
    let fast_requests = requests.clone();
    let fast = tokio::spawn(async move {
        fast_requests
            .call_with_timeout::<_, serde_json::Value>(
                "fast",
                &serde_json::json!({ "n": 2 }),
                Duration::from_secs(1),
            )
            .await
    });

    let first = receive_server_request(&mut outgoing_rx).await;
    let second = receive_server_request(&mut outgoing_rx).await;
    let (slow_request, fast_request) = if first.method == "slow" {
        (first, second)
    } else {
        (second, first)
    };
    requests.complete(fast_request.id, Ok(serde_json::json!({ "value": "fast" })));
    requests.complete(slow_request.id, Ok(serde_json::json!({ "value": "slow" })));

    assert_eq!(
        slow.await.expect("slow task").expect("slow server request"),
        serde_json::json!({ "value": "slow" })
    );
    assert_eq!(
        fast.await.expect("fast task").expect("fast server request"),
        serde_json::json!({ "value": "fast" })
    );
    assert_eq!(requests.pending_request_count(), 0);
}

#[tokio::test]
async fn rpc_server_sender_preserves_response_received_before_close() {
    let (outgoing_tx, mut outgoing_rx) = mpsc::channel(/*buffer*/ 1);
    let requests = RpcServerRequestSender::new(outgoing_tx);
    let caller = requests.clone();
    let call = tokio::spawn(async move {
        caller
            .call_with_timeout::<_, serde_json::Value>(
                "ordered",
                &serde_json::json!({}),
                Duration::from_secs(1),
            )
            .await
    });

    let request = receive_server_request(&mut outgoing_rx).await;
    requests.complete(request.id, Ok(serde_json::json!({ "value": "accepted" })));
    requests.close();

    assert_eq!(
        call.await
            .expect("server request task should join")
            .expect("server request should preserve its response"),
        serde_json::json!({ "value": "accepted" })
    );
    assert_eq!(requests.pending_request_count(), 0);
}

#[tokio::test(start_paused = true)]
async fn rpc_server_sender_timeout_removes_pending_request() {
    let (outgoing_tx, mut outgoing_rx) = mpsc::channel(/*buffer*/ 1);
    let requests = RpcServerRequestSender::new(outgoing_tx);
    let call_timeout = Duration::from_secs(1);
    let params = serde_json::json!({});
    let call = requests.call_with_timeout::<_, serde_json::Value>("slow", &params, call_timeout);
    tokio::pin!(call);
    assert!(futures::poll!(call.as_mut()).is_pending());
    let request = receive_server_request(&mut outgoing_rx).await;

    tokio::time::advance(call_timeout).await;
    assert!(matches!(
        call.await,
        Err(RpcCallError::TimedOut { method, timeout })
            if method == "slow" && timeout == call_timeout
    ));
    assert_eq!(requests.pending_request_count(), 0);
    assert!(requests.complete(request.id, Ok(serde_json::Value::Null)));
    assert!(!requests.complete(RequestId::Integer(2), Ok(serde_json::Value::Null)));
}

#[tokio::test]
async fn rpc_server_sender_bounds_and_drains_pending_requests_on_close() {
    let (outgoing_tx, mut outgoing_rx) = mpsc::channel(MAX_IN_FLIGHT_SERVER_CALLS);
    let requests = Arc::new(RpcServerRequestSender::new(outgoing_tx));
    let mut calls = JoinSet::new();
    for index in 0..MAX_IN_FLIGHT_SERVER_CALLS {
        let requests = Arc::clone(&requests);
        calls.spawn(async move {
            requests
                .call_with_timeout::<_, serde_json::Value>(
                    "pending",
                    &serde_json::json!({ "index": index }),
                    Duration::from_secs(30),
                )
                .await
        });
    }
    for _ in 0..MAX_IN_FLIGHT_SERVER_CALLS {
        receive_server_request(&mut outgoing_rx).await;
    }
    assert_eq!(requests.pending_request_count(), MAX_IN_FLIGHT_SERVER_CALLS);

    let overflow = requests
        .call_with_timeout::<_, serde_json::Value>(
            "overflow",
            &serde_json::json!({}),
            Duration::from_secs(1),
        )
        .await;
    assert!(matches!(
        overflow,
        Err(RpcCallError::PendingRequestLimitExceeded { limit })
            if limit == MAX_IN_FLIGHT_SERVER_CALLS
    ));

    requests.close();
    assert_eq!(requests.pending_request_count(), 0);
    while let Some(call) = calls.join_next().await {
        assert!(matches!(
            call.expect("pending server request task should join"),
            Err(RpcCallError::Closed)
        ));
    }
}
