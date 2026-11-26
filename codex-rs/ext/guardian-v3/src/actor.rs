//! One inbox and channel selection per live acting agent. Its isolated verdict
//! conversation can be retired and replaced while the actor stays loaded.
use std::sync::Arc;
use std::sync::PoisonError;
use std::sync::Weak;

use anyhow::Context;
use codex_analytics::GuardianReviewSessionKind;
use codex_core::CodexThread;
use codex_core::ThreadManager;
use codex_core::guardian_review::AttachedReviewer;
use codex_core::guardian_review::PreparedAssessment;
use codex_core::guardian_review::PreparedGuardianContext;
use codex_core::guardian_review::ReviewPreparationError;
use codex_core::guardian_review::ReviewRetrieval;
use codex_core::guardian_review::prepare_review_prewarm;
use codex_guardian_reviewer::ReviewerTasks;
use codex_guardian_reviewer::SessionDisposition;
use codex_message_board::AgentMessageBoard;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use tokio::sync::Mutex;
use tokio::sync::OwnedMutexGuard;
use tokio_util::sync::CancellationToken;

use crate::LiveRetrievalBinding;
use crate::conversation::Conversation;
use crate::host::RootWorker;
use crate::host::TreeDelivery;
use crate::inbox::ActorInbox;

pub struct ActorReviewer {
    manager: Weak<ThreadManager>,
    root: ThreadId,
    actor: ThreadId,
    path: AgentPath,
    retrieval: ReviewRetrieval,
    // Release the tree on actor shutdown, even if a pending request still holds this reviewer.
    delivery: std::sync::Mutex<Option<ActorDelivery>>,
    slot: Arc<Mutex<Option<Conversation>>>,
    lifetime: ReviewerTasks,
}

struct ActorDelivery {
    tree: Arc<TreeDelivery>,
    binding: Option<LiveRetrievalBinding>,
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
        if disposition == SessionDisposition::Reusable
            && !self.conversation.thread.has_shutdown_status()
        {
            self.conversation.kind = GuardianReviewSessionKind::TrunkReused;
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
        retrieval: ReviewRetrieval,
        tree: Arc<TreeDelivery>,
        cancellation: CancellationToken,
    ) -> Self {
        Self {
            manager,
            root,
            actor,
            path,
            retrieval,
            delivery: std::sync::Mutex::new(Some(ActorDelivery {
                tree,
                binding: None,
            })),
            slot: Default::default(),
            lifetime: ReviewerTasks {
                cancellation,
                ..Default::default()
            },
        }
    }

    pub(super) async fn shutdown(&self) {
        self.lifetime.cancellation.cancel();
        self.lifetime.tasks.close();
        self.lifetime.tasks.wait().await;
        // A loaded child may keep its cancelled registration until its next
        // turn. Release the retired conversation and its board handles now.
        if let Ok(mut slot) = self.slot.try_lock() {
            slot.take();
        }
        let delivery = self
            .delivery
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(binding) = delivery.and_then(|delivery| delivery.binding) {
            binding.inbox.close();
        }
    }

    pub(super) fn binding(
        &self,
        board: Arc<dyn AgentMessageBoard>,
    ) -> Result<LiveRetrievalBinding, ReviewPreparationError> {
        let mut current = self.delivery.lock().unwrap_or_else(PoisonError::into_inner);
        let state = current.as_mut().ok_or(ReviewPreparationError::Cancelled)?;
        if let Some(binding) = &state.binding {
            return Ok(binding.clone());
        }
        let published = state
            .tree
            .board
            .get()
            .ok_or(ReviewPreparationError::EvidenceUnavailable)?;
        let binding =
            LiveRetrievalBinding::new(board, self.actor, self.path.clone(), &published.delivery)
                .map_err(|_| ReviewPreparationError::EvidenceUnavailable)?;
        Ok(state.binding.insert(binding).clone())
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

    /// Warm the root conversation without routing evidence or starting a review.
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "prewarm and the first review must share a single conversation startup"
    )]
    pub(super) fn schedule_root_prewarm(self: &Arc<Self>) {
        let actor = Arc::clone(self);
        self.lifetime.tasks.spawn(async move {
            let warm = async {
                let manager = actor.manager.upgrade().context("Guardian host stopped")?;
                let root = manager.get_thread(actor.actor).await?;
                let setup = prepare_review_prewarm(&root, actor.retrieval.clone()).await?;
                let worker = root.thread_extension_data().get::<RootWorker>()
                    .context("Guardian board unavailable")?;
                // A review may already own the slot or have left a reusable conversation.
                if let Ok(mut slot) = actor.slot.try_lock()
                    && slot.is_none()
                {
                    *slot = Some(actor.start(&manager, &setup, &worker).await?);
                }
                anyhow::Ok(())
            };
            if let Some(Err(_)) = actor.lifetime.cancellation.run_until_cancelled(warm).await {
                tracing::debug!(actor = %actor.actor, "Guardian prewarm failed; will start on demand");
            }
        });
    }

    async fn start(
        &self,
        manager: &Arc<ThreadManager>,
        setup: &PreparedGuardianContext,
        worker: &RootWorker,
    ) -> Result<Conversation, ReviewPreparationError> {
        let binding = self.binding(Arc::clone(&worker.reader))?;
        let stop = self.lifetime.cancellation.child_token();
        let conversation = Conversation::start(
            manager,
            setup,
            &self.lifetime,
            binding,
            stop.clone(),
            worker.shadow,
        )
        .await
        .map_err(|_| ReviewPreparationError::ReviewerUnavailable)?;
        self.observe_inbox(
            Arc::clone(&worker.reader),
            &conversation.inbox,
            Arc::clone(&conversation.thread),
            stop,
        );
        Ok(conversation)
    }

    /// Drain this actor's current inbox without waiting for the housekeeper.
    pub async fn reserve(
        &self,
        setup: &PreparedGuardianContext,
        cancel: &CancellationToken,
    ) -> Result<PreparedReviewer, ReviewPreparationError> {
        let _task = self.lifetime.tasks.token();
        let reserve = async {
            let mut slot = self.slot.clone().lock_owned().await;
            let manager = self
                .manager
                .upgrade()
                .ok_or(ReviewPreparationError::ReviewerUnavailable)?;
            let root = manager
                .get_thread(self.root)
                .await
                .map_err(|_| ReviewPreparationError::EvidenceUnavailable)?;
            let worker = root
                .thread_extension_data()
                .get::<RootWorker>()
                .ok_or(ReviewPreparationError::EvidenceUnavailable)?;
            let mut conversation = match slot.take() {
                Some(previous)
                    if !previous.thread.has_shutdown_status()
                        && setup.matches_session_configuration(&previous.runtime) =>
                {
                    previous
                }
                _ => self.start(&manager, setup, &worker).await?,
            };
            conversation
                .deliver(self.actor, worker.reader.as_ref())
                .await
                .map_err(|_| ReviewPreparationError::DeliveryFailed)?;
            Ok(PreparedReviewer { conversation, slot })
        };
        tokio::select! {
            biased;
            _ = self.lifetime.cancellation.cancelled() => Err(ReviewPreparationError::Cancelled),
            _ = setup.history_reset.cancelled() => Err(ReviewPreparationError::Cancelled),
            _ = cancel.cancelled() => Err(ReviewPreparationError::Cancelled),
            result = reserve => result,
        }
    }
}

impl AttachedReviewer for ActorReviewer {
    fn retrieval(&self) -> ReviewRetrieval {
        self.retrieval.clone()
    }

    fn root_thread(&self) -> codex_extension_api::ExtensionFuture<'_, Option<Arc<CodexThread>>> {
        Box::pin(async move {
            if self.actor == self.root {
                return None;
            }
            self.manager.upgrade()?.get_thread(self.root).await.ok()
        })
    }

    fn prepare<'a>(
        &'a self,
        setup: &'a PreparedGuardianContext,
        cancel: &'a CancellationToken,
    ) -> codex_extension_api::ExtensionFuture<
        'a,
        Result<Box<dyn PreparedAssessment>, ReviewPreparationError>,
    > {
        Box::pin(async move {
            let reviewer = self.reserve(setup, cancel).await?;
            Ok(Box::new(reviewer) as Box<dyn PreparedAssessment>)
        })
    }
}

impl PreparedAssessment for PreparedReviewer {
    fn runtime(&self) -> &codex_core::guardian_review::GuardianReviewSession {
        &self.conversation.runtime
    }
    fn session_kind(&self) -> GuardianReviewSessionKind {
        self.conversation.kind
    }
    fn finish(self: Box<Self>, disposition: SessionDisposition) {
        PreparedReviewer::finish(*self, disposition);
    }
}
