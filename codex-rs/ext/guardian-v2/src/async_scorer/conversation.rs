//! Concurrent classifications fork the latest compatible completed conversation.
//! Admission assigns order and reserves capacity atomically before async preparation.
//! Only newer completions replace history.
//! Permits bound preparation and sampling through full response completion.
//! Backend replacement invalidates its generation.
//! Oversized retained requests rebuild from fresh evidence before the model's hard limit.
//! Preparation exposes the complete input without starting model execution.
//! Each request owns its fork; failed or cancelled requests leave committed history intact.
//! Host-only instruction delivery metadata is committed with its completed input messages.
//! Expired classifications release their transport without committing partial history.
//! Execution preflight skips work superseded in both score and history before sampling starts.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;
use std::time::SystemTime;

use codex_context_fragments::RenderedFragment;
use codex_core::CodexThread;
use codex_extension_api::ExtensionMetrics;
use codex_guardian_reviewer::ConversationState;
use codex_history::ResponseItemEnvelope;
use codex_protocol::models::ResponseItem;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;
use tokio::sync::oneshot;

use super::authorization::ScoreAuthorization;
use super::request::PreparedRequest;
use super::sampler::ConversationSample;
use super::sampler::LunaSampler;
use super::sampler::LunaSamplerError;
use super::sampler::LunaSamplingRequest;
use super::sampler::SampledScore;
use super::score::PendingScore;
use super::transcript::CollectedTranscript;

const MAX_OUTSTANDING: usize = 16;

pub(super) struct ConversationBackend {
    history: Arc<Mutex<Option<Arc<CompletedConversation>>>>,
    sampler: Arc<LunaSampler>,
    capacity: Mutex<Arc<Semaphore>>,
    generation: Arc<()>,
}

struct CompletedConversation {
    index: usize,
    key: ReuseKey,
    state: ConversationState<Vec<ResponseItemEnvelope>>,
}

pub(super) struct Reservation {
    index: usize,
    history: Arc<Mutex<Option<Arc<CompletedConversation>>>>,
    sampler: Arc<LunaSampler>,
    _permit: OwnedSemaphorePermit,
    pub(super) generation: Weak<()>,
}

pub(super) struct ConversationRequest {
    pub(super) evidence: CollectedTranscript,
    pub(super) sampling: LunaSamplingRequest,
    pub(super) reset_token_limit: usize,
    pub(super) authorization: ScoreAuthorization,
    pub(super) thread: Arc<CodexThread>,
    pub(super) ready: oneshot::Sender<Result<SampledScore, LunaSamplerError>>,
    pub(super) metrics: Option<Arc<dyn ExtensionMetrics>>,
    pub(super) pending_score: PendingScore,
    pub(super) sampled_at: SystemTime,
}

/// Owns an admitted history fork and its permit until execution completes or it is dropped.
pub(super) struct PreparedConversation {
    reservation: Reservation,
    pub(super) request: PreparedRequest,
    sampling: LunaSamplingRequest,
    key: ReuseKey,
    state: ConversationState<Vec<ResponseItemEnvelope>>,
    thread: Arc<CodexThread>,
    ready: oneshot::Sender<Result<SampledScore, LunaSamplerError>>,
    metrics: Option<Arc<dyn ExtensionMetrics>>,
    pending_score: PendingScore,
    sampled_at: SystemTime,
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
        Self {
            history: Arc::default(),
            sampler,
            capacity: Mutex::new(Arc::new(Semaphore::new(MAX_OUTSTANDING))),
            generation: Arc::new(()),
        }
    }

    pub(super) fn reserve(
        &self,
        observe: impl FnOnce() -> usize,
    ) -> (usize, Result<Reservation, LunaSamplerError>) {
        // Keep observation order and admission atomic, including when only one slot remains.
        let capacity = self
            .capacity
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let index = observe();
        let reservation = (|| {
            let permit = Arc::clone(&capacity)
                .try_acquire_owned()
                .map_err(|_| LunaSamplerError::QueueFull)?;
            Ok(Reservation {
                index,
                history: Arc::clone(&self.history),
                sampler: Arc::clone(&self.sampler),
                _permit: permit,
                generation: Arc::downgrade(&self.generation),
            })
        })();
        (index, reservation)
    }
}

impl Reservation {
    pub(super) async fn prepare(
        self,
        mut request: ConversationRequest,
    ) -> Option<PreparedConversation> {
        request.sampling.pending_score = Some(request.pending_score.clone());
        if self.generation.strong_count() == 0
            || !request.authorization.is_current(&request.thread).await
            || request.pending_score.is_expired()
        {
            let _ = request.ready.send(Err(LunaSamplerError::Superseded));
            return None;
        }
        let key = ReuseKey {
            authorization: request.authorization,
            instructions: request.sampling.instructions.clone(),
            parent_compaction: request.sampling.parent_compaction.clone(),
        };
        let previous = self
            .history
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let mut state = previous
            // Delayed preparation must never import a later action's evidence.
            .filter(|previous| previous.index < self.index && previous.key == key)
            .and_then(|previous| previous.state.snapshot().cloned())
            .map(ConversationState::from)
            .unwrap_or_default();
        let had_history = state.snapshot().is_some();
        let review_request = request
            .sampling
            .prepare_retained(
                &request.evidence,
                &mut state,
                self.sampler.max_input_tokens(),
            )
            .filter(|review_request| {
                review_request.existing_context_tokens == 0
                    || review_request.input_tokens <= request.reset_token_limit
            })
            .or_else(|| {
                if !had_history {
                    return None;
                }
                state = ConversationState::default();
                request.sampling.prepare_retained(
                    &request.evidence,
                    &mut state,
                    self.sampler.max_input_tokens(),
                )
            });
        let Some(review_request) = review_request else {
            let _ = request.ready.send(Err(LunaSamplerError::InputTooLarge));
            return None;
        };
        super::metrics::record_section_costs(
            request.metrics.as_deref(),
            review_request.section_costs.iter().copied(),
        );
        super::metrics::record_request_tokens(
            request.metrics.as_deref(),
            review_request.existing_context_tokens,
            review_request.input_tokens,
            super::metrics::RequestAdmission::Admitted,
        );
        drop(request.evidence);
        Some(PreparedConversation {
            reservation: self,
            request: review_request,
            sampling: request.sampling,
            key,
            state,
            thread: request.thread,
            ready: request.ready,
            metrics: request.metrics,
            pending_score: request.pending_score,
            sampled_at: request.sampled_at,
        })
    }
}

impl PreparedConversation {
    /// Publishes the first score, then drains and commits only valid completed history.
    pub(super) async fn run(mut self) {
        // A caller can retain prepared input while authorization or admission expires.
        let authorization_is_current = self.key.authorization.is_current(&self.thread).await;
        let history_superseded = self
            .reservation
            .history
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .is_some_and(|history| history.index > self.reservation.index);
        // Keep requests that can still contribute either a score or retained history.
        if !authorization_is_current
            || self.reservation.generation.strong_count() == 0
            || self.pending_score.is_expired()
            || (history_superseded
                && self
                    .pending_score
                    .progress
                    .inspect(/*call_id*/ None)
                    .sampled_at
                    .is_some_and(|sampled_at| sampled_at > self.sampled_at))
        {
            let _ = self.ready.send(Err(LunaSamplerError::Superseded));
            return;
        }
        let cursor = self.request.cursor;
        let pending_truncations = std::mem::take(&mut self.request.truncations);
        let sample = ConversationSample {
            input: self.request,
            sampling: self.sampling,
        };
        let history = tokio::select! {
            biased;
            () = self.pending_score.expired() => None,
            history = self.reservation.sampler.sample_retained(sample, self.ready) => history,
        };
        if let Some(history) = history
            && self.reservation.generation.strong_count() > 0
            && self.key.authorization.is_current(&self.thread).await
        {
            let mut truncations = super::truncation::ClassificationTruncations::default();
            truncations.extend(pending_truncations);
            truncations.emit(self.metrics.as_deref());
            self.state.complete_review(cursor);
            self.state.commit_snapshot(history);
            let mut latest = self
                .reservation
                .history
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if latest
                .as_ref()
                .is_none_or(|previous| previous.index < self.reservation.index)
            {
                *latest = Some(Arc::new(CompletedConversation {
                    index: self.reservation.index,
                    key: self.key,
                    state: self.state,
                }));
            }
        }
        // The permit is released only after completion and commit, not after the early score.
        drop(self.reservation._permit);
    }
}
