use std::borrow::ToOwned;
use std::sync::Arc;

use codex_git_tooling::CreateGhostCommitOptions;
use codex_git_tooling::GhostCommit;
use codex_git_tooling::GitToolingError;
use codex_git_tooling::create_ghost_commit;
use codex_protocol::models::ResponseItem;
use codex_utils_readiness::Readiness;
use codex_utils_readiness::ReadinessFlag;
use codex_utils_readiness::Token;
use tokio::task;
use tokio::task::JoinError;
use tracing::info;
use tracing::warn;

use crate::codex::Session;
use crate::codex::TurnContext;

pub(crate) fn spawn_ghost_snapshot_task(
    session: Arc<Session>,
    turn_context: Arc<TurnContext>,
    readiness: Arc<ReadinessFlag>,
    token: Token,
) {
    task::spawn(async move {
        GhostSnapshotTask::new(session, turn_context, readiness, token)
            .run()
            .await;
    });
}

struct GhostSnapshotTask {
    session: Arc<Session>,
    turn_context: Arc<TurnContext>,
    readiness: Arc<ReadinessFlag>,
    token: Token,
}

impl GhostSnapshotTask {
    fn new(
        session: Arc<Session>,
        turn_context: Arc<TurnContext>,
        readiness: Arc<ReadinessFlag>,
        token: Token,
    ) -> Self {
        Self {
            session,
            turn_context,
            readiness,
            token,
        }
    }

    async fn run(self) {
        let repo_path = self.turn_context.cwd.clone();
        let snapshot = task::spawn_blocking(move || {
            let options = CreateGhostCommitOptions::new(&repo_path);
            create_ghost_commit(&options)
        })
        .await;

        match snapshot {
            Ok(Ok(commit)) => self.handle_success(commit).await,
            Ok(Err(err)) => self.handle_git_error(err).await,
            Err(err) => self.handle_task_failure(err).await,
        }

        if let Err(err) = self.readiness.mark_ready(self.token).await {
            warn!("failed to mark ghost snapshot ready: {err}");
        }
    }

    async fn handle_success(&self, commit: GhostCommit) {

        self.session.record_conversation_items(
            &[ResponseItem::GhostSnapshot {
                commit_id: commit.id().to_string(),
                parent: commit.parent().map(ToOwned::to_owned),
            }]
        ).await;
        info!(
            sub_id = self.turn_context.sub_id.as_str(),
            commit_id = commit.id(),
            "captured ghost snapshot"
        );
    }

    async fn handle_git_error(&self, err: GitToolingError) {
        warn!(
            sub_id = self.turn_context.sub_id.as_str(),
            "failed to capture ghost snapshot: {err}"
        );
        let message = match err {
            GitToolingError::NotAGitRepository { .. } => {
                "Snapshots disabled: current directory is not a Git repository.".to_string()
            }
            _ => format!(
                "Snapshots disabled after ghost snapshot error: {err}."
            ),
        };
        self.session
            .notify_background_event(self.turn_context.as_ref(), message)
            .await;
    }

    async fn handle_task_failure(&self, err: JoinError) {
        warn!(
            sub_id = self.turn_context.sub_id.as_str(),
            "ghost snapshot task failed: {err}"
        );
        self.session
            .notify_background_event(
                self.turn_context.as_ref(),
                "Failed to capture workspace snapshot due to an internal error.",
            )
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::ghost_snapshot_response_item;
    use codex_git_tooling::GhostCommit;
    use codex_protocol::models::ResponseItem;

    #[test]
    fn ghost_snapshot_response_item_includes_commit_ids() {
        let commit = GhostCommit::new("abc123".to_string(), Some("def456".to_string()));
        let item = ghost_snapshot_response_item(&commit);
        match item {
            ResponseItem::GhostSnapshot { commit_id, parent } => {
                assert_eq!(commit_id, "abc123");
                assert_eq!(parent, Some("def456".to_string()));
            }
            other => panic!("unexpected response item: {other:?}"),
        }
    }
}
