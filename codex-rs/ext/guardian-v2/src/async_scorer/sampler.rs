//! Builds tool-less risk requests and publishes the first classifier output.
//! Both transports share request identity, retry, cancellation, and output handling.

mod connection_pool;
mod execution;

use super::request::PreparedRequest;
use connection_pool::ConnectionPool;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use codex_api::ApiError;
use codex_api::Reasoning;
use codex_api::ReasoningContext;
use codex_api::ResponsesApiRequest;
use codex_context_fragments::RenderedFragment;
use codex_extension_api::ExtensionMetrics;
use codex_history::ResponseItemEnvelope;
use codex_http_client::HttpClientFactory;
use codex_login::AgentIdentityAuthPolicy;
use codex_model_provider::SharedModelProvider;
use codex_model_provider::WorkspaceRoutingContext;
use codex_protocol::ResponseItemId;
use codex_protocol::error::CodexErr;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::SessionSource;
use thiserror::Error;
use tokio::sync::oneshot;
use uuid::Uuid;

pub(crate) const MODEL: &str = "gpt-5.6-luna";
pub(crate) const CLASSIFICATION_TOKEN_USAGE_METRIC: &str =
    "codex.guardian_v2.classification.token_usage";
const MAX_OUTPUT_BYTES: usize = 8 * 1024;
pub(super) const INITIAL_WEBSOCKET_CONNECTIONS: usize = if cfg!(test) { 2 } else { 8 };
const MAX_CONCURRENT_REQUESTS: usize = 16;

/// Host-owned provider, authentication, and attribution for one Luna connection.
pub struct LunaSamplerConfig {
    /// Provider and credentials selected for the owning thread.
    pub provider: SharedModelProvider,
    /// Routing scope and retained configuration layers for the owning thread.
    pub workspace_routing: WorkspaceRoutingContext,
    /// Effective proxy, custom-CA, and cookie configuration.
    pub http_client_factory: HttpClientFactory,
    /// Agent-identity policy selected for the owning thread.
    pub agent_identity_policy: AgentIdentityAuthPolicy,
    /// Host-resolved source used to scope agent-identity authentication.
    pub session_source: SessionSource,
    /// Owning runtime session identifier.
    pub session_id: String,
    /// Owning thread identifier.
    pub thread_id: String,
    /// Optional host-resolved request originator.
    pub originator: Option<String>,
    /// Optional inference service tier.
    pub service_tier: Option<String>,
    /// Luna model's host-resolved encrypted-compaction compatibility hash.
    pub luna_compaction_hash: Option<String>,
    /// Complete input allowance resolved for the classifier model.
    pub max_input_tokens: usize,
    /// Host-provided metrics capability with the owning session's attribution.
    pub metrics: Option<Arc<dyn ExtensionMetrics>>,
}

/// One tool-less Luna classification request.
pub struct LunaSamplingRequest {
    /// ID of the response handling the classified tool.
    pub parent_response_id: Option<String>,
    /// Trusted classifier instructions with their role and content attribution.
    pub instructions: RenderedFragment,
    /// Composed evidence messages, with roles, annotations and content order intact.
    pub input: Vec<ResponseItem>,
    /// Opaque parent compaction to reuse only for compatible model configurations.
    pub parent_compaction: Option<ResponseItem>,
    /// Host-selected compatibility hash for the supplied parent checkpoint.
    pub parent_compaction_hash: Option<String>,
    /// Reasoning budget explicitly selected for this request.
    pub reasoning_effort: ReasoningEffort,
    /// Owning turn that initiated this classification, not the classifier turn.
    pub parent_turn_id: String,
    /// Trusted causal root of the owning turn, absent when unknown or ambiguous.
    pub root_turn_id: Option<String>,
}

/// Failures returned while connecting or sampling the Luna model.
#[derive(Debug, Error)]
pub enum LunaSamplerError {
    /// The thread's provider or scoped credentials could not be resolved.
    #[error("could not resolve the Luna model provider: {0}")]
    Provider(#[source] CodexErr),
    /// The Responses request could not be opened or streamed.
    #[error("Luna Responses request failed: {0}")]
    Api(#[source] ApiError),
    /// The provider's WebSocket connect deadline elapsed.
    #[error("Luna Responses WebSocket connection timed out")]
    ConnectionTimeout,
    /// The response did not contain an assistant text value.
    #[error("Luna response did not contain assistant output")]
    MissingOutput,
    /// The response exceeded the bounded output limit.
    #[error("Luna response exceeded the output limit")]
    OutputTooLarge,
    /// A newer classification replaced this request when the pool was full.
    #[error("Luna request was superseded by a newer classification")]
    Superseded,
    /// The supplied parent checkpoint cannot be consumed by this Luna configuration.
    #[error("parent compaction is incompatible with Luna")]
    IncompatibleCompaction,
    /// The complete classifier input exceeded the model allowance.
    #[error("Luna input exceeds the complete request budget")]
    InputTooLarge,
    /// The retained conversation already owns its maximum outstanding work.
    #[error("Guardian conversation queue is full")]
    QueueFull,
}

struct ActiveRequest {
    supersede: oneshot::Sender<()>,
    scored: Arc<AtomicBool>,
}

/// Runs bounded Luna classifications over pooled WebSockets or HTTP.
pub struct LunaSampler {
    config: Arc<LunaSamplerConfig>,
    connections: Arc<ConnectionPool>,
    active_requests: Mutex<VecDeque<ActiveRequest>>,
}

impl LunaSampler {
    /// A checkpoint is reusable only when both models declare the same nonempty hash.
    pub(super) fn supports_parent_compaction(&self, parent_hash: Option<&str>) -> bool {
        parent_hash
            .zip(self.config.luna_compaction_hash.as_deref())
            .is_some_and(|(parent_hash, luna_hash)| {
                !parent_hash.is_empty() && parent_hash == luna_hash
            })
    }

    pub(super) fn new(config: LunaSamplerConfig) -> Self {
        let config = Arc::new(config);
        Self {
            connections: ConnectionPool::new(Arc::clone(&config)),
            config,
            active_requests: Mutex::new(VecDeque::with_capacity(MAX_CONCURRENT_REQUESTS)),
        }
    }

    pub(super) async fn prewarm(&self) {
        if let Some(refill) = self.connections.replenish() {
            let _ = refill.await;
        }
    }

    fn auth_owner_generation(&self) -> Option<u64> {
        self.config
            .provider
            .auth_manager()
            .filter(|_| self.config.provider.info().auth.is_none())
            .map(|manager| {
                manager
                    .auth_change_state_receiver()
                    .borrow()
                    .owner_generation
            })
    }

    // Normalize new evidence while preserving the retained prefix and its item IDs.
    pub(super) fn prepare_input(
        request: &LunaSamplingRequest,
        history: Option<Vec<ResponseItemEnvelope>>,
        mut evidence: Vec<ResponseItemEnvelope>,
    ) -> (Vec<ResponseItemEnvelope>, usize) {
        let mut input = history.unwrap_or_else(|| {
            let mut input = vec![
                ResponseItemEnvelope::new(ResponseItem::AdditionalTools {
                    id: None,
                    role: "developer".to_owned(),
                    tools: Vec::new(),
                }),
                ResponseItemEnvelope::new(ResponseItem::from(request.instructions.clone())),
            ];
            input.extend(
                request
                    .parent_compaction
                    .clone()
                    .map(ResponseItemEnvelope::new),
            );
            input
        });
        // Normalize only new evidence; never rewrite the committed prefix.
        for item in &mut evidence {
            if let ResponseItem::Message { content, .. } = &mut item.item {
                for content in content {
                    if let ContentItem::InputImage { detail, .. } = content {
                        *detail = None;
                    }
                }
            }
        }
        input.extend(evidence);
        // Assign IDs once so retries reuse the same input item identities.
        for item in &mut input {
            if item.id().is_none()
                && let Some(prefix) = item.id_prefix()
            {
                item.set_id(Some(ResponseItemId::new(prefix)));
            }
        }
        let tokens = input
            .iter()
            .map(|item| codex_guardian_context::estimate_input_tokens(&item.item))
            .fold(0usize, usize::saturating_add);
        (input, tokens)
    }

    fn execution(
        &self,
        request: &LunaSamplingRequest,
        input: Vec<ResponseItem>,
        auth_owner_generation: Option<u64>,
    ) -> execution::SamplingExecution {
        let api_request = ResponsesApiRequest {
            model: MODEL.to_owned(),
            input,
            tools: None,
            tool_choice: "none".to_owned(),
            parallel_tool_calls: false,
            reasoning: Some(Reasoning {
                effort: Some(request.reasoning_effort.clone()),
                summary: None,
                context: Some(ReasoningContext::AllTurns),
            }),
            store: false,
            stream: true,
            stream_options: None,
            include: Vec::new(),
            service_tier: None,
            prompt_cache_key: Some(format!("guardian-v2:{}", self.config.thread_id)),
            text: None,
            client_metadata: None,
            access_programs: None,
        };
        execution::SamplingExecution {
            auth_owner_generation,
            config: Arc::clone(&self.config),
            connections: Arc::clone(&self.connections),
            request: api_request,
            // Retries keep the classification's inference turn identity.
            turn_id: Uuid::now_v7().to_string(),
            parent_response_id: request.parent_response_id.clone(),
            parent_turn_id: request.parent_turn_id.clone(),
            root_turn_id: request.root_turn_id.clone(),
        }
    }

    /// Sends one tool-less classification request using an available transport.
    pub async fn sample(
        &self,
        mut request: LunaSamplingRequest,
    ) -> Result<String, LunaSamplerError> {
        let auth_owner_generation = self.auth_owner_generation();
        if request.parent_compaction.is_some()
            && !self.supports_parent_compaction(request.parent_compaction_hash.as_deref())
        {
            return Err(LunaSamplerError::IncompatibleCompaction);
        }
        let evidence = std::mem::take(&mut request.input)
            .into_iter()
            .map(ResponseItemEnvelope::new)
            .collect();
        let (input, total_tokens) = Self::prepare_input(&request, /*history*/ None, evidence);
        let input = input
            .into_iter()
            .map(ResponseItemEnvelope::into_item)
            .collect();
        super::metrics::record_request_tokens(
            self.config.metrics.as_deref(),
            /*existing*/ 0,
            total_tokens,
        );
        // Oversized classifications defer to sync with the existing failure score.
        if total_tokens > self.config.max_input_tokens.saturating_sub(/*rhs*/ 256) {
            return Err(LunaSamplerError::InputTooLarge);
        }
        let (supersede, superseded) = oneshot::channel();
        let scored = Arc::new(AtomicBool::new(/*v*/ false));
        {
            let mut active_requests = self
                .active_requests
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            active_requests.retain(|request| !request.supersede.is_closed());
            if active_requests.len() == MAX_CONCURRENT_REQUESTS {
                let oldest_scored = active_requests
                    .iter()
                    .position(|request| request.scored.load(Ordering::Relaxed))
                    .unwrap_or(0);
                if let Some(oldest) = active_requests.remove(oldest_scored) {
                    let _ = oldest.supersede.send(());
                }
            }
            active_requests.push_back(ActiveRequest {
                supersede,
                scored: Arc::clone(&scored),
            });
        }
        self.execution(&request, input, auth_owner_generation)
            .run(execution::SamplingMode::Snapshot { superseded, scored })
            .await
    }

    pub(super) fn max_input_tokens(&self) -> usize {
        self.config.max_input_tokens
    }

    /// Publishes the early score and returns only validated, completed history.
    pub(super) async fn sample_retained(
        &self,
        prepared: PreparedRequest<'_>,
        ready: oneshot::Sender<Result<String, LunaSamplerError>>,
    ) -> Option<Vec<ResponseItemEnvelope>> {
        let auth_owner_generation = self.auth_owner_generation();
        let mut completion = execution::RetainedCompletion {
            ready: Some(ready),
            output: Some(Vec::new()),
            early_score: None,
            remaining_tokens: self
                .config
                .max_input_tokens
                .saturating_sub(/*rhs*/ 256)
                .saturating_sub(prepared.input_tokens),
        };
        // Transport sees model items; delivery proof follows the unchanged input prefix.
        let (input, metadata): (Vec<_>, Vec<_>) = prepared
            .input
            .into_iter()
            .map(|envelope| (envelope.item, envelope.metadata))
            .unzip();
        let mut execution = self.execution(prepared.sampling, input, auth_owner_generation);
        execution.request.include = vec!["reasoning.encrypted_content".to_owned()];
        let result = execution
            .run(execution::SamplingMode::Retained(&mut completion))
            .await;
        execution.request.input.extend(completion.finish(result)?);
        Some(
            execution
                .request
                .input
                .into_iter()
                .zip(metadata.into_iter().chain(std::iter::repeat(None)))
                .map(|(item, metadata)| ResponseItemEnvelope { item, metadata })
                .collect(),
        )
    }
}

#[cfg(test)]
#[path = "sampler_tests.rs"]
pub(super) mod tests;
