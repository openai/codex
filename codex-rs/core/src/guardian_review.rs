//! Host adapter types and assessment results for synchronous Guardian review.
//! The extension supplies agent startup and owns the reviewer pool and its lifecycle.

pub use crate::guardian::GuardianReviewSession;
pub use crate::guardian::GuardianReviewState;
pub use crate::guardian::PreparedGuardianContext;
pub use crate::guardian::prepare_review_prewarm;
pub use crate::mcp_tool_call::conversation_history::conversation_history_tools;

/// Retrieval guidance for the tools installed in a private verdict session.
#[derive(Clone)]
pub enum ReviewRetrieval {
    ConversationHistory,
    Extension(std::sync::Arc<str>),
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

/// A thread's synchronous reviewer. Bind the evidence catch-up to the action before it waits.
pub trait AttachedReviewer: Send + Sync {
    fn bind(self: std::sync::Arc<Self>) -> BoundReviewer;
}

/// Evidence preparation bound to one action, shared by its review attempts.
#[derive(Clone)]
pub struct BoundReviewer {
    pub preparation: std::sync::Arc<dyn ReviewerPreparation>,
    pub retrieval: ReviewRetrieval,
}

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

/// Reserve the actor's reviewer after routing and delivery have covered the bound action.
pub trait ReviewerPreparation: Send + Sync {
    fn prepare<'a>(
        &'a self,
        setup: &'a PreparedGuardianContext,
        cancel: &'a tokio_util::sync::CancellationToken,
    ) -> codex_extension_api::ExtensionFuture<
        'a,
        Result<Box<dyn PreparedAssessment>, ReviewPreparationError>,
    >;
}

/// Holds the model session until the host finishes validating the original authority.
/// Dropping an interrupted reservation discards its partially admitted conversation.
pub trait PreparedAssessment: Send + Sync {
    fn runtime(&self) -> &GuardianReviewSession;
    /// Whether this reservation opened a reviewer or continued a retained one.
    fn session_kind(&self) -> codex_analytics::GuardianReviewSessionKind;
    fn finish(self: Box<Self>, disposition: codex_guardian_reviewer::SessionDisposition);
}
