//! Configured inputs retained independently of future thread settings.

use crate::config::Constrained;
use crate::config::ConstraintError;
use crate::config::ConstraintResult;
use codex_config::ConfigRequirements;
use codex_models_manager::ModelsManagerConfig;
use codex_models_manager::manager::ModelsManager;
use codex_otel::SessionTelemetry;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::SERVICE_TIER_DEFAULT_REQUEST_VALUE;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;
use std::sync::Arc;

/// Model and execution settings selected for an individual model step within
/// a turn. A turn may contain several steps, each using its own captured settings.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StepSettings {
    pub(crate) collaboration_mode: CollaborationMode,
    /// `None` follows the pinned model's default.
    pub(crate) reasoning_summary: Option<ReasoningSummary>,
    /// Normalized requested tier. Startup retains initial-model filtering;
    /// resolution filters the effective tier against the pinned model.
    pub(crate) service_tier: Option<String>,
    pub(crate) personality: Option<Personality>,
    pub(crate) approval_policy: Constrained<AskForApproval>,
    pub(crate) approvals_reviewer: ApprovalsReviewer,
}

/// Immutable configured choices, pinned model metadata, and effective request values.
///
/// One snapshot may produce several fresh `StepContext`s. Replacing the
/// snapshot does not change steps or actions that have already captured it.
#[derive(Clone, Debug)]
pub(crate) struct ResolvedStepSettings {
    /// Inputs for later sparse patches. Unset defaults and unsupported requested
    /// tiers must not be reconstructed from the effective values below.
    selected: Arc<StepSettings>,
    pub(crate) model_info: Arc<ModelInfo>,
    /// Effective request summary: the configured value or the pinned model's default.
    pub(crate) reasoning_summary: ReasoningSummary,
    /// Tier sent on requests, filtered by feature and model support.
    pub(crate) service_tier: Option<String>,
    /// MCP historically follows refreshed thread defaults. Preserve that behavior
    /// for older clients until this turn receives an explicit live reviewer update.
    pub(crate) mcp_approvals_reviewer_override: Option<ApprovalsReviewer>,
}

impl ResolvedStepSettings {
    /// Derives request values from selected settings and pinned metadata.
    pub(super) fn new(
        selected: Arc<StepSettings>,
        model_info: Arc<ModelInfo>,
        fast_mode_enabled: bool,
    ) -> Self {
        let reasoning_summary = selected
            .reasoning_summary
            .unwrap_or(model_info.default_reasoning_summary);
        let service_tier = super::get_service_tier(
            selected.service_tier.clone(),
            fast_mode_enabled,
            &model_info,
        );
        Self {
            selected,
            model_info,
            reasoning_summary,
            service_tier,
            mcp_approvals_reviewer_override: None,
        }
    }

    pub(crate) fn reasoning_effort(&self) -> Option<&ReasoningEffort> {
        self.selected
            .collaboration_mode
            .settings
            .reasoning_effort
            .as_ref()
    }

    pub(crate) fn effective_reasoning_effort(&self) -> Option<ReasoningEffort> {
        self.reasoning_effort()
            .or(self.model_info.default_reasoning_level.as_ref())
            .cloned()
    }

    pub(crate) fn effective_collaboration_mode(&self) -> CollaborationMode {
        let mut collaboration_mode = self.selected.collaboration_mode.clone();
        collaboration_mode.settings.model = self.model_info.slug.clone();
        collaboration_mode
    }

    pub(crate) fn approval_policy(&self) -> AskForApproval {
        self.selected.approval_policy.value()
    }

    pub(crate) fn approvals_reviewer(&self) -> ApprovalsReviewer {
        self.selected.approvals_reviewer
    }

    /// Retained inputs for constructing a snapshot against different model metadata.
    pub(super) fn selected(&self) -> &StepSettings {
        &self.selected
    }

    pub(super) fn selected_collaboration_mode(&self) -> &CollaborationMode {
        &self.selected.collaboration_mode
    }

    pub(crate) fn personality(&self) -> Option<Personality> {
        self.selected.personality
    }

    pub(super) fn constrained_approval_policy(&self) -> &Constrained<AskForApproval> {
        &self.selected.approval_policy
    }

    /// Applies sparse edits to the retained selection, preserving pinned metadata
    /// unless model or personality selection changes, then resolves request values.
    pub(super) async fn apply_update(
        &self,
        update: &StepSettingsUpdate,
        constraints: &StepSettingsConstraints<'_>,
        models_manager: &dyn ModelsManager,
        overrides: &ModelInfoOverrides,
        personality_enabled: bool,
        fast_mode_enabled: bool,
    ) -> ConstraintResult<Self> {
        let selected = self.selected.apply(update, constraints)?;
        let model_info = if selected.collaboration_mode.model()
            == self.selected.collaboration_mode.model()
            && selected.personality == self.selected.personality
        {
            Arc::clone(&self.model_info)
        } else {
            Arc::new(
                selected
                    .resolve_model_info(models_manager, overrides, personality_enabled)
                    .await,
            )
        };
        let mut next = Self::new(Arc::new(selected), model_info, fast_mode_enabled);
        next.mcp_approvals_reviewer_override = update
            .approvals_reviewer
            .or(self.mcp_approvals_reviewer_override);
        Ok(next)
    }

    /// Rechecks the retained selection against current managed constraints.
    pub(super) fn revalidate(
        &self,
        constraints: &StepSettingsConstraints<'_>,
    ) -> ConstraintResult<()> {
        // TODO: Revisit the split with StepSettings::validate, which does not
        // check inherited approval values against the supplied managed allow-lists.
        // Sharing these checks would also make ordinary settings updates stricter
        // after a requirements refresh.
        self.selected.validate(constraints)?;
        constraints
            .requirements
            .approval_policy
            .can_set(&self.approval_policy())?;
        constraints
            .requirements
            .approvals_reviewer
            .can_set(&self.approvals_reviewer())
    }

    pub(super) fn telemetry(&self, base: &SessionTelemetry) -> SessionTelemetry {
        base.clone().with_model(
            self.selected.collaboration_mode.model(),
            &self.model_info.slug,
        )
    }
}

/// Explicit startup overrides applied to catalog-derived model metadata.
/// Construct from `Config::to_models_manager_config()` so model-derived base
/// instructions are not mistaken for explicit overrides.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ModelInfoOverrides {
    pub(crate) context_window: Option<i64>,
    pub(crate) auto_compact_token_limit: Option<i64>,
    pub(crate) tool_output_token_limit: Option<usize>,
    pub(crate) base_instructions: Option<String>,
}

impl From<ModelsManagerConfig> for ModelInfoOverrides {
    fn from(config: ModelsManagerConfig) -> Self {
        Self {
            context_window: config.model_context_window,
            auto_compact_token_limit: config.model_auto_compact_token_limit,
            tool_output_token_limit: config.tool_output_token_limit,
            base_instructions: config.base_instructions,
        }
    }
}

impl ModelInfoOverrides {
    pub(crate) fn models_manager_config(
        &self,
        personality: Option<Personality>,
        personality_enabled: bool,
    ) -> ModelsManagerConfig {
        ModelsManagerConfig {
            model_context_window: self.context_window,
            model_auto_compact_token_limit: self.auto_compact_token_limit,
            tool_output_token_limit: self.tool_output_token_limit,
            base_instructions: self.base_instructions.clone(),
            personality,
            personality_enabled,
            // The models manager already owns its catalog.
            model_catalog: None,
        }
    }
}

/// Sparse edits applied independently to each settings owner.
///
/// Do not materialize a partial update against one settings owner and reuse
/// that full value for another. Merge the requested edits with each target.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct StepSettingsUpdate {
    pub(crate) model: Option<String>,
    pub(crate) effort: Option<Option<ReasoningEffort>>,
    /// A complete collaboration mode takes precedence over model and effort edits.
    pub(crate) collaboration_mode: Option<CollaborationMode>,
    pub(crate) reasoning_summary: Option<ReasoningSummary>,
    pub(crate) service_tier: Option<Option<String>>,
    pub(crate) personality: Option<Personality>,
    pub(crate) approval_policy: Option<AskForApproval>,
    pub(crate) approvals_reviewer: Option<ApprovalsReviewer>,
}

/// Constraints used when applying and validating a candidate settings version.
/// Future settings use the proposed environment; active settings use the
/// environment already admitted for that execution.
pub(crate) struct StepSettingsConstraints<'a> {
    pub(crate) requirements: &'a ConfigRequirements,
    pub(crate) guardian_approval_enabled: bool,
    pub(crate) trusted_guardian_reviewer: bool,
    pub(crate) has_full_disk_write_access: bool,
}

impl StepSettings {
    /// Resolves the selected model using the session's explicit startup overrides.
    pub(super) async fn resolve_model_info(
        &self,
        models_manager: &dyn ModelsManager,
        overrides: &ModelInfoOverrides,
        personality_enabled: bool,
    ) -> ModelInfo {
        let config = overrides.models_manager_config(self.personality, personality_enabled);
        models_manager
            .get_model_info(self.collaboration_mode.model(), &config)
            .await
    }

    /// Applies edits and validates the result against the supplied constraints.
    /// Callers must supply the constraints of the proposed target environment.
    pub(crate) fn apply(
        &self,
        update: &StepSettingsUpdate,
        constraints: &StepSettingsConstraints<'_>,
    ) -> ConstraintResult<Self> {
        let mut next = self.clone();
        next.collaboration_mode = update.collaboration_mode.clone().unwrap_or_else(|| {
            self.collaboration_mode.with_updates(
                update.model.clone(),
                update.effort.clone(),
                /*developer_instructions*/ None,
            )
        });
        if let Some(summary) = update.reasoning_summary {
            next.reasoning_summary = Some(summary);
        }
        if let Some(service_tier) = update.service_tier.clone() {
            // TODO(aibrahim): Remove once v2 clients no longer send the legacy
            // "fast" service tier value.
            next.service_tier = Some(match service_tier {
                Some(service_tier) => ServiceTier::from_request_value(&service_tier)
                    .map_or(service_tier, |service_tier| {
                        service_tier.request_value().to_string()
                    }),
                None => SERVICE_TIER_DEFAULT_REQUEST_VALUE.to_string(),
            });
        }
        if let Some(personality) = update.personality {
            next.personality = Some(personality);
        }
        if let Some(approval_policy) = update.approval_policy {
            next.approval_policy.set(approval_policy)?;
        }
        if let Some(approvals_reviewer) = update.approvals_reviewer {
            constraints
                .requirements
                .approvals_reviewer
                .can_set(&approvals_reviewer)?;
            next.approvals_reviewer = approvals_reviewer;
        }
        if !constraints.trusted_guardian_reviewer
            && self.collaboration_mode.model() != next.collaboration_mode.model()
            && constraints
                .requirements
                .auto_review_required_for_model(next.collaboration_mode.model())
            && update.approvals_reviewer.is_none()
        {
            constraints
                .requirements
                .approvals_reviewer
                .can_set(&ApprovalsReviewer::AutoReview)?;
            next.approvals_reviewer = ApprovalsReviewer::AutoReview;
        }
        next.validate(constraints)?;
        Ok(next)
    }

    /// Checks directly constructed settings or rechecks changed constraints.
    pub(super) fn validate(
        &self,
        constraints: &StepSettingsConstraints<'_>,
    ) -> ConstraintResult<()> {
        if constraints.trusted_guardian_reviewer {
            return Ok(());
        }

        let model = self.collaboration_mode.model();
        if !constraints
            .requirements
            .auto_review_required_for_model(model)
        {
            return Ok(());
        }

        if self.approvals_reviewer == ApprovalsReviewer::AutoReview
            && !constraints.has_full_disk_write_access
            && constraints.guardian_approval_enabled
        {
            return Ok(());
        }

        Err(ConstraintError::AutoReviewRequired {
            model: model.to_string(),
        })
    }
}

#[cfg(test)]
#[path = "step_settings_tests.rs"]
pub(super) mod tests;
