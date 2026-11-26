//! Host adapter types and assessment results for synchronous Guardian review.
//! The extension supplies agent startup and owns the reviewer pool and its lifecycle.

pub use crate::guardian::GuardianReviewSession;
pub use crate::guardian::GuardianReviewState;
pub use crate::guardian::PreparedGuardianContext;
pub use crate::guardian::prepare_review_prewarm;
pub use crate::mcp_tool_call::conversation_history::conversation_history_tools;

/// Opts a root into session-owned evidence saves that survive turn cancellation.
/// Install at thread startup. Reviewers do not wait for these writes.
pub struct PreserveReviewWrites;

/// Retrieval guidance for the tools installed in a private verdict session.
#[derive(Clone)]
pub enum ReviewRetrieval {
    ConversationHistory,
    Extension(std::sync::Arc<str>),
}

/// How the host requests review of a single recorded tool call.
pub enum RecordedReviewMode {
    Transcript {
        root_evidence: Option<codex_app_server_protocol::GuardianRootEvidence>,
    },
    /// Route pending history under the recorded review deadline.
    Attached(codex_extension_api::ExtensionFuture<'static, Result<(), ReviewPreparationError>>),
}

impl RecordedReviewMode {
    pub(crate) fn root_evidence(&self) -> Option<&codex_app_server_protocol::GuardianRootEvidence> {
        match self {
            Self::Transcript { root_evidence } => root_evidence.as_ref(),
            Self::Attached(_) => None,
        }
    }
}

/// The result of checking a recorded call without executing it.
#[derive(Debug, PartialEq, Eq)]
pub struct GuardianReviewResult {
    pub call_id: String,
    pub assessment: codex_guardian_reviewer::GuardianAssessment,
}

/// Scope backend reviewer attribution to the provider-selected Guardian model.
pub fn reviewer_headers(config: &crate::config::Config) -> crate::CodexResponsesHeaders {
    let provider = codex_model_provider::create_model_provider(
        config.model_provider.clone(),
        /*auth_manager*/ None,
    );
    crate::CodexResponsesHeaders {
        model: provider.approval_review_preferred_model().to_owned(),
        headers: http::HeaderMap::from_iter([(
            http::HeaderName::from_static("x-codex-guardian"),
            http::HeaderValue::from_static("reviewer"),
        )]),
    }
}

/// Kept out of the authoritative reviewer slot so routing and prewarming stay unchanged.
pub struct ShadowReviewer {
    pub reviewer: std::sync::Arc<dyn AttachedReviewer>,
    /// Stops this actor when it shuts down or the root leaves Auto Review.
    pub cancellation: tokio_util::sync::CancellationToken,
}

/// Marks the private inference thread, which must never forward items to its parent.
pub struct ShadowReviewSession;

/// Safe to report with a review ID before a verdict has started.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ReviewPreparationError {
    #[error("Guardian board evidence is unavailable")]
    EvidenceUnavailable,
    #[error("timed out preparing Guardian review")]
    PreparationTimedOut,
    #[error("reviewer is unavailable")]
    ReviewerUnavailable,
    #[error("evidence delivery failed")]
    DeliveryFailed,
    #[error("preparation was cancelled")]
    Cancelled,
}

/// A thread's synchronous reviewer. Reserve the conversation and admit the
/// available evidence for each attempt, including retries.
pub trait AttachedReviewer: Send + Sync {
    fn retrieval(&self) -> ReviewRetrieval;

    /// Resolve the worker's currently loaded root. The root actor uses its own reset signal.
    fn root_thread(
        &self,
    ) -> codex_extension_api::ExtensionFuture<'_, Option<std::sync::Arc<crate::CodexThread>>>;

    fn prepare<'a>(
        &'a self,
        setup: &'a PreparedGuardianContext,
        cancel: &'a tokio_util::sync::CancellationToken,
    ) -> codex_extension_api::ExtensionFuture<
        'a,
        Result<Box<dyn PreparedAssessment>, ReviewPreparationError>,
    >;
}

/// Holds the model session until the host accepts or discards its verdict.
/// Dropping an interrupted reservation discards its partially admitted conversation.
pub trait PreparedAssessment: Send + Sync {
    fn runtime(&self) -> &GuardianReviewSession;
    /// Whether this reservation opened a reviewer or continued a retained one.
    fn session_kind(&self) -> codex_analytics::GuardianReviewSessionKind;
    fn finish(self: Box<Self>, disposition: codex_guardian_reviewer::SessionDisposition);
}
