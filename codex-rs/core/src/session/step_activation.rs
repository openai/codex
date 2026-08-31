//! Restricted updates to a running turn's immutable settings snapshots.

use super::session::Session;
use super::session::SessionConfiguration;
use super::step_settings::ResolvedStepSettings;
use super::step_settings::StepSettingsConstraints;
use super::step_settings::StepSettingsUpdate;
use super::turn_context::TurnContext;
use crate::config::Config;
use crate::config::ConstraintResult;
use crate::context::GuardianNodeReplPolicy;
use crate::exec_policy::AllowPrefixRules;
use crate::guardian::BUNDLED_GUARDIAN_POLICY_TEMPLATE;
use codex_features::Feature;
use codex_protocol::openai_models::GuardianV2ModelConfig;
use codex_protocol::openai_models::GuardianV2TranscriptModelConfig;
use codex_protocol::openai_models::MODEL_SPECIALTY_CYBER;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::protocol::TurnSettingsUpdate;
use codex_protocol::protocol::TurnSettingsUpdateOutcome;
use std::sync::Arc;

/// Temporary restrictions while approvals and Guardian still read the admitted
/// `TurnContext`. Ordinary live authorization is validated separately. Remove
/// these restrictions as their consumers migrate to captured step settings.
fn check_legacy_turn_safety(
    turn_context: &TurnContext,
    current: &ResolvedStepSettings,
    destination: &ResolvedStepSettings,
    live_config: &Config,
) -> Result<(), String> {
    let stack = &live_config.config_layer_stack;
    let requirements = stack.requirements();
    let required_review = requirements
        .auto_review_required_for_model(destination.selected_collaboration_mode().model());
    let admitted_required_review = turn_context
        .config
        .config_layer_stack
        .requirements()
        .auto_review_required_for_model(&turn_context.model_info().slug);
    let ignored_models = stack
        .requirements_toml()
        .auto_review
        .as_ref()
        .and_then(|review| review.ignore_rules.as_ref());
    let ignores_prefix_rules = |model: &ModelInfo| {
        model.model_specialty.as_deref() == Some(MODEL_SPECIALTY_CYBER)
            || ignored_models.is_some_and(|models| models.contains(&model.slug))
    };

    // tools::approvals and guardian::review still route approvals using the
    // originating turn's policy, reviewer, and required-model classification.
    if destination.constrained_approval_policy() != current.constrained_approval_policy()
        || destination.approval_policy() != turn_context.approval_policy()
    {
        return Err("the destination changes the admitted approval policy".to_string());
    }
    if destination.approvals_reviewer() != current.approvals_reviewer()
        || destination.approvals_reviewer() != turn_context.config.approvals_reviewer
    {
        return Err("the destination changes the admitted approvals reviewer".to_string());
    }
    if required_review
        != requirements
            .auto_review_required_for_model(current.selected_collaboration_mode().model())
        || required_review != admitted_required_review
    {
        return Err("the destination changes model-required approval authority".to_string());
    }
    // Command approval continues to use TurnContext::allow_prefix_rules.
    if ignores_prefix_rules(&destination.model_info) != ignores_prefix_rules(&current.model_info)
        || ignores_prefix_rules(&destination.model_info)
            != (turn_context.allow_prefix_rules() == AllowPrefixRules::IgnoreForCyberModel)
    {
        return Err("the destination changes the admitted prefix-rule policy".to_string());
    }

    check_legacy_model_safety(
        turn_context.model_info(),
        &current.model_info,
        &destination.model_info,
        &turn_context.config,
        live_config,
    )
}

/// Model-owned portion of the temporary legacy-turn safety check. Ordinary
/// model metadata may differ so diagnostics can expose unmigrated consumers.
fn check_legacy_model_safety(
    admitted: &ModelInfo,
    current: &ModelInfo,
    destination: &ModelInfo,
    admitted_config: &Config,
    live_config: &Config,
) -> Result<(), String> {
    if admitted.used_fallback_model_metadata || current.used_fallback_model_metadata {
        return Err("the active model has only fallback metadata".to_string());
    }
    if destination.used_fallback_model_metadata {
        return Err("the destination model has only fallback metadata".to_string());
    }
    let retained_models = [admitted, current];
    // guardian::review::record_guardian_denial still selects its circuit-breaker
    // policy from the admitted model's Cyber classification.
    let destination_is_cyber =
        destination.model_specialty.as_deref() == Some(MODEL_SPECIALTY_CYBER);
    if retained_models.iter().any(|model| {
        (model.model_specialty.as_deref() == Some(MODEL_SPECIALTY_CYBER)) != destination_is_cyber
    }) {
        return Err("the destination changes the admitted Guardian rejection policy".to_string());
    }
    // TurnMetadataState pins both node REPL flags. Guardian prompt/evidence
    // construction also reads node_repl_auto_review_required from the turn.
    if retained_models.iter().any(|model| {
        model.node_repl_auto_review_required != destination.node_repl_auto_review_required
    }) {
        return Err(
            "the destination changes the admitted node REPL review requirement".to_string(),
        );
    }
    if retained_models
        .iter()
        .any(|model| model.node_repl_disabled != destination.node_repl_disabled)
    {
        return Err(
            "the destination changes the admitted node REPL availability restriction".to_string(),
        );
    }
    // guardian::review::guardian_review_session_config and Guardian V2 still
    // select the reviewer from the retained parent metadata.
    if retained_models
        .iter()
        .any(|model| model.auto_review_model_override != destination.auto_review_model_override)
    {
        return Err("the destination changes the explicit Guardian reviewer model".to_string());
    }

    if admitted_config.features.enabled(Feature::GuardianV2)
        && admitted_config.features.enabled(Feature::GuardianApproval)
    {
        // GuardianV2Extension::on_tool_start reads the parent ModelInfo from
        // thread_store. Its classifier settings are independent of the reviewer
        // override. Local overrides may mask differences, but resolving those
        // overrides remains the extension's responsibility.
        let classification_settings =
            |model: &ModelInfo| -> GuardianV2ModelConfig {
                let mut settings = model
                    .model_messages
                    .as_ref()
                    .and_then(|messages| messages.guardian_v2.clone())
                    .unwrap_or_default();
                // Missing and empty transcript records supply the same defaults.
                if settings.transcript.as_ref().is_some_and(|transcript| {
                    *transcript == GuardianV2TranscriptModelConfig::default()
                }) {
                    settings.transcript = None;
                }
                settings
            };
        let destination_settings = classification_settings(destination);
        if retained_models
            .iter()
            .any(|model| classification_settings(model) != destination_settings)
        {
            return Err(
                "the destination changes the admitted Guardian V2 classification settings"
                    .to_string(),
            );
        }
    }

    // guardian_review_session_config and Guardian V2 can fall back to parent
    // metadata if their preferred reviewer is unavailable, including after a
    // catalog refresh. V1 uses the admitted config; V2 can use the live config.
    // An unchanged explicit reviewer override prevents both fallback paths.
    if destination.auto_review_model_override.is_none() {
        let destination_node_repl_policy =
            GuardianNodeReplPolicy::from_model_messages(destination.model_messages.as_ref());
        for model in retained_models {
            let policy = GuardianNodeReplPolicy::from_model_messages(model.model_messages.as_ref());
            if policy != destination_node_repl_policy {
                return Err(
                    "the destination changes the Guardian parent-fallback node REPL policy"
                        .to_string(),
                );
            }
        }
        for config in [admitted_config, live_config] {
            let destination_policy =
                config.resolve_guardian_policy(destination.model_messages.as_ref());
            if retained_models.iter().any(|model| {
                config.resolve_guardian_policy(model.model_messages.as_ref()) != destination_policy
            }) {
                return Err(
                    "the destination changes the Guardian parent-fallback policy".to_string(),
                );
            }
        }
        let destination_template = destination
            .model_messages
            .as_ref()
            .and_then(|messages| messages.auto_review.as_ref())
            .and_then(|messages| messages.policy_template.as_deref())
            .unwrap_or(BUNDLED_GUARDIAN_POLICY_TEMPLATE)
            .trim_end();
        if retained_models.iter().any(|model| {
            model
                .model_messages
                .as_ref()
                .and_then(|messages| messages.auto_review.as_ref())
                .and_then(|messages| messages.policy_template.as_deref())
                .unwrap_or(BUNDLED_GUARDIAN_POLICY_TEMPLATE)
                .trim_end()
                != destination_template
        }) {
            return Err(
                "the destination changes the Guardian parent-fallback policy template".to_string(),
            );
        }
    }
    Ok(())
}

impl Session {
    /// Publishes settings to the named, originally captured live task, regardless
    /// of task kind. Publication does not propagate to child sessions or require
    /// the task to sample; consumers using initial settings remain unchanged.
    ///
    /// Callers must serialize updates through completion, including model
    /// resolution, so each sparse patch sees the preceding publication.
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "the final managed-policy check and active settings publication must remain atomic"
    )]
    pub(super) async fn apply_turn_settings(
        &self,
        turn_id: &str,
        update: TurnSettingsUpdate,
    ) -> TurnSettingsUpdateOutcome {
        if !self.features.enabled(Feature::StepModelSwitching) {
            return TurnSettingsUpdateOutcome::Rejected {
                reason: "turn settings updates require the step_model_switching feature"
                    .to_string(),
            };
        }

        // Capture the exact live task and its settings, then release the
        // lock. A task that starts during preparation is never a new target.
        let target = {
            let active = self.active_turn.lock().await;
            active.as_ref().and_then(|active| {
                active.task.as_ref().and_then(|task| {
                    (task.turn_context.sub_id == turn_id && !task.cancellation_token.is_cancelled())
                        .then(|| {
                            (
                                Arc::clone(&task.turn_context),
                                Arc::clone(&task.done),
                                task.turn_context.current_settings.load_full(),
                            )
                        })
                })
            })
        };
        let Some((turn_context, task_done, current)) = target else {
            return TurnSettingsUpdateOutcome::TargetUnavailable;
        };
        let TurnSettingsUpdate {
            model,
            effort,
            summary,
            service_tier,
        } = update;
        let update = StepSettingsUpdate {
            model,
            effort,
            reasoning_summary: summary,
            service_tier,
            ..Default::default()
        };
        // Apply the sparse patch to the captured active base using the shared
        // settings rules. The task can progress, finish, or be cancelled while
        // preparation awaits; no publication locks are held here.
        let prepared = self
            .prepare_step_settings_activation(&turn_context, &current, &update)
            .await;
        let active = self.active_turn.lock().await;
        let Some(task) = active.as_ref().and_then(|active| active.task.as_ref()) else {
            return TurnSettingsUpdateOutcome::TargetUnavailable;
        };
        // A later task may reuse the same context and turn ID. `done` is
        // allocated per task, so matching only the ID/context is insufficient.
        // A mismatch abandons the update without retrying or retargeting.
        if !Arc::ptr_eq(&task.done, &task_done)
            || !Arc::ptr_eq(&task.turn_context, &turn_context)
            || !Arc::ptr_eq(&task.turn_context.current_settings.load_full(), &current)
            || task.cancellation_token.is_cancelled()
        {
            return TurnSettingsUpdateOutcome::TargetUnavailable;
        }
        let destination = match prepared {
            Ok(destination) => destination,
            Err(reason) => return TurnSettingsUpdateOutcome::Rejected { reason },
        };
        // Managed requirements can change during resolution. Keep the live
        // authorization and safety checks atomic with publication under state
        // and active_turn; no asynchronous preparation runs under these locks.
        let state = self.state.lock().await;
        if let Err(reason) = self
            .validate_active_step_settings(
                &turn_context,
                &destination,
                &state.session_configuration,
            )
            .map_err(|error| error.to_string())
            .and_then(|()| {
                check_legacy_turn_safety(
                    &turn_context,
                    &current,
                    &destination,
                    &state.session_configuration.original_config_do_not_use,
                )
            })
        {
            return TurnSettingsUpdateOutcome::Rejected { reason };
        }
        // Publish the immutable snapshot. Frozen initial settings, existing step
        // captures, and future thread settings are not changed.
        task.turn_context
            .current_settings
            .store(Arc::new(destination));
        TurnSettingsUpdateOutcome::Applied
    }

    async fn prepare_step_settings_activation(
        &self,
        turn_context: &TurnContext,
        current: &ResolvedStepSettings,
        update: &StepSettingsUpdate,
    ) -> Result<ResolvedStepSettings, String> {
        let (requirements, overrides, trusted_guardian_reviewer) = {
            let state = self.state.lock().await;
            let configuration = &state.session_configuration;
            let stack = &configuration.original_config_do_not_use.config_layer_stack;
            (
                stack.requirements().clone(),
                configuration.model_info_overrides.clone(),
                configuration.trusted_guardian_reviewer,
            )
        };
        let constraints = StepSettingsConstraints {
            requirements: &requirements,
            guardian_approval_enabled: self.features.enabled(Feature::GuardianApproval),
            trusted_guardian_reviewer,
            has_full_disk_write_access: turn_context
                .file_system_sandbox_policy()
                .has_full_disk_write_access(),
        };
        current
            .apply_update(
                update,
                &constraints,
                self.services.models_manager.as_ref(),
                &overrides,
                self.features.enabled(Feature::Personality),
                self.features.enabled(Feature::FastMode),
            )
            .await
            .map_err(|error| error.to_string())
    }

    /// Rechecks ordinary managed authorization after asynchronous resolution.
    /// Unlike the temporary legacy-turn check, these requirements also apply
    /// once all execution consumers read their captured `StepContext`.
    fn validate_active_step_settings(
        &self,
        turn_context: &TurnContext,
        settings: &ResolvedStepSettings,
        configuration: &SessionConfiguration,
    ) -> ConstraintResult<()> {
        let requirements = configuration
            .original_config_do_not_use
            .config_layer_stack
            .requirements();
        settings.revalidate(&StepSettingsConstraints {
            requirements,
            guardian_approval_enabled: self.features.enabled(Feature::GuardianApproval),
            trusted_guardian_reviewer: configuration.trusted_guardian_reviewer,
            has_full_disk_write_access: turn_context
                .file_system_sandbox_policy()
                .has_full_disk_write_access(),
        })
    }
}

#[cfg(test)]
#[path = "step_activation_tests.rs"]
mod tests;
