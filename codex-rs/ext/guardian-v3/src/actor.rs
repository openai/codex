//! One isolated verdict conversation per acting agent. Only a completed reusable turn
//! returns its conversation to the slot; dropping a reservation retires its thread.
use std::sync::Arc;
use std::sync::Weak;

use anyhow::Context;
use codex_core::CodexThread;
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
use crate::RootEvidence;
use crate::conversation::Conversation;
use crate::host::RootWorker;
use crate::inbox::ActorInbox;
use codex_message_board::AgentMessageBoard;

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

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "idle delivery and verdicts must have exclusive access to the same reviewer conversation"
    )]
    fn observe_inbox(
        &self,
        board: Arc<dyn AgentMessageBoard>,
        inbox: &ActorInbox,
        thread: Arc<CodexThread>,
        stop: CancellationToken,
    ) {
        let slot = Arc::clone(&self.slot);
        let actor = self.actor;
        let mut updates = inbox.subscribe();
        self.lifetime.tasks.spawn(async move {
            let deliver = async {
                while updates.changed().await.is_ok() {
                    let mut slot = slot.lock().await;
                    // Restore this reviewer's conversation only after delivery succeeds.
                    // Cancellation can interrupt injection after history has changed.
                    let Some(mut conversation) =
                        slot.take_if(|current| Arc::ptr_eq(&current.thread, &thread))
                    else {
                        break;
                    };
                    if conversation.deliver(actor, board.as_ref()).await.is_err() {
                        tracing::warn!(%actor, "Guardian retired a reviewer after evidence delivery failed");
                        break;
                    }
                    *slot = Some(conversation);
                }
            };
            tokio::select! {
                biased;
                _ = stop.cancelled() => (),
                () = thread.wait_until_terminated() => (),
                () = deliver => (),
            }
        });
    }

    /// Reserve this actor's reviewer for one verdict without starting a model turn.
    pub async fn reserve(
        &self,
        setup: &PreparedGuardianContext,
        cancel: &CancellationToken,
    ) -> anyhow::Result<PreparedReviewer> {
        let _task = self.lifetime.tasks.token();
        let reserve = async {
            let manager = self.manager.upgrade().context("Guardian host stopped")?;
            let root = manager.get_thread(self.root).await?;
            let data = root.thread_extension_data();
            let worker = data
                .get::<RootWorker>()
                .context("Guardian board unavailable")?;
            let evidence = data
                .get::<RootEvidence>()
                .context("Guardian intake unavailable")?;
            let inbox = evidence.delivery.inbox(self.actor);
            let mut slot = self.slot.clone().lock_owned().await;
            let mut conversation = match slot.take() {
                Some(previous)
                    if Arc::ptr_eq(&previous.inbox, &inbox)
                        && setup.matches_session_configuration(&previous.runtime) =>
                {
                    previous
                }
                _ => {
                    let binding = LiveRetrievalBinding::new(
                        worker.reader.clone(),
                        self.actor,
                        self.path.clone(),
                    )?;
                    let stop = self.lifetime.cancellation.child_token();
                    let conversation = Conversation::start(
                        &manager,
                        setup,
                        &self.lifetime,
                        binding,
                        Arc::clone(&inbox),
                        stop.clone(),
                    )
                    .await?;
                    self.observe_inbox(
                        Arc::clone(&worker.reader),
                        &inbox,
                        Arc::clone(&conversation.thread),
                        stop,
                    );
                    conversation
                }
            };
            conversation
                .deliver(self.actor, worker.reader.as_ref())
                .await?;
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
