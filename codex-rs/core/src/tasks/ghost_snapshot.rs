use std::borrow::ToOwned;
use std::sync::Arc;
use async_trait::async_trait;
use crate::codex::TurnContext;
use crate::state::TaskKind;
use crate::tasks::SessionTask;
use crate::tasks::SessionTaskContext;
use codex_git_tooling::CreateGhostCommitOptions;
use codex_git_tooling::GitToolingError;
use codex_git_tooling::create_ghost_commit;
use codex_protocol::models::ResponseItem;
use codex_protocol::user_input::UserInput;
use codex_utils_readiness::Readiness;
use codex_utils_readiness::Token;
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
        cancellation_token: CancellationToken, // TODO handle cancellation token with a tokio select
    ) -> Option<String> {
        tokio::task::spawn(async move {
            let repo_path = ctx.cwd.clone();
            let options = CreateGhostCommitOptions::new(&repo_path);
            let ghost_commit = create_ghost_commit(&options);
            info!("ghost snapshot blocking task finished");
            match ghost_commit {
                Ok(ghost_commit) => {
                    session
                        .session
                        .record_conversation_items(&[ResponseItem::GhostSnapshot {
                            commit_id: ghost_commit.id().to_string(),
                            parent: ghost_commit.parent().map(ToOwned::to_owned),
                        }])
                        .await;
                    info!("ghost commit captured: {}", ghost_commit.id());
                }
                Err(err) => {
                    warn!(
                        sub_id = ctx.sub_id.as_str(),
                        "failed to capture ghost snapshot: {err}"
                    );
                    let message = match err {
                        GitToolingError::NotAGitRepository { .. } => {
                            "Snapshots disabled: current directory is not a Git repository."
                                .to_string()
                        }
                        _ => format!("Snapshots disabled after ghost snapshot error: {err}."),
                    };
                    session.session
                        .notify_background_event(&ctx, message)
                        .await;
                }
            }
            match ctx.tool_call_gate.mark_ready(self.token).await {
                Ok(true) => info!("ghost snapshot gate marked ready"),
                Ok(false) => warn!("ghost snapshot gate already ready"),
                Err(err) => warn!("failed to mark ghost snapshot ready: {err}"),
            }
        });
        None
    }
}

impl GhostSnapshotTask {
    pub(crate) fn new(
        token: Token,
    ) -> Self {
        Self {
            token,
        }
    }
}