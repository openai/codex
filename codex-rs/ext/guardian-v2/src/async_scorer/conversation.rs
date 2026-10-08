//! Concurrent classifications fork the latest compatible completed conversation.
//! Admission assigns order and reserves capacity atomically before async preparation.
//! Only newer completions replace history.
//! Permits bound preparation and sampling through full response completion.
//! Backend replacement invalidates its generation without changing snapshot shutdown policy.
//! Oversized retained requests rebuild from fresh evidence before the model's hard limit.
//! Each request owns its fork; failed or cancelled requests leave committed history intact.
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
use tokio::sync::oneshot;

use super::authorization::ScoreAuthorization;
use super::sampler::LunaSampler;
use super::sampler::LunaSamplerError;
use super::sampler::LunaSamplingRequest;
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
    pub(super) async fn run(self, request: ConversationRequest) {
        if self.generation.strong_count() == 0
            || !request.authorization.is_current(&request.thread).await
        {
            let _ = request.ready.send(Err(LunaSamplerError::Superseded));
            return;
        }
        let key = ReuseKey {
            authorization: request.authorization.clone(),
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
            .map(|checkpoint| {
                let (mut state, history) = ConversationState::fork(checkpoint);
                state.commit_snapshot(history);
                state
            })
            .unwrap_or_default();
        let had_history = state.snapshot().is_some();
        let prepared = request
            .sampling
            .prepare_retained(
                &request.evidence,
                &mut state,
                self.sampler.max_input_tokens(),
            )
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
                    self.sampler.max_input_tokens(),
                )
            });
        let Some(mut prepared) = prepared else {
            let _ = request.ready.send(Err(LunaSamplerError::InputTooLarge));
            return;
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
        let history = self.sampler.sample_retained(prepared, request.ready).await;
        if let Some(history) = history
            && self.generation.strong_count() > 0
            && request.authorization.is_current(&request.thread).await
        {
            let mut truncations = super::truncation::ClassificationTruncations::default();
            truncations.extend(pending_truncations);
            truncations.emit(request.metrics.as_deref());
            state.complete_review(cursor);
            state.commit_snapshot(history);
            let mut latest = self
                .history
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if latest
                .as_ref()
                .is_none_or(|previous| previous.index < self.index)
            {
                *latest = Some(Arc::new(CompletedConversation {
                    index: self.index,
                    key,
                    state,
                }));
            }
        }
        // The permit is released only after completion and commit, not after the early score.
        drop(self._permit);
    }
}
