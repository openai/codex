//! Exercises the checkpoint between a steered input and its next model request.

use std::any::Any;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_core::TurnInput;
use codex_core::TurnInputRequest;
use codex_core::TurnInputSubmission;
use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::user_input::UserInput;
use codex_thread_store::AppendThreadItemsParams;
use codex_thread_store::ArchiveThreadParams;
use codex_thread_store::CreateThreadParams;
use codex_thread_store::DeleteThreadParams;
use codex_thread_store::InMemoryThreadStore;
use codex_thread_store::ListThreadsParams;
use codex_thread_store::LoadThreadHistoryParams;
use codex_thread_store::PersistContext;
use codex_thread_store::ReadThreadByRolloutPathParams;
use codex_thread_store::ReadThreadParams;
use codex_thread_store::ResumeThreadParams;
use codex_thread_store::StoredThread;
use codex_thread_store::StoredThreadHistory;
use codex_thread_store::ThreadPage;
use codex_thread_store::ThreadStore;
use codex_thread_store::ThreadStoreFuture;
use codex_thread_store::UpdateThreadMetadataParams;
use core_test_support::responses;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::timeout;

#[derive(Clone, Copy, PartialEq, Eq)]
enum CheckpointPolicy {
    Background,
    Synchronous,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InputKind {
    User,
    ToolOutput,
}

#[derive(Debug)]
struct PendingCheckpoint {
    context: PersistContext,
    complete: oneshot::Sender<()>,
}

/// Records one checkpoint and gates it only when its persistence must be synchronous.
struct GatedCheckpointStore {
    inner: InMemoryThreadStore,
    policy: CheckpointPolicy,
    armed: AtomicBool,
    checkpoints: mpsc::UnboundedSender<PendingCheckpoint>,
}

macro_rules! delegate_store_methods {
    ($(fn $name:ident($param:ident: $params:ty) -> $result:ty;)*) => {
        $(fn $name(&self, $param: $params) -> ThreadStoreFuture<'_, $result> {
            ThreadStore::$name(&self.inner, $param)
        })*
    };
}

impl ThreadStore for GatedCheckpointStore {
    fn as_any(&self) -> &dyn Any {
        self
    }

    delegate_store_methods! {
        fn create_thread(params: CreateThreadParams) -> ();
        fn resume_thread(params: ResumeThreadParams) -> ();
        fn append_items(params: AppendThreadItemsParams) -> ();
        fn discard_thread(thread_id: ThreadId) -> ();
        fn load_history(params: LoadThreadHistoryParams) -> StoredThreadHistory;
        fn read_thread(params: ReadThreadParams) -> StoredThread;
        fn read_thread_by_rollout_path(params: ReadThreadByRolloutPathParams) -> StoredThread;
        fn list_threads(params: ListThreadsParams) -> ThreadPage;
        fn update_thread_metadata(params: UpdateThreadMetadataParams) -> Option<StoredThread>;
        fn archive_thread(params: ArchiveThreadParams) -> ();
        fn unarchive_thread(params: ArchiveThreadParams) -> StoredThread;
        fn delete_thread(params: DeleteThreadParams) -> ();
        fn flush_thread(thread_id: ThreadId) -> ();
        fn shutdown_thread(thread_id: ThreadId) -> ();
    }

    fn persist_thread(
        &self,
        thread_id: ThreadId,
        context: PersistContext,
    ) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move {
            if self.armed.swap(false, Ordering::SeqCst) {
                let (complete, completed) = oneshot::channel();
                self.checkpoints
                    .send(PendingCheckpoint { context, complete })
                    .expect("checkpoint receiver should stay alive");
                if self.policy == CheckpointPolicy::Synchronous
                    || !context.allows_background_persistence()
                {
                    completed.await.expect("test should complete checkpoint");
                }
            }
            self.inner.persist_thread(thread_id, context).await
        })
    }
}

#[test_case(CheckpointPolicy::Background, InputKind::User; "background_user_input")]
#[test_case(CheckpointPolicy::Synchronous, InputKind::User; "synchronous_store")]
#[test_case(CheckpointPolicy::Background, InputKind::ToolOutput; "tool_output_stays_synchronous")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn steered_input_checkpoint_controls_next_request(
    policy: CheckpointPolicy,
    input_kind: InputKind,
) -> anyhow::Result<()> {
    let (first_completed, first_completion) = oneshot::channel();
    let (second_completed, second_completion) = oneshot::channel();
    let (server, _completions) = start_streaming_sse_server(vec![
        vec![
            StreamingSseChunk {
                gate: None,
                body: responses::sse(vec![
                    responses::ev_response_created("first"),
                    responses::ev_message_item_added("first-message", ""),
                    responses::ev_output_text_delta("original answer"),
                ]),
            },
            StreamingSseChunk {
                gate: Some(first_completion),
                body: responses::sse(vec![
                    responses::ev_assistant_message("first-message", "original answer"),
                    responses::ev_completed("first"),
                ]),
            },
        ],
        vec![StreamingSseChunk {
            gate: Some(second_completion),
            body: responses::sse(vec![
                responses::ev_response_created("second"),
                responses::ev_completed("second"),
            ]),
        }],
    ])
    .await;
    let (checkpoints, mut checkpoint_requests) = mpsc::unbounded_channel();
    let store = Arc::new(GatedCheckpointStore {
        inner: InMemoryThreadStore::default(),
        policy,
        armed: AtomicBool::new(false),
        checkpoints,
    });
    let base_url = format!("{}/v1", server.uri());
    let config_server = responses::start_mock_server().await;
    let test = test_codex()
        .with_thread_store(store.clone())
        .with_history_mode(ThreadHistoryMode::Legacy)
        .with_config(move |config| config.model_provider.base_url = Some(base_url))
        .build_with_auto_env(&config_server)
        .await?;
    let first = test
        .codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "first prompt".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let TurnInputSubmission::Started { turn_id } = first else {
        panic!("first input should start a turn");
    };
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::AgentMessageContentDelta(_))
    })
    .await;
    store.armed.store(true, Ordering::SeqCst);
    let input = match input_kind {
        InputKind::User => TurnInputRequest::user_input(vec![UserInput::Text {
            text: "steered input".to_string(),
            text_elements: Vec::new(),
        }]),
        InputKind::ToolOutput => {
            TurnInputRequest::new(TurnInput::ResponseItem(serde_json::from_value(json!({
                "type": "function_call_output",
                "name": "send_message_to_thread",
                "namespace": "codex_app",
                "output": "steered input",
            }))?))
        }
    };
    assert_eq!(
        test.codex.start_or_steer_turn(input).await?,
        TurnInputSubmission::Steered { turn_id }
    );
    // Steering still waits for the existing inference stream to finish.
    assert!(checkpoint_requests.try_recv().is_err());
    first_completed.send(()).expect("finish original inference");
    let checkpoint = timeout(Duration::from_secs(10), checkpoint_requests.recv())
        .await?
        .expect("Core should checkpoint the accepted input");
    assert_eq!(
        checkpoint.context,
        match input_kind {
            InputKind::User => PersistContext::SteeredUserInput,
            InputKind::ToolOutput => PersistContext::Standard,
        }
    );
    let should_overlap = policy == CheckpointPolicy::Background && input_kind == InputKind::User;
    if !should_overlap {
        assert!(
            timeout(
                Duration::from_millis(50),
                server.wait_for_request_count(/*count*/ 2)
            )
            .await
            .is_err()
        );
        checkpoint.complete.send(()).expect("complete checkpoint");
    }
    timeout(
        Duration::from_secs(10),
        server.wait_for_request_count(/*count*/ 2),
    )
    .await?;
    let requests = server.requests().await;
    assert_eq!(requests.len(), 2);
    assert!(!String::from_utf8_lossy(&requests[0]).contains("steered input"));
    assert!(String::from_utf8_lossy(&requests[1]).contains("steered input"));
    second_completed
        .send(())
        .expect("finish follow-up inference");
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.codex.shutdown_and_wait().await?;
    server.shutdown().await;
    Ok(())
}
