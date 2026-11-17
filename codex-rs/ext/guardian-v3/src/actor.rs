//! One isolated verdict conversation per acting agent. Only a completed reusable turn
//! returns its conversation to the slot; dropping a reservation retires its thread.
use std::sync::Arc;
use std::sync::Weak;

use anyhow::Context;
use codex_core::ThreadManager;
use codex_core::guardian_review::PreparedGuardianContext;
use codex_guardian_reviewer::ReviewerTasks;
use codex_guardian_reviewer::SessionDisposition;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use tokio::sync::Mutex;
use tokio::sync::OwnedMutexGuard;
use tokio_util::sync::CancellationToken;

use crate::LiveRetrievalBinding;
use crate::conversation::Conversation;
use crate::host::RootWorker;

pub struct ActorReviewer {
    manager: Weak<ThreadManager>,
    root: ThreadId,
    actor: ThreadId,
    path: AgentPath,
    slot: Arc<Mutex<Option<Conversation>>>,
    lifetime: ReviewerTasks,
}

pub struct PreparedReviewer {
    conversation: Conversation,
    slot: OwnedMutexGuard<Option<Conversation>>,
}

impl PreparedReviewer {
    pub fn thread_id(&self) -> ThreadId {
        self.conversation.thread.startup_metadata().thread_id
    }

    pub fn finish(mut self, disposition: SessionDisposition) {
        if disposition == SessionDisposition::Reusable {
            *self.slot = Some(self.conversation);
        }
    }
}

impl ActorReviewer {
    pub(super) fn new(
        manager: Weak<ThreadManager>,
        root: ThreadId,
        actor: ThreadId,
        path: AgentPath,
    ) -> Self {
        Self {
            manager,
            root,
            actor,
            path,
            slot: Default::default(),
            lifetime: Default::default(),
        }
    }

    pub(super) async fn shutdown(&self) {
        self.lifetime.cancellation.cancel();
        self.lifetime.tasks.close();
        self.lifetime.tasks.wait().await;
    }

    /// Reserve this actor's reviewer for one verdict without starting a model turn.
    pub async fn reserve(
        &self,
        setup: &PreparedGuardianContext,
        cancel: &CancellationToken,
    ) -> anyhow::Result<PreparedReviewer> {
        let _task = self.lifetime.tasks.token();
        let reserve = async {
            let mut slot = self.slot.clone().lock_owned().await;
            let conversation = match slot.take() {
                Some(previous) if setup.matches_session_configuration(&previous.runtime) => {
                    previous
                }
                _ => {
                    let manager = self.manager.upgrade().context("Guardian host stopped")?;
                    let root = manager.get_thread(self.root).await?;
                    let worker = root
                        .thread_extension_data()
                        .get::<RootWorker>()
                        .context("Guardian board unavailable")?;
                    let binding = LiveRetrievalBinding::new(
                        worker.reader.clone(),
                        self.actor,
                        self.path.clone(),
                    )?;
                    Conversation::start(&manager, setup, &self.lifetime, binding).await?
                }
            };
            Ok(PreparedReviewer { conversation, slot })
        };
        tokio::select! {
            biased;
            _ = self.lifetime.cancellation.cancelled() => anyhow::bail!("Guardian actor stopped"),
            _ = setup.history_reset.cancelled() => anyhow::bail!("Guardian actor history reset"),
            _ = cancel.cancelled() => anyhow::bail!("Guardian review cancelled"),
            result = reserve => result,
        }
    }
}
