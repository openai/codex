//! A root keeps its enrollment across settings changes, but each stretch of
//! Auto Review owns a separate board, housekeeper and set of reviewers.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::Weak;

use anyhow::Context;
use codex_core::config::Config;
use codex_core::guardian_review::ShadowReviewer;
use codex_extension_api::ConfigContributor;
use codex_extension_api::ExtensionData;
use codex_guardian_reviewer::routes_approval_policy_to_guardian;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use tokio_util::task::TaskTracker;

use super::ActorReviewer;
use super::BoardSettings;
use super::BoardStorage;
use super::Host;
use super::IntakeStart;
use super::RootWorker;
use super::TreeDelivery;

pub(super) struct Actor {
    pub(super) root: ThreadId,
    pub(super) actor: ThreadId,
    pub(super) path: AgentPath,
}

enum State {
    // Covers startup and the published worker. Both share one cancellation token.
    Running(Arc<TreeDelivery>),
    Off,
    PendingStart(IntakeStart),
}

pub(super) struct Enrollment {
    state: Mutex<State>,
    tasks: TaskTracker,
}

impl Default for Enrollment {
    fn default() -> Self {
        Self {
            state: Mutex::new(State::PendingStart(IntakeStart::Initial(
                codex_protocol::protocol::ThreadHistoryMode::Paginated,
                codex_history::HistoryInitialization::New,
            ))),
            tasks: TaskTracker::new(),
        }
    }
}

impl Enrollment {
    pub(super) fn stop(&self, thread: &ExtensionData) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let State::Running(tree) = std::mem::replace(&mut *state, State::Off) else {
            return;
        };
        // Serialize cancellation with publishing a reviewer or worker at startup.
        let actors = tree.actors.lock().unwrap_or_else(PoisonError::into_inner);
        tree.stop.cancel();
        let reviewers: Vec<_> = actors.iter().filter_map(Weak::upgrade).collect();
        thread.remove::<ShadowReviewer>();
        thread.remove::<ActorReviewer>();
        let worker = thread.remove::<RootWorker>();
        // Settings callbacks are synchronous. Drain accepted writes and reviewer
        // sessions off the live path; thread shutdown joins this cleanup.
        self.tasks.spawn(async move {
            for reviewer in reviewers {
                reviewer.shutdown().await;
            }
            if let Some(worker) = worker {
                worker.lifetime.tasks.close();
                worker.lifetime.tasks.wait().await;
            }
        });
    }

    pub(super) async fn drain(&self) {
        self.tasks.close();
        self.tasks.wait().await;
    }
}

impl ConfigContributor<Config> for Host {
    fn on_config_changed(
        &self,
        _session: &ExtensionData,
        thread: &ExtensionData,
        _previous: &Config,
        config: &Config,
    ) {
        let Some(enrollment) = thread.get::<Enrollment>() else {
            return;
        };
        if !self.enabled(config) {
            enrollment.stop(thread);
        } else {
            let mut state = enrollment
                .state
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if matches!(*state, State::Off) {
                *state = State::PendingStart(IntakeStart::Restart);
            }
        }
    }
}

impl Host {
    fn attach_shadow(&self, thread: &ExtensionData, identity: &Actor, tree: &Arc<TreeDelivery>) {
        let mut actors = tree.actors.lock().unwrap_or_else(PoisonError::into_inner);
        if tree.stop.is_cancelled() {
            return;
        }
        let Some(board) = tree.board.get() else {
            return;
        };
        let cancellation = tree.stop.child_token();
        let reviewer = self.reviewer(identity, tree, &board.retrieval, cancellation.clone());
        actors.retain(|actor| actor.strong_count() > 0);
        actors.push(Arc::downgrade(&reviewer));
        thread.insert_arc(Arc::clone(&reviewer));
        thread.insert(ShadowReviewer {
            reviewer: reviewer.clone(),
            intake_stopped: tree.shadow_intake_stopped.clone(),
            cancellation,
        });
        if identity.actor == identity.root {
            reviewer.schedule_root_prewarm();
            // User-message and turn-end wakes may have arrived before startup.
            super::wake_worker(thread);
        }
    }

    pub(super) fn start_shadow_if_needed(&self, thread: &ExtensionData) {
        let Some(identity) = thread.get::<Actor>() else {
            return;
        };
        let root = identity.root;
        if identity.actor != root {
            // A surviving child joins the new board on its next turn.
            if thread
                .get::<ShadowReviewer>()
                .is_some_and(|r| !r.cancellation.is_cancelled())
            {
                return;
            }
            self.attach_shadow(thread, &identity, &self.tree_delivery(root));
            return;
        }
        let Some(enrollment) = thread.get::<Enrollment>() else {
            return;
        };
        let mut state = enrollment
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let State::PendingStart(start) = *state else {
            return;
        };
        let tree = self.tree_delivery(root);
        *state = State::Running(Arc::clone(&tree));
        let metrics = thread.get::<crate::shadow_metrics::ShadowMetrics>();
        let host = self.clone();
        let current = Arc::clone(&enrollment);
        enrollment.tasks.spawn(async move {
            // On restart the durable tail may already include the enabling turn.
            // Missing it is preferable to delaying the turn or replaying disabled history.
            let startup = async {
                let manager = host.manager.upgrade().context("Guardian host stopped")?;
                let root_thread = manager
                    .get_thread_with_extension::<Enrollment>(root, |e| {
                        std::ptr::eq(e, current.as_ref())
                    })
                    .await?;
                // Config notifications can arrive out of order. Confirm the root
                // still uses Auto Review before opening a board or reading history.
                if matches!(start, IntakeStart::Restart) {
                    let settings = root_thread.thread_settings_snapshot().await;
                    if !routes_approval_policy_to_guardian(
                        settings.approval_policy,
                        settings.approvals_reviewer,
                    ) {
                        let mut state = current.state.lock().unwrap_or_else(PoisonError::into_inner);
                        if !tree.stop.is_cancelled() {
                            *state = State::Off;
                        }
                        return anyhow::Ok(());
                    }
                }
                let thread = root_thread.thread_extension_data();
                let settings = BoardSettings::new(root_thread.config().await.as_ref(), BoardStorage::Memory);
                host.activate(root, &tree, thread, start, &settings).await?;
                host.attach_shadow(thread, &identity, &tree);
                if !tree.stop.is_cancelled() {
                    let event_name = match start {
                        IntakeStart::Initial(..) => "guardian_v3.shadow_intake_started",
                        IntakeStart::Restart => "guardian_v3.shadow_intake_restarted",
                    };
                    tracing::debug!(event.name = event_name, %root, "Guardian shadow intake started");
                }
                anyhow::Ok(())
            };
            if let Some(Err(_)) = tree.stop.run_until_cancelled(startup).await {
                let mut state = current.state.lock().unwrap_or_else(PoisonError::into_inner);
                if !tree.stop.is_cancelled() {
                    *state = State::PendingStart(start);
                    drop(state);
                    if let Some(metrics) = metrics {
                        metrics.stopped("intake");
                    }
                    tracing::warn!(%root, "Guardian shadow intake unavailable until the next turn");
                }
            }
        });
    }
}
