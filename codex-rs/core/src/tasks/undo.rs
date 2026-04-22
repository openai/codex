use std::sync::Arc;

use crate::session::turn_context::TurnContext;
use crate::state::TaskKind;
use crate::tasks::SessionTask;
use crate::tasks::SessionTaskContext;
use codex_git_utils::RestoreGhostCommitOptions;
use codex_git_utils::restore_ghost_commit_with_options;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::UndoCompletedEvent;
use codex_protocol::protocol::UndoStartedEvent;
use codex_protocol::user_input::UserInput;
use tokio_util::sync::CancellationToken;
use tracing::error;
use tracing::info;
use tracing::warn;

pub(crate) struct UndoTask;

impl UndoTask {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl SessionTask for UndoTask {
    fn kind(&self) -> TaskKind {
        TaskKind::Regular
    }

    fn span_name(&self) -> &'static str {
        "session_task.undo"
    }

    async fn run(
        self: Arc<Self>,
        session: Arc<SessionTaskContext>,
        ctx: Arc<TurnContext>,
        _input: Vec<UserInput>,
        cancellation_token: CancellationToken,
    ) -> Option<String> {
        session
            .session
            .services
            .session_telemetry
            .counter("codex.task.undo", /*inc*/ 1, &[]);
        let sess = session.clone_session();
        sess.send_event(
            ctx.as_ref(),
            EventMsg::UndoStarted(UndoStartedEvent {
                message: Some("Undo in progress...".to_string()),
            }),
        )
        .await;

        if cancellation_token.is_cancelled() {
            sess.send_event(
                ctx.as_ref(),
                EventMsg::UndoCompleted(UndoCompletedEvent {
                    success: false,
                    message: Some("Undo cancelled.".to_string()),
                }),
            )
            .await;
            return None;
        }

        let history = sess.clone_history().await;
        let mut items = history.raw_items().to_vec();
        let mut completed = UndoCompletedEvent {
            success: false,
            message: None,
        };

        let Some((idx, ghost_commit)) =
            items
                .iter()
                .enumerate()
                .rev()
                .find_map(|(idx, item)| match item {
                    ResponseItem::GhostSnapshot { ghost_commit } => {
                        Some((idx, ghost_commit.clone()))
                    }
                    _ => None,
                })
        else {
            completed.message = Some("No ghost snapshot available to undo.".to_string());
            sess.send_event(ctx.as_ref(), EventMsg::UndoCompleted(completed))
                .await;
            return None;
        };

        let commit_id = ghost_commit.id().to_string();
        let repo_path = ctx.cwd().clone();
        let ghost_snapshot = ctx.ghost_snapshot.clone();
        let restore_result = tokio::task::spawn_blocking(move || {
            let options = RestoreGhostCommitOptions::new(&repo_path).ghost_snapshot(ghost_snapshot);
            restore_ghost_commit_with_options(&options, &ghost_commit)
        })
        .await;

        match restore_result {
            Ok(Ok(())) => {
                items.remove(idx);
                let reference_context_item = sess.reference_context_item().await;
                sess.replace_history(items, reference_context_item).await;
                let short_id: String = commit_id.chars().take(7).collect();
                info!(commit_id = commit_id, "Undo restored ghost snapshot");
                completed.success = true;
                completed.message = Some(format!("Undo restored snapshot {short_id}."));
            }
            Ok(Err(err)) => {
                let message = format!("Failed to restore snapshot {commit_id}: {err}");
                warn!("{message}");
                completed.message = Some(message);
            }
            Err(err) => {
                let message = format!("Failed to restore snapshot {commit_id}: {err}");
                error!("{message}");
                completed.message = Some(message);
            }
        }

        sess.send_event(ctx.as_ref(), EventMsg::UndoCompleted(completed))
            .await;
        None
    }
}
