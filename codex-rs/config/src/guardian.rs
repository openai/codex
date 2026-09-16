//! Translates legacy Guardian settings and applies live managed constraints.
//! Catalog policies take precedence over legacy scope and feature settings.

use codex_features::FeatureToml;
use codex_features::GuardianV2ConfigToml;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::openai_models::GuardianModelPolicy;
use codex_protocol::openai_models::GuardianReviewMode;
use codex_protocol::openai_models::GuardianUnscoredAction;
use codex_protocol::openai_models::ModelInfo;

use crate::ConfigRequirements;

/// Load-boundary adapter for configuration that predates model-owned policies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuardianPolicyLoader {
    default: GuardianModelPolicy,
    force_synchronous_review: bool,
    scoring_disabled: bool,
}

impl GuardianPolicyLoader {
    pub fn new(
        legacy: Option<&FeatureToml<GuardianV2ConfigToml>>,
        requirements: &ConfigRequirements,
    ) -> Self {
        let scope = match legacy {
            Some(FeatureToml::Config(config)) => config.review_scope.as_ref(),
            Some(FeatureToml::Enabled(_)) | None => None,
        };
        let computer_use_only = scope
            .and_then(|scope| scope.computer_use_only)
            .unwrap_or(/*default*/ true);
        let other = if computer_use_only {
            GuardianReviewMode::Synchronous
        } else {
            GuardianReviewMode::Adaptive
        };
        let default = GuardianModelPolicy {
            computer_use: Some(GuardianReviewMode::Adaptive),
            shell: Some(other),
            file_changes: Some(other),
            mcp: Some(other),
            network: Some(other),
            permissions: Some(other),
            other_tools: other,
            unscored_action: if computer_use_only {
                GuardianUnscoredAction::Ignore
            } else {
                GuardianUnscoredAction::AgeScore
            },
            initial_cua_call: Some(computer_use_only),
            sandboxed_exec_commands: Some(
                !computer_use_only
                    && scope
                        .and_then(|scope| scope.sandboxed_exec_commands)
                        .unwrap_or(/*default*/ false),
            ),
        };
        let enabled = legacy.and_then(FeatureToml::enabled);
        Self {
            default,
            scoring_disabled: !enabled.unwrap_or(/*default*/ false),
            force_synchronous_review: requirements
                .approvals_reviewer
                .can_set(&ApprovalsReviewer::User)
                .is_err(),
        }
    }

    pub fn resolve(&self, model: Option<&ModelInfo>) -> GuardianModelPolicy {
        let catalog = model.and_then(|model| model.guardian.as_ref());
        let mut policy = catalog.cloned().unwrap_or_else(|| self.default.clone());
        if catalog.is_none()
            && policy.allows_initial_cua_call()
            && model.is_some_and(|model| !model.node_repl_auto_review_required)
        {
            policy.computer_use = Some(GuardianReviewMode::Synchronous);
            policy.initial_cua_call = Some(false);
            policy.unscored_action = GuardianUnscoredAction::InvalidateScore;
        }
        // Preserve the legacy feature gate: explicit catalog policies bypass it.
        if self.force_synchronous_review || self.scoring_disabled && catalog.is_none() {
            policy.disable_scoring();
        }
        policy
    }
}

impl ConfigRequirements {
    /// Applies live administrator requirements to an already resolved model policy.
    pub fn constrain_guardian_policy(&self, policy: &mut GuardianModelPolicy, model: &str) {
        if self.auto_review_required_for_model(model) {
            let computer_use = policy.computer_use;
            let initial_cua_call = policy.allows_initial_cua_call();
            policy.disable_scoring();
            if initial_cua_call {
                policy.computer_use = computer_use;
            }
        }
    }
}

#[cfg(test)]
#[path = "guardian_tests.rs"]
mod tests;
