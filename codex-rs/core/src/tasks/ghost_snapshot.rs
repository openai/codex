use crate::codex::TurnContext;
use crate::state::TaskKind;
use crate::tasks::SessionTask;
use crate::tasks::SessionTaskContext;
use async_trait::async_trait;
use codex_git_tooling::CreateGhostCommitOptions;
use codex_git_tooling::GitToolingError;
use codex_git_tooling::create_ghost_commit;
use codex_protocol::models::ResponseItem;
use codex_protocol::user_input::UserInput;
use codex_utils_readiness::Readiness;
use codex_utils_readiness::Token;
use std::borrow::ToOwned;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing::warn;

pub(crate) struct GhostSnapshotTask {
    token: Token,
}

#[async_trait]
impl SessionTask for GhostSnapshotTask {
    fn kind(&self) -> TaskKind {
        TaskKind::Regular
    }

    async fn run(
        self: Arc<Self>,
        session: Arc<SessionTaskContext>,
        ctx: Arc<TurnContext>,
        _input: Vec<UserInput>,
        cancellation_token: CancellationToken,
    ) -> Option<String> {
        tokio::task::spawn(async move {
            let token = self.token;
            let ctx_for_task = Arc::clone(&ctx);
            let cancelled = tokio::select! {
                _ = cancellation_token.cancelled() => true,
                _ = async {
                    let repo_path = ctx_for_task.cwd.clone();
                    // Required to run in a dedicated blocking pool.
                    match tokio::task::spawn_blocking(move || {
                        let options = CreateGhostCommitOptions::new(&repo_path);
                        create_ghost_commit(&options)
                    })
                    .await
                    {
                        Ok(Ok(ghost_commit)) => {
                            info!("ghost snapshot blocking task finished");
                            session
                                .session
                                .record_conversation_items(&ctx, &[ResponseItem::GhostSnapshot {
                                    commit_id: ghost_commit.id().to_string(),
                                    parent: ghost_commit.parent().map(ToOwned::to_owned),
                                }])
                                .await;
                            info!("ghost commit captured: {}", ghost_commit.id());
                        }
                        Ok(Err(err)) => {
                            warn!(
                                sub_id = ctx_for_task.sub_id.as_str(),
                                "failed to capture ghost snapshot: {err}"
                            );
                            let message = match err {
                                GitToolingError::NotAGitRepository { .. } => {
                                    "Snapshots disabled: current directory is not a Git repository."
                                        .to_string()
                                }
                                _ => format!("Snapshots disabled after ghost snapshot error: {err}."),
                            };
                            session
                                .session
                                .notify_background_event(&ctx_for_task, message)
                                .await;
                        }
                        Err(err) => {
                            warn!(
                                sub_id = ctx_for_task.sub_id.as_str(),
                                "ghost snapshot task panicked: {err}"
                            );
                            let message =
                                format!("Snapshots disabled after ghost snapshot panic: {err}.");
                            session
                                .session
                                .notify_background_event(&ctx_for_task, message)
                                .await;
                        }
                    }
                } => false,
            };

            if cancelled {
                info!("ghost snapshot task cancelled");
            }

            match ctx.tool_call_gate.mark_ready(token).await {
                Ok(true) => info!("ghost snapshot gate marked ready"),
                Ok(false) => warn!("ghost snapshot gate already ready"),
                Err(err) => warn!("failed to mark ghost snapshot ready: {err}"),
            }
        });
        None
    }
}

impl GhostSnapshotTask {
    pub(crate) fn new(token: Token) -> Self {
        Self { token }
    }
}
