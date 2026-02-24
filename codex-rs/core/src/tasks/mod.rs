mod compact;
mod ghost_snapshot;
mod regular;
mod review;
mod undo;
mod user_shell;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::select;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use tokio_util::task::AbortOnDropHandle;
use tracing::Instrument;
use tracing::Span;
use tracing::trace;
use tracing::warn;

use crate::AuthManager;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::models_manager::manager::ModelsManager;
use crate::protocol::EventMsg;
use crate::protocol::TurnAbortReason;
use crate::protocol::TurnAbortedEvent;
use crate::protocol::TurnCompleteEvent;
use crate::session_prefix::TURN_ABORTED_OPEN_TAG;
use crate::state::ActiveTurn;
use crate::state::RunningTask;
use crate::state::TaskKind;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::user_input::UserInput;

pub(crate) use compact::CompactTask;
pub(crate) use ghost_snapshot::GhostSnapshotTask;
pub(crate) use regular::RegularTask;
pub(crate) use review::ReviewTask;
pub(crate) use undo::UndoTask;
pub(crate) use user_shell::UserShellCommandMode;
pub(crate) use user_shell::UserShellCommandTask;
pub(crate) use user_shell::execute_user_shell_command;

const GRACEFULL_INTERRUPTION_TIMEOUT_MS: u64 = 100;
const TURN_ABORTED_INTERRUPTED_GUIDANCE: &str = "The user interrupted the previous turn on purpose. Any running unified exec processes were terminated. If any tools/commands were aborted, they may have partially executed; verify current state before retrying.";

/// Thin wrapper that exposes the parts of [`Session`] task runners need.
#[derive(Clone)]
pub(crate) struct SessionTaskContext {
    session: Arc<Session>,
}

impl SessionTaskContext {
    pub(crate) fn new(session: Arc<Session>) -> Self {
        Self { session }
    }

    pub(crate) fn clone_session(&self) -> Arc<Session> {
        Arc::clone(&self.session)
    }

    pub(crate) fn auth_manager(&self) -> Arc<AuthManager> {
        Arc::clone(&self.session.services.auth_manager)
    }

    pub(crate) fn models_manager(&self) -> Arc<ModelsManager> {
        Arc::clone(&self.session.services.models_manager)
    }
}

/// Async task that drives a [`Session`] turn.
///
/// Implementations encapsulate a specific Codex workflow (regular chat,
/// reviews, ghost snapshots, etc.). Each task instance is owned by a
/// [`Session`] and executed on a background Tokio task. The trait is
/// intentionally small: implementers identify themselves via
/// [`SessionTask::kind`], perform their work in [`SessionTask::run`], and may
/// release resources in [`SessionTask::abort`].
#[async_trait]
pub(crate) trait SessionTask: Send + Sync + 'static {
    /// Describes the type of work the task performs so the session can
    /// surface it in telemetry and UI.
    fn kind(&self) -> TaskKind;

    /// Executes the task until completion or cancellation.
    ///
    /// Implementations typically stream protocol events using `session` and
    /// `ctx`, returning an optional final agent message when finished. The
    /// provided `cancellation_token` is cancelled when the session requests an
    /// abort; implementers should watch for it and terminate quickly once it
    /// fires. Returning [`Some`] yields a final message that
    /// [`Session::on_task_finished`] will emit to the client.
    async fn run(
        self: Arc<Self>,
        session: Arc<SessionTaskContext>,
        ctx: Arc<TurnContext>,
        input: Vec<UserInput>,
        cancellation_token: CancellationToken,
    ) -> Option<String>;

    /// Gives the task a chance to perform cleanup after an abort.
    ///
    /// The default implementation is a no-op; override this if additional
    /// teardown or notifications are required once
    /// [`Session::abort_all_tasks`] cancels the task.
    async fn abort(&self, session: Arc<SessionTaskContext>, ctx: Arc<TurnContext>) {
        let _ = (session, ctx);
    }
}

impl Session {
    pub async fn spawn_task<T: SessionTask>(
        self: &Arc<Self>,
        turn_context: Arc<TurnContext>,
        input: Vec<UserInput>,
        task: T,
    ) {
        self.abort_all_tasks(TurnAbortReason::Replaced).await;
        self.clear_connector_selection().await;

        let task: Arc<dyn SessionTask> = Arc::new(task);
        let task_kind = task.kind();

        let cancellation_token = CancellationToken::new();
        let done = Arc::new(Notify::new());

        let done_clone = Arc::clone(&done);
        let handle = {
            let session_ctx = Arc::new(SessionTaskContext::new(Arc::clone(self)));
            let ctx = Arc::clone(&turn_context);
            let task_for_run = Arc::clone(&task);
            let task_cancellation_token = cancellation_token.child_token();
            let session_span = Span::current();
            tokio::spawn(
                async move {
                    let ctx_for_finish = Arc::clone(&ctx);
                    let last_agent_message = task_for_run
                        .run(
                            Arc::clone(&session_ctx),
                            ctx,
                            input,
                            task_cancellation_token.child_token(),
                        )
                        .await;
                    let sess = session_ctx.clone_session();
                    sess.flush_rollout().await;
                    if !task_cancellation_token.is_cancelled() {
                        // Emit completion uniformly from spawn site so all tasks share the same lifecycle.
                        sess.on_task_finished(Arc::clone(&ctx_for_finish), last_agent_message)
                            .await;
                    }
                    done_clone.notify_waiters();
                }
                .instrument(session_span),
            )
        };

        let timer = turn_context
            .otel_manager
            .start_timer("codex.turn.e2e_duration_ms", &[])
            .ok();

        let running_task = RunningTask {
            done,
            handle: Arc::new(AbortOnDropHandle::new(handle)),
            kind: task_kind,
            task,
            cancellation_token,
            turn_context: Arc::clone(&turn_context),
            _timer: timer,
        };
        self.register_new_active_task(running_task).await;
    }

    pub async fn abort_all_tasks(self: &Arc<Self>, reason: TurnAbortReason) {
        for task in self.take_all_running_tasks().await {
            self.handle_task_abort(task, reason.clone()).await;
        }
        if reason == TurnAbortReason::Interrupted {
            self.close_unified_exec_processes().await;
        }
    }

    pub async fn on_task_finished(
        self: &Arc<Self>,
        turn_context: Arc<TurnContext>,
        last_agent_message: Option<String>,
    ) {
        turn_context
            .turn_metadata_state
            .cancel_git_enrichment_task();

        let mut active = self.active_turn.lock().await;
        let mut pending_input = Vec::<ResponseInputItem>::new();
        let mut should_clear_active_turn = false;
        if let Some(at) = active.as_mut()
            && at.remove_task(&turn_context.sub_id)
        {
            let mut ts = at.turn_state.lock().await;
            pending_input = ts.take_pending_input();
            should_clear_active_turn = true;
        }
        if should_clear_active_turn {
            *active = None;
        }
        drop(active);
        if !pending_input.is_empty() {
            let pending_response_items = pending_input
                .into_iter()
                .map(ResponseItem::from)
                .collect::<Vec<_>>();
            self.record_conversation_items(turn_context.as_ref(), &pending_response_items)
                .await;
        }
        let event = EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: turn_context.sub_id.clone(),
            last_agent_message,
        });
        self.send_event(turn_context.as_ref(), event).await;
    }

    async fn register_new_active_task(&self, task: RunningTask) {
        let mut active = self.active_turn.lock().await;
        let mut turn = ActiveTurn::default();
        turn.add_task(task);
        *active = Some(turn);
    }

    async fn take_all_running_tasks(&self) -> Vec<RunningTask> {
        let mut active = self.active_turn.lock().await;
        match active.take() {
            Some(mut at) => {
                at.clear_pending().await;

                at.drain_tasks()
            }
            None => Vec::new(),
        }
    }

    pub(crate) async fn close_unified_exec_processes(&self) {
        self.services
            .unified_exec_manager
            .terminate_all_processes()
            .await;
    }

    async fn handle_task_abort(self: &Arc<Self>, task: RunningTask, reason: TurnAbortReason) {
        let sub_id = task.turn_context.sub_id.clone();
        if task.cancellation_token.is_cancelled() {
            return;
        }

        trace!(task_kind = ?task.kind, sub_id, "aborting running task");
        task.cancellation_token.cancel();
        task.turn_context
            .turn_metadata_state
            .cancel_git_enrichment_task();
        let session_task = task.task;

        select! {
            _ = task.done.notified() => {
            },
            _ = tokio::time::sleep(Duration::from_millis(GRACEFULL_INTERRUPTION_TIMEOUT_MS)) => {
                warn!("task {sub_id} didn't complete gracefully after {}ms", GRACEFULL_INTERRUPTION_TIMEOUT_MS);
            }
        }

        task.handle.abort();

        let session_ctx = Arc::new(SessionTaskContext::new(Arc::clone(self)));
        session_task
            .abort(session_ctx, Arc::clone(&task.turn_context))
            .await;

        if reason == TurnAbortReason::Interrupted {
            let marker = ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: format!(
                        "{TURN_ABORTED_OPEN_TAG}\n{TURN_ABORTED_INTERRUPTED_GUIDANCE}\n</turn_aborted>"
                    ),
                }],
                end_turn: None,
                phase: None,
            };
            self.record_into_history(std::slice::from_ref(&marker), task.turn_context.as_ref())
                .await;
            self.persist_rollout_items(&[RolloutItem::ResponseItem(marker)])
                .await;
            // Ensure the marker is durably visible before emitting TurnAborted: some clients
            // synchronously re-read the rollout on receipt of the abort event.
            self.flush_rollout().await;
        }

        let event = EventMsg::TurnAborted(TurnAbortedEvent {
            turn_id: Some(task.turn_context.sub_id.clone()),
            reason,
        });
        self.send_event(task.turn_context.as_ref(), event).await;
    }
}

#[cfg(test)]
mod tests {}
