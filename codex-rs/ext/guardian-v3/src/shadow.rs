//! Shadow intake starts off the live path and stops when the root leaves Auto Review.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::Weak;

use anyhow::Context;
use codex_core::config::Config;
use codex_core::guardian_review::ShadowReviewer;
use codex_extension_api::ConfigContributor;
use codex_extension_api::ExtensionData;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use tokio_util::task::TaskTracker;

use super::ActorReviewer;
use super::Host;
use super::RootWorker;
use super::TreeDelivery;

pub(super) struct Actor {
    pub(super) root: ThreadId,
    pub(super) actor: ThreadId,
    pub(super) path: AgentPath,
}

#[derive(Default)]
enum State {
    // Covers startup and the published worker. Both share one cancellation token.
    Running(Arc<TreeDelivery>),
    Off,
    #[default]
    PendingStart,
}

#[derive(Default)]
pub(super) struct Enrollment {
    state: Mutex<State>,
    tasks: TaskTracker,
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
        }
    }
}

impl Host {
    fn attach_shadow(&self, thread: &ExtensionData, identity: &Actor, tree: &Arc<TreeDelivery>) {
        let mut actors = tree.actors.lock().unwrap_or_else(PoisonError::into_inner);
        if tree.stop.is_cancelled() || tree.delivery.get().is_none() {
            return;
        }
        let cancellation = tree.stop.child_token();
        let reviewer = self.reviewer(identity, tree, cancellation.clone());
        actors.retain(|actor| actor.strong_count() > 0);
        actors.push(Arc::downgrade(&reviewer));
        thread.insert_arc(Arc::clone(&reviewer));
        thread.insert(ShadowReviewer {
            reviewer: reviewer.clone(),
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
            // The root board may become available after a child starts.
            if thread.get::<ShadowReviewer>().is_some() {
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
        let State::PendingStart = *state else {
            return;
        };
        let tree = self.tree_delivery(root);
        *state = State::Running(Arc::clone(&tree));
        let host = self.clone();
        let current = Arc::clone(&enrollment);
        enrollment.tasks.spawn(async move {
            let startup = async {
                let manager = host.manager.upgrade().context("Guardian host stopped")?;
                let root_thread = manager
                    .get_thread_with_extension::<Enrollment>(root, |e| {
                        std::ptr::eq(e, current.as_ref())
                    })
                    .await?;
                let thread = root_thread.thread_extension_data();
                host.activate(root, &tree, thread, codex_protocol::protocol::ThreadHistoryMode::Paginated, codex_history::HistoryInitialization::New).await?;
                host.attach_shadow(thread, &identity, &tree);
                if !tree.stop.is_cancelled() {
                    tracing::debug!(event.name = "guardian_v3.shadow_intake_started", %root, "Guardian shadow intake started");
                }
                anyhow::Ok(())
            };
            if let Some(Err(_)) = tree.stop.run_until_cancelled(startup).await {
                let mut state = current.state.lock().unwrap_or_else(PoisonError::into_inner);
                if !tree.stop.is_cancelled() {
                    *state = State::PendingStart;
                    drop(state);
                    tracing::warn!(%root, "Guardian shadow intake unavailable until the next turn");
                }
            }
        });
    }
}
