//! Keeps late tool-completion errors scoped to the canceled invocation.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use pretty_assertions::assert_eq;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tower::service_fn;
use tower::util::BoxCloneSyncService;

use super::super::GrpcClient;
use super::super::state::SessionState;
use super::super::transport::SharedTransport;
use super::SessionInner;

#[tokio::test]
async fn missing_invocation_completion_racing_cancellation_preserves_session() {
    let cancellation = CancellationToken::new();
    let completion_cancellation = cancellation.clone();
    let client = GrpcClient::new(BoxCloneSyncService::new(service_fn(move |_request| {
        // Cancel after the biased select has polled cancellation, before the RPC fails.
        completion_cancellation.cancel();
        async { Ok(tonic::Status::not_found("unknown code-mode tool invocation").into_http()) }
    })));
    let session = SessionInner {
        id: "session".to_string(),
        client: client.clone(),
        runtime: tokio::runtime::Handle::current(),
        state: Mutex::new(SessionState::default()),
        wait_slots: Mutex::new(HashMap::new()),
        shutdown_requested: AtomicBool::new(/*v*/ false),
        shutdown_result: Mutex::new(/*t*/ None),
        stopped: CancellationToken::new(),
        stream_tasks: TaskTracker::new(),
        _transport: Arc::new(SharedTransport::Connected(client)),
    };

    session
        .complete_tool_call(
            "invocation".to_string(),
            cancellation,
            Ok(serde_json::Value::Null),
        )
        .await;

    assert_eq!(session.require_open(), Ok(()));
}
