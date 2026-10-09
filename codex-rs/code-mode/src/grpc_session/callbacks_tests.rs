//! Keeps late callback failures from disrupting cancellation and shutdown.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use http_body_util::Full;
use pretty_assertions::assert_eq;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tonic::body::Body;
use tonic::codegen::Bytes;
use tonic::codegen::http::Response;
use tower::service_fn;
use tower::util::BoxCloneSyncService;

use super::super::GrpcClient;
use super::super::state::SessionState;
use super::super::transport::SharedTransport;
use super::SessionInner;
use crate::remote_session::wait_for_watch;

#[tokio::test]
async fn missing_invocation_completion_racing_cancellation_preserves_session() {
    let cancellation = CancellationToken::new();
    let completion_cancellation = cancellation.clone();
    let client = GrpcClient::new(BoxCloneSyncService::new(service_fn(move |_request| {
        // Cancel after the biased select has polled cancellation, before the RPC fails.
        completion_cancellation.cancel();
        async { Ok(tonic::Status::not_found("unknown code-mode tool invocation").into_http()) }
    })));
    let session = session(client);

    session
        .complete_tool_call(
            "invocation".to_string(),
            cancellation,
            Ok(serde_json::Value::Null),
        )
        .await;

    assert_eq!(session.require_open(), Ok(()));
}

fn session(client: GrpcClient) -> Arc<SessionInner> {
    Arc::new(SessionInner {
        id: "session".to_string(),
        client: client.clone(),
        route: None,
        runtime: tokio::runtime::Handle::current(),
        state: Mutex::new(SessionState::default()),
        wait_slots: Mutex::new(HashMap::new()),
        shutdown_requested: AtomicBool::new(/*v*/ false),
        shutdown_result: Mutex::new(/*t*/ None),
        stopped: CancellationToken::new(),
        stream_tasks: TaskTracker::new(),
        _transport: Arc::new(SharedTransport::Connected(client)),
    })
}

#[tokio::test(start_paused = true)]
async fn shutdown_is_ordered_with_session_failure() {
    let close_started = Arc::new(Notify::new());
    let finish_close = Arc::new(Notify::new());
    let started = Arc::clone(&close_started);
    let finish = Arc::clone(&finish_close);
    let client = GrpcClient::new(BoxCloneSyncService::new(service_fn(move |_request| {
        let started = Arc::clone(&started);
        let finish = Arc::clone(&finish);
        async move {
            started.notify_one();
            finish.notified().await;
            Ok(Response::builder()
                .header("content-type", "application/grpc")
                .header("grpc-status", "0")
                // An empty protobuf CloseSessionResponse in a gRPC frame.
                .body(Body::new(Full::new(Bytes::from_static(b"\0\0\0\0\0"))))
                .expect("close response"))
        }
    })));
    let closing = session(client.clone());
    let result = closing.request_shutdown();
    tokio::time::timeout(Duration::from_secs(5), close_started.notified())
        .await
        .expect("CloseSession started");

    // A callback can decide to fail just before shutdown starts, then reach
    // fail() after CloseSession is in flight. Recheck at the state transition.
    closing.fail("late callback failure".to_string());
    assert!(!closing.stopped.is_cancelled());
    finish_close.notify_one();
    assert_eq!(wait_for_watch(result).await, Ok(()));

    // If failure wins first, shutdown observes a closed session and skips the RPC.
    let failed = session(client);
    failed.fail("earlier callback failure".to_string());
    assert!(failed.stopped.is_cancelled());
    assert_eq!(wait_for_watch(failed.request_shutdown()).await, Ok(()));
}
