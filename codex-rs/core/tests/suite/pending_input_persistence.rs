//! Exercises preparation and execution checkpoints before model requests.

use std::any::Any;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_core::TurnInput;
use codex_core::TurnInputRequest;
use codex_core::TurnInputSubmission;
use codex_history::InitialHistory;
use codex_history::ResumedHistory;
use codex_protocol::ThreadId;
use codex_protocol::mcp::ClientMcpExtensions;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::user_input::UserInput;
use codex_rollout::RolloutItem;
use codex_rollout::RolloutRecorder;
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
    Preparation,
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
        fn archive_thread(params: ArchiveThreadParams) -> ();
        fn unarchive_thread(params: ArchiveThreadParams) -> StoredThread;
        fn delete_thread(params: DeleteThreadParams) -> ();
        fn flush_thread(thread_id: ThreadId) -> ();
        fn shutdown_thread(thread_id: ThreadId) -> ();
    }

    fn update_thread_metadata(
        &self,
        params: UpdateThreadMetadataParams,
    ) -> ThreadStoreFuture<'_, Option<StoredThread>> {
        Box::pin(async move {
            if self.policy == CheckpointPolicy::Preparation {
                std::future::pending::<()>().await;
            }
            self.inner.update_thread_metadata(params).await
        })
    }

    fn record_thread_metadata(
        &self,
        params: UpdateThreadMetadataParams,
    ) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move { self.inner.update_thread_metadata(params).await.map(|_| ()) })
    }

    fn persist_thread(
        &self,
        thread_id: ThreadId,
        context: PersistContext,
    ) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move {
            if self.policy == CheckpointPolicy::Preparation
                && context == PersistContext::ThreadPreparation
            {
                return Ok(());
            }
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn preparation_and_first_sampling_do_not_wait_for_durable_metadata() -> anyhow::Result<()> {
    let (complete, completion) = oneshot::channel();
    let (server, _completions) = start_streaming_sse_server(vec![vec![StreamingSseChunk {
        gate: Some(completion),
        body: responses::sse(vec![
            responses::ev_response_created("first"),
            responses::ev_completed("first"),
        ]),
    }]])
    .await;
    let (checkpoints, mut checkpoint_requests) = mpsc::unbounded_channel();
    let store = Arc::new(GatedCheckpointStore {
        inner: InMemoryThreadStore::default(),
        policy: CheckpointPolicy::Preparation,
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
    store.armed.store(true, Ordering::SeqCst);
    timeout(Duration::from_secs(10), test.codex.inject_response_items(vec![serde_json::from_value(json!({
        "type": "message", "role": "developer", "content": [{"type": "input_text", "text": "prepared context"}]
    }))?])).await??;
    assert!(checkpoint_requests.try_recv().is_err());
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "first prompt".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let checkpoint = timeout(Duration::from_secs(10), checkpoint_requests.recv())
        .await?
        .expect("first execution checkpoint");
    assert_eq!(checkpoint.context, PersistContext::TurnStart);
    timeout(
        Duration::from_secs(10),
        server.wait_for_request_count(/*count*/ 1),
    )
    .await?;
    let requests = server.requests().await;
    assert_eq!(requests.len(), 1);
    let request: serde_json::Value = serde_json::from_slice(&requests[0])?;
    let prepared_input = request["input"]
        .as_array()
        .expect("request input")
        .iter()
        .flat_map(|item| {
            item["content"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(move |content| {
                    let text = content["text"].as_str()?;
                    matches!(text, "prepared context" | "first prompt")
                        .then(|| (item["role"].as_str().expect("message role"), text))
                })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        prepared_input,
        vec![("developer", "prepared context"), ("user", "first prompt")]
    );
    drop(checkpoint.complete);
    complete.send(()).expect("complete inference");
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    Ok(())
}

#[test_case(ThreadHistoryMode::Legacy; "legacy")]
#[test_case(ThreadHistoryMode::Paginated; "paginated")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_preparation_is_durable_before_first_input_and_survives_restart(
    history_mode: ThreadHistoryMode,
) -> anyhow::Result<()> {
    const PREPARED_CONTEXT: &str = "prepared context persisted before first input";
    const FIRST_PROMPT: &str = "first prompt after restart";

    let server = responses::start_mock_server().await;
    let mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("first-response"),
            responses::ev_assistant_message("first-message", "prepared context restored"),
            responses::ev_completed("first-response"),
        ]),
    )
    .await;
    let mut builder = test_codex().with_history_mode(history_mode);
    let initial = builder.build_with_auto_env(&server).await?;
    let prepared: ResponseItem = serde_json::from_value(json!({
        "type": "message",
        "id": "msg_preparation_before_input",
        "role": "developer",
        "content": [{"type": "input_text", "text": PREPARED_CONTEXT}],
    }))?;
    initial
        .codex
        .inject_response_items(vec![prepared.clone()])
        .await?;

    // Read disk before shutdown or a first turn can add another durability barrier.
    let rollout_path = initial.codex.rollout_path().expect("local rollout path");
    let (items, _, parse_errors) = RolloutRecorder::load_rollout_items(&rollout_path).await?;
    let persisted_preparation = items
        .into_iter()
        .filter_map(|item| match item {
            RolloutItem::ResponseItem(envelope) if envelope.item.id() == prepared.id() => {
                // Core preserves this ID and adds runtime turn/creation-time metadata.
                Some(responses::strip_metadata(envelope.item))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!((parse_errors, persisted_preparation), (0, vec![prepared]));
    assert!(mock.requests().is_empty());

    initial.codex.shutdown_and_wait().await?;
    let thread_id = initial.session_configured.thread_id;
    initial.thread_manager.remove_thread(&thread_id).await;
    // Reopen persisted context by ID, which supports both local history modes.
    let store = codex_core::thread_store_from_config(
        &initial.config,
        codex_core::init_state_db(&initial.config).await,
    );
    let context = store
        .load_latest_model_context(LoadThreadHistoryParams {
            thread_id,
            include_archived: false,
        })
        .await?;
    let resumed = initial
        .thread_manager
        .resume_thread_with_history(
            initial.config.clone(),
            InitialHistory::Resumed(ResumedHistory {
                conversation_id: context.thread_id,
                history: Arc::new(context.items),
                rollout_path: Some(rollout_path),
            }),
            initial.thread_manager.auth_manager(),
            /*parent_trace*/ None,
            ClientMcpExtensions::default(),
        )
        .await?;
    assert!(!Arc::ptr_eq(&initial.codex, &resumed.thread));
    resumed
        .thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: FIRST_PROMPT.to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&resumed.thread, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let input = mock.single_request().input();
    let prepared_input = input
        .iter()
        .flat_map(|item| {
            item["content"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(move |content| {
                    let text = content["text"].as_str()?;
                    matches!(text, PREPARED_CONTEXT | FIRST_PROMPT)
                        .then(|| (item["role"].as_str().expect("message role"), text))
                })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        prepared_input,
        vec![("developer", PREPARED_CONTEXT), ("user", FIRST_PROMPT)]
    );
    resumed.thread.shutdown_and_wait().await?;
    Ok(())
}
