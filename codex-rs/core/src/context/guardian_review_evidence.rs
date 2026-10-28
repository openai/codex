//! Typed context fragment for bounded, host-supplied synchronous review evidence.
//! Runtime evidence storage and selection belong to the reviewer extension.

use super::ContextualUserFragment;
pub use codex_guardian_reviewer::GuardianReviewEvidence;
pub use codex_guardian_reviewer::GuardianReviewEvidenceRecord;
pub use codex_guardian_reviewer::GuardianUserInputSnapshot;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::input_text;

/// A bounded, host-supplied sync-review record for async classifier input only.
#[derive(Clone, Debug)]
pub struct GuardianReviewEvidenceFragment {
    body: String,
}

impl GuardianReviewEvidenceFragment {
    /// Creates a trusted fragment from classifier-bounded review evidence.
    pub fn new(body: String) -> Self {
        Self { body }
    }
}

impl ContextualUserFragment for GuardianReviewEvidenceFragment {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("guardian.review_evidence".to_string())
    }

    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<guardian_sync_review>", "</guardian_sync_review>")
    }

    fn body(&self) -> ContentItem {
        input_text(self.body.clone())
    }
}
