//! One worker owns each treatment conversation and commits only completed requests.
//! Admission reserves order before async preparation; permits bound active and queued work.
//! Backend replacement invalidates its generation without changing snapshot shutdown policy.
//! Oversized retained requests rebuild from fresh evidence before the model's hard limit.
//! Prepared requests own their history; only progress remains in state during sampling.
//! Host-only instruction delivery metadata is committed with its completed input messages.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;

use codex_context_fragments::RenderedFragment;
use codex_core::CodexThread;
use codex_extension_api::ExtensionMetrics;
use codex_guardian_reviewer::ConversationState;
use codex_history::ResponseItemEnvelope;
use codex_protocol::models::ResponseItem;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use super::authorization::ScoreAuthorization;
use super::sampler::LunaSampler;
use super::sampler::LunaSamplerError;
use super::sampler::LunaSamplingRequest;
use super::transcript::CollectedTranscript;

const MAX_OUTSTANDING: usize = 16;

pub(super) struct ConversationBackend {
    queue: Mutex<mpsc::UnboundedSender<QueuedRequest>>,
    capacity: Arc<Semaphore>,
    generation: Arc<()>,
}

struct QueuedRequest {
    request: oneshot::Receiver<ConversationRequest>,
    _permit: OwnedSemaphorePermit,
}

pub(super) struct Reservation {
    request: oneshot::Sender<ConversationRequest>,
    pub(super) generation: Weak<()>,
}

pub(super) struct ConversationRequest {
    pub(super) evidence: CollectedTranscript,
    pub(super) sampling: LunaSamplingRequest,
    pub(super) reset_token_limit: usize,
    pub(super) authorization: ScoreAuthorization,
    pub(super) thread: Arc<CodexThread>,
    pub(super) ready: oneshot::Sender<Result<String, LunaSamplerError>>,
    pub(super) metrics: Option<Arc<dyn ExtensionMetrics>>,
}

// Authorization covers model settings; local config changes replace the backend.
#[derive(PartialEq)]
struct ReuseKey {
    authorization: ScoreAuthorization,
    instructions: RenderedFragment,
    parent_compaction: Option<ResponseItem>,
}

impl ConversationBackend {
    pub(super) fn new(sampler: Arc<LunaSampler>) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let generation = Arc::new(());
        tokio::spawn(run(receiver, sampler, Arc::downgrade(&generation)));
        Self {
            queue: Mutex::new(sender),
            capacity: Arc::new(Semaphore::new(MAX_OUTSTANDING)),
            generation,
        }
    }

    pub(super) fn reserve(
        &self,
        observe: impl FnOnce() -> usize,
    ) -> (usize, Result<Reservation, LunaSamplerError>) {
        // Keep the tool index and queue position in the same admission order.
        let queue = self
            .queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let index = observe();
        let reservation = (|| {
            let permit = Arc::clone(&self.capacity)
                .try_acquire_owned()
                .map_err(|_| LunaSamplerError::QueueFull)?;
            let (sender, request) = oneshot::channel();
            queue
                .send(QueuedRequest {
                    request,
                    _permit: permit,
                })
                .map_err(|_| LunaSamplerError::Superseded)?;
            Ok(Reservation {
                request: sender,
                generation: Arc::downgrade(&self.generation),
            })
        })();
        (index, reservation)
    }
}

impl Reservation {
    pub(super) fn submit(self, request: ConversationRequest) {
        if let Err(request) = self.request.send(request) {
            let _ = request.ready.send(Err(LunaSamplerError::Superseded));
        }
    }
}

async fn run(
    mut queue: mpsc::UnboundedReceiver<QueuedRequest>,
    sampler: Arc<LunaSampler>,
    generation: Weak<()>,
) {
    // Only completed requests have reusable history and a matching reuse key.
    let mut committed: Option<(ReuseKey, ConversationState<Vec<ResponseItemEnvelope>>)> = None;
    while let Some(queued) = queue.recv().await {
        // A preparation error drops its sender, preserving the order of later observations.
        let Ok(request) = queued.request.await else {
            continue;
        };
        if generation.strong_count() == 0
            || !request.authorization.is_current(&request.thread).await
        {
            committed = None;
            let _ = request.ready.send(Err(LunaSamplerError::Superseded));
            continue;
        }
        let key = ReuseKey {
            authorization: request.authorization.clone(),
            instructions: request.sampling.instructions.clone(),
            parent_compaction: request.sampling.parent_compaction.clone(),
        };
        let mut state = committed
            .take()
            .filter(|(previous, _)| previous == &key)
            .map(|(_, state)| state)
            .unwrap_or_default();
        let had_history = state.snapshot().is_some();
        let prepared = request
            .sampling
            .prepare_retained(&request.evidence, &mut state, sampler.max_input_tokens())
            .filter(|prepared| {
                prepared.existing_context_tokens == 0
                    || prepared.input_tokens <= request.reset_token_limit
            })
            .or_else(|| {
                if !had_history {
                    return None;
                }
                state = ConversationState::default();
                request.sampling.prepare_retained(
                    &request.evidence,
                    &mut state,
                    sampler.max_input_tokens(),
                )
            });
        let Some(mut prepared) = prepared else {
            let _ = request.ready.send(Err(LunaSamplerError::InputTooLarge));
            continue;
        };
        super::metrics::record_section_costs(
            request.metrics.as_deref(),
            prepared.section_costs.iter().copied(),
        );
        super::metrics::record_request_tokens(
            request.metrics.as_deref(),
            prepared.existing_context_tokens,
            prepared.input_tokens,
        );
        drop(request.evidence);
        let cursor = prepared.cursor;
        let pending_truncations = std::mem::take(&mut prepared.truncations);
        let history = sampler.sample_retained(prepared, request.ready).await;
        if let Some(history) = history
            && generation.strong_count() > 0
            && request.authorization.is_current(&request.thread).await
        {
            let mut truncations = super::truncation::ClassificationTruncations::default();
            truncations.extend(pending_truncations);
            truncations.emit(request.metrics.as_deref());
            state.complete_review(cursor);
            state.commit_snapshot(history);
            committed = Some((key, state));
        }
        // The permit is released only after completion and commit, not after the early score.
        drop(queued._permit);
    }
}
