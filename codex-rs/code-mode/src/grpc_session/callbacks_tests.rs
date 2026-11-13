//! Keeps late callback failures from disrupting cancellation and shutdown.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use codex_code_mode_protocol::CodeModeOutputTruncation;
use codex_code_mode_protocol::grpc;
use http_body_util::Full;
use pretty_assertions::assert_eq;
use tokio::sync::Notify;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tonic::body::Body;
use tonic::codegen::Bytes;
use tonic::codegen::http::Response;
use tower::service_fn;
use tower::util::BoxCloneSyncService;

use super::super::GrpcClient;
use super::super::MAX_IN_FLIGHT_COMPLETIONS;
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

#[tokio::test]
async fn tool_call_heartbeats_do_not_enter_callback_admission() {
    let client = GrpcClient::new(BoxCloneSyncService::new(service_fn(|_request| async {
        panic!("heartbeat must not send an RPC");
    })));
    let session = session(client);
    let heartbeat = grpc::ToolCall {
        session_id: session.id.clone(),
        heartbeat: Some(grpc::StreamHeartbeat {}),
        ..Default::default()
    };
    assert_eq!(session.handle_tool_call(heartbeat.clone()), Ok(()));
    assert_eq!(
        session.handle_tool_call(grpc::ToolCall {
            invocation_id: "not-a-heartbeat".to_string(),
            ..heartbeat
        }),
        Err("gRPC code-mode heartbeat carried a tool invocation".to_string())
    );
}

fn session(client: GrpcClient) -> Arc<SessionInner> {
    let client = Arc::new(client);
    Arc::new(SessionInner {
        id: "session".to_string(),
        output_truncation: CodeModeOutputTruncation::Client,
        route: None,
        runtime: tokio::runtime::Handle::current(),
        state: Mutex::new(SessionState::with_client(Arc::clone(&client))),
        wait_slots: Mutex::new(HashMap::new()),
        shutdown_requested: AtomicBool::new(/*v*/ false),
        shutdown_result: Mutex::new(/*t*/ None),
        stopped: CancellationToken::new(),
        stream_tasks: TaskTracker::new(),
        completion_permits: Arc::new(Semaphore::new(MAX_IN_FLIGHT_COMPLETIONS)),
        _transport: Arc::new(SharedTransport::Connected(client)),
    })
}

#[tokio::test]
async fn cancelled_completion_does_not_wait_for_delivery_capacity() {
    let client = GrpcClient::new(BoxCloneSyncService::new(service_fn(|_request| async {
        Ok(tonic::Status::internal("a queued completion must not send an RPC").into_http())
    })));
    let session = session(client);
    let _capacity = session
        .completion_permits
        .acquire_many(MAX_IN_FLIGHT_COMPLETIONS as u32)
        .await
        .expect("fill delivery window");
    let cancellation = CancellationToken::new();
    let completion = session.complete_tool_call(
        "queued".to_string(),
        cancellation.clone(),
        Ok(serde_json::Value::Null),
    );
    tokio::pin!(completion);
    assert!(futures::poll!(completion.as_mut()).is_pending());

    cancellation.cancel();
    completion.await;
    assert_eq!(session.require_open(), Ok(()));
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
