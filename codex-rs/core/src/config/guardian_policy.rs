//! Resolves the effective Guardian reviewer while retaining the requested choice.

use codex_config::ConfigRequirements;
use codex_protocol::config_types::ApprovalsReviewer;

/// Resolves the effective reviewer without replacing the retained request.
/// Trusted private reviewers omit the model to bypass model-required review.
pub(crate) fn resolve_guardian_reviewer(
    requested: ApprovalsReviewer,
    model: Option<&str>,
    requirements: &ConfigRequirements,
) -> super::ConstraintResult<ApprovalsReviewer> {
    let reviewer = if model.is_some_and(|model| requirements.auto_review_required_for_model(model))
    {
        ApprovalsReviewer::AutoReview
    } else if requirements.approvals_reviewer.can_set(&requested).is_ok() {
        requested
    } else {
        requirements.approvals_reviewer.value()
    };
    requirements.approvals_reviewer.can_set(&reviewer)?;
    Ok(reviewer)
}
