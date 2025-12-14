use crate::codex::TurnContext;
use crate::protocol::EventMsg;
use crate::protocol::WarningEvent;
use crate::state::TaskKind;
use crate::tasks::SessionTask;
use crate::tasks::SessionTaskContext;
use async_trait::async_trait;
use codex_git::CreateGhostCommitOptions;
use codex_git::GhostSnapshotReport;
use codex_git::GitToolingError;
use codex_git::capture_ghost_snapshot_report;
use codex_git::create_ghost_commit;
use codex_protocol::models::ResponseItem;
use codex_protocol::user_input::UserInput;
use codex_utils_readiness::Readiness;
use codex_utils_readiness::Token;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing::warn;

pub(crate) struct GhostSnapshotTask {
    token: Token,
}

const SNAPSHOT_WARNING_THRESHOLD: Duration = Duration::from_secs(240);

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
            // Channel used to signal when the snapshot work has finished so the
            // timeout warning task can exit early without sending a warning.
            let (snapshot_done_tx, snapshot_done_rx) = oneshot::channel::<()>();
            let ctx_for_warning = ctx.clone();
            let cancellation_token_for_warning = cancellation_token.clone();
            let session_for_warning = session.clone();
            // Fire a generic warning if the snapshot is still running after
            // three minutes; this helps users discover large untracked files
            // that might need to be added to .gitignore.
            tokio::task::spawn(async move {
                tokio::select! {
                    _ = tokio::time::sleep(SNAPSHOT_WARNING_THRESHOLD) => {
                        session_for_warning.session
                            .send_event(
                                &ctx_for_warning,
                                EventMsg::Warning(WarningEvent {
                                    message: "Repository snapshot is taking longer than expected. Large untracked or ignored files can slow snapshots; consider adding large files or directories to .gitignore or disabling `undo` in your config.".to_string()
                                }),
                            )
                            .await;
                    }
                    _ = snapshot_done_rx => {}
                    _ = cancellation_token_for_warning.cancelled() => {}
                }
            });

            let ctx_for_task = ctx.clone();
            let cancelled = tokio::select! {
                _ = cancellation_token.cancelled() => true,
                _ = async {
                    let repo_path = ctx_for_task.cwd.clone();
                    // First, compute a snapshot report so we can warn about
                    // large untracked directories before running the heavier
                    // snapshot logic.
                    if let Ok(Ok(report)) = tokio::task::spawn_blocking({
                        let repo_path = repo_path.clone();
                        move || {
                            let options = CreateGhostCommitOptions::new(&repo_path);
                            capture_ghost_snapshot_report(&options)
                        }
                    })
                    .await
                        && let Some(message) = format_large_untracked_warning(&report) {
                                session
                                    .session
                                    .send_event(
                                        &ctx_for_task,
                                        EventMsg::Warning(WarningEvent { message }),
                                    )
                                    .await;
                            }

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
                                    ghost_commit: ghost_commit.clone(),
                                }])
                                .await;
                            info!("ghost commit captured: {}", ghost_commit.id());
                        }
                        Ok(Err(err)) => match err {
                            GitToolingError::NotAGitRepository { .. } => info!(
                                sub_id = ctx_for_task.sub_id.as_str(),
                                "skipping ghost snapshot because current directory is not a Git repository"
                            ),
                            _ => {
                                warn!(
                                    sub_id = ctx_for_task.sub_id.as_str(),
                                    "failed to capture ghost snapshot: {err}"
                                );
                            }
                        },
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

            let _ = snapshot_done_tx.send(());

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

fn format_large_untracked_warning(report: &GhostSnapshotReport) -> Option<String> {
    if report.large_untracked_dirs.is_empty() {
        return None;
    }
    const MAX_DIRS: usize = 3;
    let mut parts: Vec<String> = Vec::new();
    for dir in report.large_untracked_dirs.iter().take(MAX_DIRS) {
        parts.push(format!("{} ({} files)", dir.path.display(), dir.file_count));
    }
    if report.large_untracked_dirs.len() > MAX_DIRS {
        let remaining = report.large_untracked_dirs.len() - MAX_DIRS;
        parts.push(format!("{remaining} more"));
    }
    Some(format!(
        "Repository snapshot encountered large untracked directories: {}. This can slow Codex; consider adding these paths to .gitignore or disabling undo in your config.",
        parts.join(", ")
    ))
}
