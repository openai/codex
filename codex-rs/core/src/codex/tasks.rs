use std::future::Future;
use std::sync::Arc;

use super::Session;
use super::TurnContext;
use super::compact;
use super::exit_review_mode;
use super::run_task;
use crate::protocol::Event;
use crate::protocol::EventMsg;
use crate::protocol::InputItem;
use crate::protocol::TaskCompleteEvent;
use crate::protocol::TurnAbortReason;
use crate::protocol::TurnAbortedEvent;
use crate::state::ActiveTurn;
use crate::state::RunningTask;
use crate::state::TaskKind;

impl Session {
    pub async fn spawn_task_regular(
        self: &Arc<Self>,
        turn_context: Arc<TurnContext>,
        sub_id: String,
        input: Vec<InputItem>,
    ) {
        self.spawn_task_with(turn_context, sub_id, input, TaskKind::Regular, run_task)
            .await;
    }

    pub async fn spawn_task_review(
        self: &Arc<Self>,
        turn_context: Arc<TurnContext>,
        sub_id: String,
        input: Vec<InputItem>,
    ) {
        self.spawn_task_with(turn_context, sub_id, input, TaskKind::Review, run_task)
            .await;
    }

    pub async fn spawn_task_compact(
        self: &Arc<Self>,
        turn_context: Arc<TurnContext>,
        sub_id: String,
        input: Vec<InputItem>,
    ) {
        self.spawn_task_with(
            turn_context,
            sub_id,
            input,
            TaskKind::Compact,
            compact::run_compact_task,
        )
        .await;
    }

    pub async fn abort_all_tasks(self: &Arc<Self>, reason: TurnAbortReason) {
        for (sub_id, task) in self.take_all_running_tasks().await {
            self.handle_task_abort(sub_id, task, reason.clone()).await;
        }
    }

    pub async fn on_task_finished(
        self: &Arc<Self>,
        sub_id: String,
        last_agent_message: Option<String>,
    ) {
        let mut active = self.active_turn.lock().await;
        if let Some(at) = active.as_mut() {
            at.remove_task(&sub_id);
            if at.is_empty() {
                *active = None;
            }
        }
        drop(active);
        let event = Event {
            id: sub_id,
            msg: EventMsg::TaskComplete(TaskCompleteEvent { last_agent_message }),
        };
        self.send_event(event).await;
    }

    async fn spawn_task_with<R, Fut>(
        self: &Arc<Self>,
        turn_context: Arc<TurnContext>,
        sub_id: String,
        input: Vec<InputItem>,
        kind: TaskKind,
        runner: R,
    ) where
        R: FnOnce(Arc<Session>, Arc<TurnContext>, String, Vec<InputItem>) -> Fut,
        R: Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.abort_all_tasks(TurnAbortReason::Replaced).await;
        let handle = {
            let sess = Arc::clone(self);
            let sub_clone = sub_id.clone();
            let ctx = Arc::clone(&turn_context);
            tokio::spawn(async move {
                runner(sess, ctx, sub_clone, input).await;
            })
            .abort_handle()
        };
        let running_task = RunningTask { handle, kind };
        self.register_new_active_task(sub_id, running_task).await;
    }

    async fn register_new_active_task(&self, sub_id: String, task: RunningTask) {
        let mut active = self.active_turn.lock().await;
        let mut turn = ActiveTurn::default();
        turn.add_task(sub_id, task);
        *active = Some(turn);
    }

    async fn take_all_running_tasks(&self) -> Vec<(String, RunningTask)> {
        let mut active = self.active_turn.lock().await;
        match active.as_mut() {
            Some(at) => {
                {
                    let mut ts = at.turn_state.lock().await;
                    ts.clear_pending();
                }
                let tasks = at.drain_tasks();
                *active = None;
                tasks.into_iter().collect()
            }
            None => Vec::new(),
        }
    }

    async fn handle_task_abort(
        self: &Arc<Self>,
        sub_id: String,
        task: RunningTask,
        reason: TurnAbortReason,
    ) {
        if task.handle.is_finished() {
            return;
        }
        task.handle.abort();
        if task.kind == TaskKind::Review {
            exit_review_mode(Arc::clone(self), sub_id.clone(), None).await;
        }
        let event = Event {
            id: sub_id.clone(),
            msg: EventMsg::TurnAborted(TurnAbortedEvent { reason }),
        };
        self.send_event(event).await;
    }
}
