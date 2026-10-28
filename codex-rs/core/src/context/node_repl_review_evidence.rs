//! Chooses REPL evidence capture mode from the issuing turn.
//! The reviewer extension owns the bounded store and immutable snapshots.

use codex_features::Feature;
pub(crate) use codex_guardian_context::NodeReplReviewEvidenceMode;
pub use codex_guardian_reviewer::NodeReplReviewEvidence;

use crate::session::turn_context::TurnContext;

pub(crate) fn node_repl_review_evidence_mode(turn: &TurnContext) -> NodeReplReviewEvidenceMode {
    let features = &turn.config.features;
    if turn.model_info().computer_use_review_required()
        || features.enabled(Feature::GuardianEnhancedNodeReplTranscripts)
            && features.enabled(Feature::GuardianNodeReplTranscriptImages)
    {
        NodeReplReviewEvidenceMode::Multimodal
    } else if features.enabled(Feature::GuardianEnhancedNodeReplTranscripts) {
        NodeReplReviewEvidenceMode::TextOnly
    } else {
        NodeReplReviewEvidenceMode::Disabled
    }
}
