mod compact;
mod ghost_snapshot;
mod regular;
mod review;
mod undo;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::select;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use tokio_util::task::AbortOnDropHandle;
use tracing::trace;
use tracing::warn;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::protocol::EventMsg;
use crate::protocol::TaskCompleteEvent;
use crate::protocol::TurnAbortReason;
use crate::protocol::TurnAbortedEvent;
use crate::state::ActiveTurn;
use crate::state::RunningTask;
use crate::state::TaskKind;
use codex_protocol::user_input::UserInput;

pub(crate) use compact::CompactTask;
pub(crate) use ghost_snapshot::GhostSnapshotTask;
pub(crate) use regular::RegularTask;
pub(crate) use review::ReviewTask;
pub(crate) use undo::UndoTask;

const GRACEFULL_INTERRUPTION_TIMEOUT_MS: u64 = 100;

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
}

#[async_trait]
pub(crate) trait SessionTask: Send + Sync + 'static {
    fn kind(&self) -> TaskKind;

    async fn run(
        self: Arc<Self>,
        session: Arc<SessionTaskContext>,
        ctx: Arc<TurnContext>,
        input: Vec<UserInput>,
        cancellation_token: CancellationToken,
    ) -> Option<String>;

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
            tokio::spawn(async move {
                let ctx_for_finish = Arc::clone(&ctx);
                let last_agent_message = task_for_run
                    .run(
                        Arc::clone(&session_ctx),
                        ctx,
                        input,
                        task_cancellation_token.child_token(),
                    )
                    .await;

                if !task_cancellation_token.is_cancelled() {
                    // Emit completion uniformly from spawn site so all tasks share the same lifecycle.
                    let sess = session_ctx.clone_session();
                    sess.on_task_finished(ctx_for_finish, last_agent_message)
                        .await;
                }
                done_clone.notify_waiters();
            })
        };

        let running_task = RunningTask {
            done,
            handle: Arc::new(AbortOnDropHandle::new(handle)),
            kind: task_kind,
            task,
            cancellation_token,
            turn_context: Arc::clone(&turn_context),
        };
        self.register_new_active_task(running_task).await;
    }

    pub async fn abort_all_tasks(self: &Arc<Self>, reason: TurnAbortReason) {
        for task in self.take_all_running_tasks().await {
            self.handle_task_abort(task, reason.clone()).await;
        }
    }

    pub async fn on_task_finished(
        self: &Arc<Self>,
        turn_context: Arc<TurnContext>,
        last_agent_message: Option<String>,
    ) {
        let mut active = self.active_turn.lock().await;
        if let Some(at) = active.as_mut()
            && at.remove_task(&turn_context.sub_id)
        {
            *active = None;
        }
        drop(active);
        let event = EventMsg::TaskComplete(TaskCompleteEvent { last_agent_message });
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

    async fn handle_task_abort(self: &Arc<Self>, task: RunningTask, reason: TurnAbortReason) {
        let sub_id = task.turn_context.sub_id.clone();
        if task.cancellation_token.is_cancelled() {
            return;
        }

        trace!(task_kind = ?task.kind, sub_id, "aborting running task");
        task.cancellation_token.cancel();
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

        let event = EventMsg::TurnAborted(TurnAbortedEvent { reason });
        self.send_event(task.turn_context.as_ref(), event).await;
    }
}

#[cfg(test)]
mod tests {}
