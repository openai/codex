//! Resolves Guardian requirements and retained configuration fallbacks identically
//! during startup and runtime publication.

use super::guardian_policy_config_from_requirements;
use super::normalize_guardian_policy_config;
use codex_config::ConfigRequirements;
use codex_config::ConfigRequirementsToml;
use codex_config::config_toml::AutoReviewToml;
use codex_protocol::config_types::ApprovalsReviewer;

pub(crate) fn resolve_guardian_policy_text(
    requirements: &ConfigRequirementsToml,
    fallback: Option<&AutoReviewToml>,
) -> (Option<String>, Option<String>) {
    let policy = guardian_policy_config_from_requirements(requirements).or_else(|| {
        normalize_guardian_policy_config(fallback.and_then(|fallback| fallback.policy.as_deref()))
    });
    let extra = normalize_guardian_policy_config(requirements.guardian_extra_policy.as_deref())
        .or_else(|| {
            normalize_guardian_policy_config(
                fallback.and_then(|fallback| fallback.extra_policy.as_deref()),
            )
        });
    (policy, extra)
}

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
