//! One isolated verdict conversation per acting agent. Only a completed reusable turn
//! returns its conversation to the slot; dropping a reservation retires its thread.
use std::sync::Arc;
use std::sync::Weak;

use anyhow::Context;
use codex_analytics::GuardianReviewSessionKind;
use codex_core::CodexThread;
use codex_core::ThreadManager;
use codex_core::guardian_review::PreparedGuardianContext;
use codex_core::guardian_review::ReviewPreparationError;
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
    kind: GuardianReviewSessionKind,
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
    ) -> Result<PreparedReviewer, ReviewPreparationError> {
        let _task = self.lifetime.tasks.token();
        let reserve = async {
            let manager = self
                .manager
                .upgrade()
                .ok_or(ReviewPreparationError::ReviewerUnavailable)?;
            let root = manager
                .get_thread(self.root)
                .await
                .map_err(|_| ReviewPreparationError::EvidenceUnavailable)?;
            let data = root.thread_extension_data();
            let worker = data
                .get::<RootWorker>()
                .ok_or(ReviewPreparationError::EvidenceUnavailable)?;
            let evidence = data
                .get::<RootEvidence>()
                .ok_or(ReviewPreparationError::EvidenceUnavailable)?;
            let inbox = evidence.delivery.inbox(self.actor);
            let mut slot = self.slot.clone().lock_owned().await;
            let (mut conversation, kind) = match slot.take() {
                Some(previous)
                    if Arc::ptr_eq(&previous.inbox, &inbox)
                        && setup.matches_session_configuration(&previous.runtime) =>
                {
                    (previous, GuardianReviewSessionKind::TrunkReused)
                }
                _ => {
                    let binding = LiveRetrievalBinding::new(
                        worker.reader.clone(),
                        self.actor,
                        self.path.clone(),
                    )
                    .map_err(|_| ReviewPreparationError::EvidenceUnavailable)?;
                    let stop = self.lifetime.cancellation.child_token();
                    let conversation = Conversation::start(
                        &manager,
                        setup,
                        &self.lifetime,
                        binding,
                        Arc::clone(&inbox),
                        stop.clone(),
                    )
                    .await
                    .map_err(|_| ReviewPreparationError::ReviewerUnavailable)?;
                    self.observe_inbox(
                        Arc::clone(&worker.reader),
                        &inbox,
                        Arc::clone(&conversation.thread),
                        stop,
                    );
                    (conversation, GuardianReviewSessionKind::TrunkNew)
                }
            };
            conversation
                .deliver(self.actor, worker.reader.as_ref())
                .await
                .map_err(|_| ReviewPreparationError::DeliveryFailed)?;
            Ok(PreparedReviewer {
                conversation,
                slot,
                kind,
            })
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

/// One intake request shared by all verdict attempts for the same action.
struct BoundActorReview {
    actor: Arc<ActorReviewer>,
    routed: tokio::sync::watch::Receiver<Option<Arc<ActorInbox>>>,
}

impl codex_core::guardian_review::AttachedReviewer for ActorReviewer {
    fn bind(self: Arc<Self>) -> codex_core::guardian_review::BoundReviewer {
        let (routed, ready) = tokio::sync::watch::channel(None);
        let actor = Arc::clone(&self);
        self.lifetime.tasks.spawn(async move {
            // Start capture at request time even if a previous verdict occupies the slot.
            let catch_up = async {
                let manager = actor.manager.upgrade().context("Guardian host stopped")?;
                let root = manager.get_thread(actor.root).await?;
                let evidence = root
                    .thread_extension_data()
                    .get::<RootEvidence>()
                    .context("Guardian intake unavailable")?;
                evidence.request_catch_up().await?.await?;
                anyhow::Ok(evidence.delivery.inbox(actor.actor))
            };
            tokio::select! {
                biased;
                _ = routed.closed() => (),
                _ = actor.lifetime.cancellation.cancelled() => (),
                result = catch_up => {
                    if let Ok(inbox) = result {
                        routed.send_replace(Some(inbox));
                    }
                }
            }
        });
        codex_core::guardian_review::BoundReviewer {
            preparation: Arc::new(BoundActorReview {
                actor: self,
                routed: ready,
            }),
            retrieval: crate::BOARD_RETRIEVAL,
        }
    }
}

impl codex_core::guardian_review::ReviewerPreparation for BoundActorReview {
    fn prepare<'a>(
        &'a self,
        setup: &'a PreparedGuardianContext,
        cancel: &'a CancellationToken,
    ) -> codex_extension_api::ExtensionFuture<
        'a,
        Result<Box<dyn codex_core::guardian_review::PreparedAssessment>, ReviewPreparationError>,
    > {
        Box::pin(async move {
            let mut routed = self.routed.clone();
            let inbox = tokio::select! {
                biased;
                _ = cancel.cancelled() => return Err(ReviewPreparationError::Cancelled),
                _ = setup.history_reset.cancelled() => return Err(ReviewPreparationError::Cancelled),
                result = routed.wait_for(Option::is_some) => result.ok().and_then(|value| value.clone()),
            }.ok_or(ReviewPreparationError::EvidenceUnavailable)?;
            // All matching originals through the requested position are queued. The
            // reservation drains this guardian's inbox before submitting the action.
            let reviewer = self.actor.reserve(setup, cancel).await?;
            if !Arc::ptr_eq(&inbox, &reviewer.conversation.inbox) {
                return Err(ReviewPreparationError::EvidenceUnavailable);
            }
            Ok(Box::new(reviewer) as Box<dyn codex_core::guardian_review::PreparedAssessment>)
        })
    }
}

impl codex_core::guardian_review::PreparedAssessment for PreparedReviewer {
    fn runtime(&self) -> &codex_core::guardian_review::GuardianReviewSession {
        &self.conversation.runtime
    }
    fn session_kind(&self) -> GuardianReviewSessionKind {
        self.kind
    }
    fn finish(self: Box<Self>, disposition: SessionDisposition) {
        PreparedReviewer::finish(*self, disposition);
    }
}
