use std::sync::Arc;

use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use super::CellHost;
use super::CellToolCall;
use crate::runtime::RuntimeCommand;

#[derive(Clone, Copy)]
pub(super) enum CallbackCompletion {
    DrainNotifications,
    Cancel,
}

pub(super) fn spawn_notification<H: CellHost>(
    tasks: &mut JoinSet<()>,
    host: Arc<H>,
    call_id: String,
    text: String,
    cancellation_token: CancellationToken,
) {
    tasks.spawn(async move {
        if let Err(err) = host.notify(call_id, text, cancellation_token).await {
            warn!("failed to deliver code mode notification: {err}");
        }
    });
}

pub(super) fn spawn_tool<H: CellHost>(
    tasks: &mut JoinSet<()>,
    host: Arc<H>,
    invocation: CellToolCall,
    runtime_tx: std::sync::mpsc::Sender<RuntimeCommand>,
    cancellation_token: CancellationToken,
) {
    tasks.spawn(async move {
        let id = invocation.id.clone();
        let command = match host.invoke_tool(invocation, cancellation_token).await {
            Ok(result) => RuntimeCommand::ToolResponse { id, result },
            Err(error_text) => RuntimeCommand::ToolError { id, error_text },
        };
        let _ = runtime_tx.send(command);
    });
}

pub(super) async fn finish_callbacks(
    cancellation_token: &CancellationToken,
    notification_tasks: &mut JoinSet<()>,
    tool_tasks: &mut JoinSet<()>,
    completion: CallbackCompletion,
) {
    if matches!(completion, CallbackCompletion::Cancel) {
        cancellation_token.cancel();
    }
    drain_tasks(notification_tasks, "notification").await;
    cancellation_token.cancel();
    drain_tasks(tool_tasks, "tool").await;
}

pub(super) fn log_task_result(
    task_result: Option<Result<(), tokio::task::JoinError>>,
    description: &str,
) {
    if let Some(Err(err)) = task_result
        && !err.is_cancelled()
    {
        warn!("code mode {description} task failed: {err}");
    }
}

async fn drain_tasks(tasks: &mut JoinSet<()>, description: &str) {
    while let Some(result) = tasks.join_next().await {
        log_task_result(Some(result), description);
    }
}
