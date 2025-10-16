use std::sync::Arc;

use codex_git_tooling::CreateGhostCommitOptions;
use codex_git_tooling::create_ghost_commit;
use codex_git_tooling::restore_ghost_commit;
use codex_utils_readiness::Readiness;
use codex_utils_readiness::ReadinessFlag;
use tokio::task;
use tracing::info;

use crate::codex::Session;
use crate::codex::TurnContext;

impl Session {
    /// Initialize pre-tool readiness and kick off the ghost snapshot worker for this turn.
    /// No-op for review mode or when snapshots are disabled for this session.
    pub async fn init_pretool_from_turn(self: &Arc<Self>, turn_context: &TurnContext) {
        if turn_context.is_review_mode {
            return;
        }
        {
            let state = self.state.lock().await;
            if state.undo_snapshots_disabled {
                return;
            }
        }

        let flag = Arc::new(ReadinessFlag::new());
        let token = match flag.subscribe().await {
            Ok(tok) => tok,
            Err(_) => return,
        };

        {
            if let Some(active) = self.active_turn.lock().await.as_mut() {
                let mut ts = active.turn_state.lock().await;
                ts.pretool_flag = Some(Arc::clone(&flag));
                ts.pretool_sub_token = Some(token);
            }
        }

        let cwd = turn_context.cwd.clone();
        let sess = Arc::clone(self);
        // Capture the readiness flag and token so we can always mark readiness,
        // avoiding races with locks held by await_pretool_ready().
        let ready_flag = Arc::clone(&flag);
        let ready_token = token;
        tokio::spawn(async move {
            // Perform git operations on a blocking thread.
            let res = task::spawn_blocking(move || {
                let options = CreateGhostCommitOptions::new(&cwd);
                create_ghost_commit(&options)
            })
            .await;

            // Mark flag as ready in all cases, unconditionally using the captured token.
            let _ = ready_flag.mark_ready(ready_token).await;

            match res {
                Ok(Ok(commit)) => {
                    let short_id: String = commit.id().chars().take(8).collect();
                    info!("created ghost snapshot {short_id}");
                    let mut state = sess.state.lock().await;
                    state.push_undo_snapshot(commit);
                    state.undo_snapshots_disabled = false;
                }
                Ok(Err(err)) => {
                    let mut state = sess.state.lock().await;
                    state.undo_snapshots_disabled = true;
                    let msg = match &err {
                        codex_git_tooling::GitToolingError::NotAGitRepository { .. } => {
                            "Snapshots disabled: current directory is not a Git repository."
                                .to_string()
                        }
                        _ => format!("Snapshots disabled after error: {err}"),
                    };
                    let _ = sess.notify_background_event("", msg).await;
                }
                Err(join_err) => {
                    let mut state = sess.state.lock().await;
                    state.undo_snapshots_disabled = true;
                    let msg = format!("Snapshot worker failed to run: {join_err}");
                    let _ = sess.notify_background_event("", msg).await;
                }
            }
        });
    }

    /// Await pre-tool readiness (ghost snapshot) once. Subsequent calls resolve immediately.
    pub async fn await_pretool_ready(&self) {
        let flag_opt = {
            let active = self.active_turn.lock().await;
            match active.as_ref() {
                Some(at) => {
                    let ts = at.turn_state.lock().await;
                    ts.pretool_flag.clone()
                }
                None => None,
            }
        };

        if let Some(flag) = flag_opt {
            flag.wait_ready().await;
        }
    }

    /// Restore the workspace to the last ghost snapshot, if any.
    pub async fn undo_last_snapshot(&self, cwd: &std::path::Path, sub_id: &str) {
        let maybe_commit = {
            let mut state = self.state.lock().await;
            state.pop_undo_snapshot()
        };

        let Some(commit) = maybe_commit else {
            self.notify_background_event(sub_id, "No snapshot available to undo.")
                .await;
            return;
        };

        let commit_id = commit.id().to_string();
        match task::spawn_blocking({
            let cwd = cwd.to_path_buf();
            let commit = commit.clone();
            move || restore_ghost_commit(&cwd, &commit)
        })
        .await
        {
            Ok(Ok(())) => {
                let short_id: String = commit_id.chars().take(8).collect();
                let msg = format!("Restored workspace to snapshot {short_id}");
                self.notify_background_event(sub_id, msg).await;
            }
            Ok(Err(err)) => {
                let mut state = self.state.lock().await;
                state.push_back_undo_snapshot(commit);
                let msg = format!("Failed to restore snapshot: {err}");
                self.notify_background_event(sub_id, msg).await;
            }
            Err(join_err) => {
                let mut state = self.state.lock().await;
                state.push_back_undo_snapshot(commit);
                let msg = format!("Failed to restore snapshot: {join_err}");
                self.notify_background_event(sub_id, msg).await;
            }
        }
    }
}
