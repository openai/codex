//! Checks the save boundary between a private Guardian decision and the reviewed action.

use codex_core::TurnInputRequest;
use codex_core::config::Constrained;
use codex_history::RolloutItem;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::ThreadSource;
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
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::any::Any;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::timeout;

struct PendingSave {
    history: StoredThreadHistory,
    complete: oneshot::Sender<()>,
}

#[derive(Default)]
struct ReviewerSaves {
    thread_id: Option<ThreadId>,
    reviews: usize,
}

struct GatedReviewerStore {
    inner: InMemoryThreadStore,
    reviewer: Mutex<ReviewerSaves>,
    saves: mpsc::UnboundedSender<PendingSave>,
}

macro_rules! delegate_store_methods {
    ($(fn $name:ident($param:ident: $params:ty) -> $result:ty;)*) => {
        $(fn $name(&self, $param: $params) -> ThreadStoreFuture<'_, $result> {
            ThreadStore::$name(&self.inner, $param)
        })*
    };
}

impl ThreadStore for GatedReviewerStore {
    fn as_any(&self) -> &dyn Any {
        self
    }

    delegate_store_methods! {
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
        fn shutdown_thread(thread_id: ThreadId) -> ();
    }

    fn create_thread(&self, params: CreateThreadParams) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move {
            if params.thread_source == Some(ThreadSource::GuardianReview) {
                self.reviewer.lock().await.thread_id = Some(params.thread_id);
            }
            ThreadStore::create_thread(&self.inner, params).await
        })
    }

    fn persist_thread(
        &self,
        thread_id: ThreadId,
        context: PersistContext,
    ) -> ThreadStoreFuture<'_, ()> {
        self.inner.persist_thread(thread_id, context)
    }

    fn flush_thread(&self, thread_id: ThreadId) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move {
            let is_reviewer = self.reviewer.lock().await.thread_id == Some(thread_id);
            let pending = if is_reviewer {
                let history = ThreadStore::load_history(
                    &self.inner,
                    LoadThreadHistoryParams {
                        thread_id,
                        include_archived: true,
                    },
                )
                .await?;
                let reviews = history.items.iter().filter(|item| matches!(
                    item, RolloutItem::ResponseItem(item)
                        if matches!(&item.item, ResponseItem::Message { role, .. } if role == "assistant")
                )).count();
                let mut reviewer = self.reviewer.lock().await;
                if reviews > reviewer.reviews {
                    reviewer.reviews = reviews;
                    Some(history)
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(history) = pending {
                let (complete, completed) = oneshot::channel();
                self.saves
                    .send(PendingSave { history, complete })
                    .expect("save receiver");
                completed.await.expect("release the reviewer save");
            }
            self.inner.flush_thread(thread_id).await
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_saves_each_completed_review_before_releasing_its_action() -> anyhow::Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    core_test_support::skip_if_wine_exec!(Ok(()), "uses the Unix command approval harness");
    let server = responses::start_mock_server().await;
    let (saves, mut pending_saves) = mpsc::unbounded_channel();
    let store = Arc::new(GatedReviewerStore {
        inner: InMemoryThreadStore::default(),
        reviewer: Mutex::new(ReviewerSaves::default()),
        saves,
    });
    let test = test_codex()
        .with_thread_store(store)
        .with_history_mode(ThreadHistoryMode::Legacy)
        .with_config(|config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
        })
        .build_with_auto_env(&server)
        .await?;
    let command = json!({
        "cmd": "true", "sandbox_permissions": "require_escalated",
        "justification": "Run the authorized command.",
    })
    .to_string();
    let approval = r#"{"risk_level":"low","user_authorization":"high","outcome":"allow"}"#;
    let mut turns = Vec::new();
    for index in 0..2 {
        turns.push(responses::sse(vec![
            responses::ev_function_call(&format!("command-{index}"), "exec_command", &command),
            responses::ev_completed(&format!("parent-{index}")),
        ]));
        turns.push(responses::sse(vec![
            responses::ev_assistant_message(&format!("review-{index}"), approval),
            responses::ev_completed(&format!("guardian-{index}")),
        ]));
    }
    turns.push(responses::sse(vec![responses::ev_completed("parent-done")]));
    let mock = responses::mount_sse_sequence(&server, turns).await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Run both commands.".to_owned(),
            text_elements: Vec::new(),
        }]))
        .await?;

    let mut reviewer_id = None;
    for review in 1..=2 {
        let pending = timeout(Duration::from_secs(10), pending_saves.recv())
            .await?
            .expect("review save");
        assert_eq!(
            *reviewer_id.get_or_insert(pending.history.thread_id),
            pending.history.thread_id
        );
        assert_eq!(
            pending
                .history
                .items
                .iter()
                .filter(|item| matches!(item, RolloutItem::EventMsg(EventMsg::TurnComplete(_))))
                .count(),
            review,
            "the first save after inference must include turn completion"
        );
        // The parent cannot consume the approval while its review is still being saved.
        assert!(
            timeout(
                Duration::from_millis(50),
                wait_for_event(&test.codex, |event| {
                    matches!(event, EventMsg::ExecCommandBegin(_))
                })
            )
            .await
            .is_err()
        );
        assert_eq!(mock.requests().len(), review * 2);
        pending.complete.send(()).expect("finish save");
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::ExecCommandEnd(_))
        })
        .await;
    }
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(mock.requests().len(), 5);
    test.codex.shutdown_and_wait().await?;
    Ok(())
}
