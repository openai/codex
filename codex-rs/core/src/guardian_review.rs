//! Host adapter types and assessment results for synchronous Guardian review.
//! The extension supplies agent startup and owns the reviewer pool and its lifecycle.

use crate::context_manager::ContextManager;
pub use crate::guardian::GuardianReviewSession;
pub use crate::guardian::GuardianReviewState;
pub use crate::guardian::PreparedGuardianContext;
pub use crate::guardian::prepare_review_prewarm;
pub use crate::mcp_tool_call::conversation_history::conversation_history_tools;
use std::sync::Mutex;
use std::sync::PoisonError;
use tokio::sync::watch;

/// Tracks root evidence that is visible in memory and still awaiting a history append.
/// Only reviewers that read persisted history install this tracker.
#[derive(Default)]
pub struct PendingReviewWrites {
    pending: Mutex<Vec<watch::Receiver<bool>>>,
}

/// An evidence write that the session must finish. After a failed append, we
/// still preserve later writes, but can no longer confirm a complete history.
pub(crate) struct ReviewWrite {
    receipt: Option<watch::Sender<bool>>,
}

impl ReviewWrite {
    /// Record under the session lock so a reviewer cannot see the new revision
    /// before its pending write is registered.
    pub(crate) fn record<T>(
        writes: Option<&PendingReviewWrites>,
        history: &mut ContextManager,
        record: impl FnOnce(&mut ContextManager) -> T,
    ) -> (T, Option<Self>) {
        let Some(writes) = writes else {
            return (record(history), None);
        };
        let before = history.guardian_revisions();
        let result = record(history);
        let write = (history.guardian_revisions() != before).then(|| writes.begin());
        (result, write)
    }

    pub(crate) fn confirm(self) {
        if let Some(receipt) = self.receipt {
            receipt.send_replace(true);
        }
    }
}

impl PendingReviewWrites {
    fn begin(&self) -> ReviewWrite {
        let mut pending = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
        pending.retain(|write| !*write.borrow());
        // A dropped or failed append leaves a gap for this root's lifetime.
        if pending
            .iter()
            .any(|write| write.has_changed().is_err() && !*write.borrow())
        {
            return ReviewWrite { receipt: None };
        }
        let (appended, write) = watch::channel(/*init*/ false);
        pending.push(write);
        ReviewWrite {
            receipt: Some(appended),
        }
    }

    /// Wait for writes already visible to this review. Later instructions are
    /// covered by the reviewer's authorization recheck and its next attempt.
    pub async fn wait(&self) -> Result<(), ReviewPreparationError> {
        let pending = {
            let mut pending = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
            pending.retain(|write| !*write.borrow());
            pending.clone()
        };
        for mut write in pending {
            write
                .wait_for(|appended| *appended)
                .await
                .map_err(|_| ReviewPreparationError::HistoryWriteUnconfirmed)?;
        }
        Ok(())
    }
}

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

/// A thread's synchronous reviewer. Bind its evidence position before the action waits.
/// Routing may hand the action to a human without ever requesting preparation.
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
    #[error(
        "An instruction may not have been saved. Reload the root thread, check the history and resend any missing instructions before retrying."
    )]
    HistoryWriteUnconfirmed,
    #[error("timed out preparing Guardian review")]
    PreparationTimedOut,
    #[error("reviewer is unavailable")]
    ReviewerUnavailable,
    #[error("evidence delivery failed")]
    DeliveryFailed,
    #[error("preparation was cancelled")]
    Cancelled,
}

/// Reserve the actor's reviewer after routing and delivery have covered this attempt.
/// A retry must cover authorization changes since the previous attempt.
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
