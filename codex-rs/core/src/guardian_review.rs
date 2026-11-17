//! Host adapter types and assessment results for synchronous Guardian review.
//! The extension supplies agent startup and owns the reviewer pool and its lifecycle.

pub use crate::guardian::GuardianReviewSession;
pub use crate::guardian::GuardianReviewState;
pub use crate::guardian::PreparedGuardianContext;
pub use crate::guardian::prepare_review_prewarm;
pub use crate::mcp_tool_call::conversation_history::conversation_history_tools;

/// Retrieval guidance for the tools installed in a private verdict session.
#[derive(Clone, Copy)]
pub enum ReviewRetrieval {
    ConversationHistory,
    Extension(&'static str),
}

/// The result of checking a recorded call without executing it.
#[derive(Debug, PartialEq, Eq)]
pub struct GuardianReviewResult {
    pub call_id: String,
    /// Absent when current policy or a remembered approval skips review.
    pub assessment: Option<codex_guardian_reviewer::GuardianAssessment>,
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
