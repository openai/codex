//! One inbox and channel selection per live acting agent. Its isolated verdict
//! conversation can be retired and replaced while the actor stays loaded.
use std::sync::Arc;
use std::sync::PoisonError;
use std::sync::Weak;

use anyhow::Context;
use codex_analytics::GuardianReviewSessionKind;
use codex_core::CodexThread;
use codex_core::ThreadManager;
use codex_core::guardian_review::PreparedGuardianContext;
use codex_core::guardian_review::ReviewPreparationError;
use codex_core::guardian_review::ReviewRetrieval;
use codex_guardian_reviewer::ReviewerTasks;
use codex_guardian_reviewer::SessionDisposition;
use codex_message_board::AgentMessageBoard;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::protocol::HistoryPosition;
use tokio::sync::Mutex;
use tokio::sync::OwnedMutexGuard;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use crate::LiveRetrievalBinding;
use crate::RootEvidence;
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
        retrieval: ReviewRetrieval,
        tree: Arc<TreeDelivery>,
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
            lifetime: Default::default(),
        }
    }

    pub(super) async fn shutdown(&self) {
        self.lifetime.cancellation.cancel();
        self.lifetime.tasks.close();
        self.lifetime.tasks.wait().await;
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
        let delivery = state
            .tree
            .get()
            .ok_or(ReviewPreparationError::EvidenceUnavailable)?;
        let binding = LiveRetrievalBinding::new(board, self.actor, self.path.clone(), delivery)
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

    /// Reserve this actor's reviewer for a verdict against the root intake that caught up.
    pub async fn reserve(
        &self,
        routed: &RootEvidence,
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
            let data = root.thread_extension_data();
            let worker = data
                .get::<RootWorker>()
                .ok_or(ReviewPreparationError::EvidenceUnavailable)?;
            let evidence = data
                .get::<RootEvidence>()
                .ok_or(ReviewPreparationError::EvidenceUnavailable)?;
            if !std::ptr::eq(routed, evidence.as_ref()) {
                return Err(ReviewPreparationError::EvidenceUnavailable);
            }
            let (mut conversation, kind) = match slot.take() {
                Some(previous) if setup.matches_session_configuration(&previous.runtime) => {
                    (previous, GuardianReviewSessionKind::TrunkReused)
                }
                _ => {
                    let binding = self.binding(Arc::clone(&worker.reader))?;
                    let stop = self.lifetime.cancellation.child_token();
                    let conversation =
                        Conversation::start(&manager, setup, &self.lifetime, binding, stop.clone())
                            .await
                            .map_err(|_| ReviewPreparationError::ReviewerUnavailable)?;
                    self.observe_inbox(
                        Arc::clone(&worker.reader),
                        &conversation.inbox,
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

/// The first attempt uses the request-time position. Every later attempt flushes and
/// captures again after core has sampled its new authorization revision.
struct BoundActorReview {
    actor: Arc<ActorReviewer>,
    initial: Mutex<Option<oneshot::Receiver<RequestCutoff>>>,
}

struct RequestCutoff {
    evidence: Arc<RootEvidence>,
    position: HistoryPosition,
    revisions: (u64, u64),
}

async fn root_revisions(root: &CodexThread) -> (u64, u64) {
    let history = root.conversation_history_snapshot().await;
    (
        history.user_message_revision(),
        history.guardian_review_context_revision(),
    )
}

impl codex_core::guardian_review::AttachedReviewer for ActorReviewer {
    fn bind(self: Arc<Self>) -> codex_core::guardian_review::BoundReviewer {
        let (mut sender, initial) = oneshot::channel();
        let actor = Arc::clone(&self);
        self.lifetime.tasks.spawn(async move {
            let capture = async {
                let manager = actor.manager.upgrade().context("Guardian host stopped")?;
                let root = manager.get_thread(actor.root).await?;
                let evidence = root
                    .thread_extension_data()
                    .get::<RootEvidence>()
                    .context("Guardian intake unavailable")?;
                let revisions = root_revisions(&root).await;
                let position = evidence.capture_cutoff().await?;
                anyhow::Ok(RequestCutoff {
                    evidence,
                    position,
                    revisions,
                })
            };
            tokio::select! {
                biased;
                _ = sender.closed() => (),
                _ = actor.lifetime.cancellation.cancelled() => (),
                result = capture => {
                    if let Ok(cutoff) = result {
                        let _ = sender.send(cutoff);
                    }
                }
            }
        });
        codex_core::guardian_review::BoundReviewer {
            retrieval: self.retrieval.clone(),
            preparation: Arc::new(BoundActorReview {
                actor: self,
                initial: Mutex::new(Some(initial)),
            }),
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
            let prepare = async {
                let manager = self
                    .actor
                    .manager
                    .upgrade()
                    .ok_or(ReviewPreparationError::ReviewerUnavailable)?;
                let root = manager
                    .get_thread(self.actor.root)
                    .await
                    .map_err(|_| ReviewPreparationError::EvidenceUnavailable)?;
                let evidence = root
                    .thread_extension_data()
                    .get::<RootEvidence>()
                    .ok_or(ReviewPreparationError::EvidenceUnavailable)?;
                let initial = self.initial.lock().await.take();
                let cutoff = if let Some(initial) = initial {
                    let initial = initial
                        .await
                        .map_err(|_| ReviewPreparationError::EvidenceUnavailable)?;
                    if !Arc::ptr_eq(&initial.evidence, &evidence) {
                        return Err(ReviewPreparationError::EvidenceUnavailable);
                    }
                    // Authorization may also advance between binding and routing the first attempt.
                    (root_revisions(&root).await == initial.revisions).then_some(initial.position)
                } else {
                    None
                };
                let ready = evidence
                    .request_through(cutoff)
                    .await
                    .map_err(|_| ReviewPreparationError::EvidenceUnavailable)?;
                ready
                    .await
                    .map_err(|_| ReviewPreparationError::EvidenceUnavailable)?;
                let reviewer = self.actor.reserve(&evidence, setup, cancel).await?;
                Ok(Box::new(reviewer) as Box<dyn codex_core::guardian_review::PreparedAssessment>)
            };
            tokio::select! {
                biased;
                _ = self.actor.lifetime.cancellation.cancelled() => Err(ReviewPreparationError::Cancelled),
                _ = cancel.cancelled() => Err(ReviewPreparationError::Cancelled),
                _ = setup.history_reset.cancelled() => Err(ReviewPreparationError::Cancelled),
                result = prepare => result,
            }
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
