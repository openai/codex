use super::*;
use crate::config::PermissionProfileState;
use crate::config::test_config;
use crate::session::session::SessionConfiguration;
use crate::session::session::SessionSettingsUpdate;
use crate::session::tests::make_session_configuration_for_tests;
use codex_config::ConfigLayerStack;
use codex_config::RequirementSource;
use codex_config::Sourced;
use codex_features::Feature;
use codex_models_manager::manager::StaticModelsManager;
use codex_models_manager::model_info::model_info_from_slug;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::models::BaseInstructionsProvenance;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::AutoReviewMessages;
use codex_protocol::openai_models::GuardianV2ModelConfig;
use codex_protocol::openai_models::ModelInstructionsVariables;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxKind;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::TurnEnvironmentSelections;
use core_test_support::test_codex::local;
use pretty_assertions::assert_eq;
use std::collections::BTreeSet;
use std::sync::Arc;
use test_case::test_case;

pub(crate) fn update_selected_settings_for_test(
    settings: &mut ResolvedStepSettings,
    update: impl FnOnce(&mut StepSettings),
) {
    update(Arc::make_mut(&mut settings.selected));
}

fn set_requirements(configuration: &mut SessionConfiguration, requirements: ConfigRequirements) {
    let config = Arc::make_mut(&mut configuration.original_config_do_not_use);
    config.config_layer_stack = ConfigLayerStack::new(
        config
            .config_layer_stack
            .all_layers_low_to_high()
            .cloned()
            .collect(),
        requirements,
        config.config_layer_stack.requirements_toml().clone(),
    )
    .expect("replace test requirements");
}

#[tokio::test]
async fn proposed_permission_profile_is_checked_before_step_settings() {
    let mut configuration = make_session_configuration_for_tests().await;
    let permission = Constrained::allow_only(PermissionProfile::read_only());
    let permission_error = permission
        .can_set(&PermissionProfile::Disabled)
        .unwrap_err();
    configuration.permission_profile_state =
        PermissionProfileState::from_constrained_legacy(permission).unwrap();
    let approval = Constrained::allow_only(AskForApproval::OnRequest);
    let approval_error = approval.can_set(&AskForApproval::Never).unwrap_err();
    Arc::make_mut(&mut configuration.step_settings).approval_policy = approval;
    let mut requirements = configuration
        .original_config_do_not_use
        .config_layer_stack
        .requirements()
        .clone();
    requirements.approvals_reviewer.value = Constrained::allow_only(ApprovalsReviewer::User);
    let reviewer_error = requirements
        .approvals_reviewer
        .can_set(&ApprovalsReviewer::AutoReview)
        .unwrap_err();
    requirements.auto_review_required_models = Some(Sourced::new(
        BTreeSet::from(["protected-model".to_string()]),
        RequirementSource::Unknown,
    ));
    set_requirements(&mut configuration, requirements);
    let protected_mode = configuration.step_settings.collaboration_mode.with_updates(
        Some("protected-model".to_string()),
        /*effort*/ None,
        /*developer_instructions*/ None,
    );

    let invalid_profile = SessionSettingsUpdate {
        permission_profile: Some(PermissionProfile::Disabled),
        ..Default::default()
    };
    assert_eq!(
        configuration.apply(&invalid_profile, &[]).err().as_ref(),
        Some(&permission_error)
    );
    for (step_settings, expected) in [
        (
            StepSettingsUpdate {
                approval_policy: Some(AskForApproval::Never),
                ..Default::default()
            },
            &approval_error,
        ),
        (
            StepSettingsUpdate {
                approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
                ..Default::default()
            },
            &reviewer_error,
        ),
        (
            StepSettingsUpdate {
                collaboration_mode: Some(protected_mode),
                ..Default::default()
            },
            &reviewer_error,
        ),
    ] {
        assert_eq!(
            configuration
                .step_settings
                .apply(
                    &step_settings,
                    &configuration.step_settings_constraints(&[]),
                )
                .err()
                .as_ref(),
            Some(expected),
        );
        assert_eq!(
            configuration
                .apply(
                    &SessionSettingsUpdate {
                        step_settings,
                        ..invalid_profile.clone()
                    },
                    &[]
                )
                .err()
                .as_ref(),
            Some(&permission_error),
        );
    }
}

#[tokio::test]
async fn model_review_requirement_uses_the_proposed_permission_profile() {
    let mut configuration = make_session_configuration_for_tests().await;
    configuration.permission_profile_state = PermissionProfileState::from_constrained_legacy(
        Constrained::allow_any(PermissionProfile::Disabled),
    )
    .unwrap();
    Arc::make_mut(&mut configuration.step_settings).approvals_reviewer = ApprovalsReviewer::User;
    Arc::make_mut(&mut configuration.original_config_do_not_use)
        .features
        .enable(Feature::GuardianApproval)
        .unwrap();
    let mut requirements = configuration
        .original_config_do_not_use
        .config_layer_stack
        .requirements()
        .clone();
    requirements.auto_review_required_models = Some(Sourced::new(
        BTreeSet::from(["protected-model".to_string()]),
        RequirementSource::Unknown,
    ));
    set_requirements(&mut configuration, requirements);
    let collaboration_mode = configuration.step_settings.collaboration_mode.with_updates(
        Some("protected-model".to_string()),
        /*effort*/ None,
        /*developer_instructions*/ None,
    );
    let updates = SessionSettingsUpdate {
        step_settings: StepSettingsUpdate {
            collaboration_mode: Some(collaboration_mode.clone()),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(
        configuration.apply(&updates, &[]).err(),
        Some(ConstraintError::AutoReviewRequired {
            model: "protected-model".to_string()
        }),
    );
    let updated = configuration
        .apply(
            &SessionSettingsUpdate {
                permission_profile: Some(PermissionProfile::read_only()),
                ..updates
            },
            &[],
        )
        .expect("the proposed restricted profile permits the protected model");
    assert_eq!(
        updated.step_settings.as_ref(),
        &StepSettings {
            collaboration_mode,
            approvals_reviewer: ApprovalsReviewer::AutoReview,
            ..configuration.step_settings.as_ref().clone()
        }
    );
    assert_eq!(updated.permission_profile(), PermissionProfile::read_only());
    assert_eq!(
        updated
            .apply(
                &SessionSettingsUpdate {
                    permission_profile: Some(PermissionProfile::Disabled),
                    ..Default::default()
                },
                &[],
            )
            .err(),
        Some(ConstraintError::AutoReviewRequired {
            model: "protected-model".to_string(),
        }),
    );
}

#[tokio::test]
async fn environment_only_update_revalidates_existing_step_settings() {
    let mut configuration = make_session_configuration_for_tests().await;
    let profile = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy {
            kind: FileSystemSandboxKind::Restricted,
            glob_scan_max_depth: None,
            entries: vec![
                FileSystemSandboxEntry::new(
                    FileSystemPath::Special {
                        value: FileSystemSpecialPath::Root,
                    },
                    FileSystemAccessMode::Write,
                ),
                FileSystemSandboxEntry::new(
                    FileSystemPath::Special {
                        value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                    },
                    FileSystemAccessMode::Read,
                ),
            ],
        },
        NetworkSandboxPolicy::Restricted,
    );
    configuration.permission_profile_state =
        PermissionProfileState::from_constrained_legacy(Constrained::allow_any(profile)).unwrap();
    let settings = Arc::make_mut(&mut configuration.step_settings);
    settings.collaboration_mode = settings.collaboration_mode.with_updates(
        Some("protected-model".to_string()),
        /*effort*/ None,
        /*developer_instructions*/ None,
    );
    settings.approvals_reviewer = ApprovalsReviewer::AutoReview;
    Arc::make_mut(&mut configuration.original_config_do_not_use)
        .features
        .enable(Feature::GuardianApproval)
        .unwrap();
    let mut requirements = configuration
        .original_config_do_not_use
        .config_layer_stack
        .requirements()
        .clone();
    requirements.auto_review_required_models = Some(Sourced::new(
        BTreeSet::from(["protected-model".to_string()]),
        RequirementSource::Unknown,
    ));
    set_requirements(&mut configuration, requirements);

    let environments = vec![local(configuration.cwd().clone())];
    assert_eq!(configuration.validate(&environments), Ok(()));
    // Removing the selected workspace also removes its read-only carveout,
    // leaving this profile with full-disk write access.
    assert_eq!(
        configuration
            .apply(
                &SessionSettingsUpdate {
                    environments: Some(TurnEnvironmentSelections::new(
                        configuration.cwd().clone(),
                        Vec::new(),
                    )),
                    ..Default::default()
                },
                &environments,
            )
            .err(),
        Some(ConstraintError::AutoReviewRequired {
            model: "protected-model".to_string(),
        }),
    );
}

fn configured_settings() -> StepSettings {
    StepSettings {
        collaboration_mode: CollaborationMode {
            mode: ModeKind::Default,
            settings: Settings {
                model: "model-a".to_string(),
                reasoning_effort: Some(ReasoningEffort::Low),
                developer_instructions: Some("keep these instructions".to_string()),
            },
        },
        reasoning_summary: Some(ReasoningSummary::Concise),
        service_tier: None,
        personality: Some(Personality::Friendly),
        approval_policy: Constrained::allow_any(AskForApproval::OnRequest),
        approvals_reviewer: ApprovalsReviewer::User,
    }
}

fn step_settings_constraints(requirements: &ConfigRequirements) -> StepSettingsConstraints<'_> {
    StepSettingsConstraints {
        requirements,
        guardian_approval_enabled: false,
        trusted_guardian_reviewer: false,
        has_full_disk_write_access: false,
    }
}

#[test]
fn sparse_patch_uses_the_settings_version_being_updated() {
    let requirements = ConfigRequirements::default();
    let constraints = step_settings_constraints(&requirements);
    let initial = configured_settings();
    let tier_update = StepSettingsUpdate {
        service_tier: Some(Some("fast".to_string())),
        ..Default::default()
    };
    let latest = initial
        .apply(
            &StepSettingsUpdate {
                model: Some("model-b".to_string()),
                effort: Some(Some(ReasoningEffort::High)),
                ..Default::default()
            },
            &constraints,
        )
        .expect("model update should apply");

    let updated = latest
        .apply(&tier_update, &constraints)
        .expect("a previously prepared sparse update should apply");
    let expected = StepSettings {
        service_tier: Some(ServiceTier::Fast.request_value().to_string()),
        ..latest
    };
    assert_eq!(updated, expected);
}

#[test]
fn collaboration_replacement_wins_and_effort_clear_remains_sparse() {
    let requirements = ConfigRequirements::default();
    let constraints = step_settings_constraints(&requirements);
    let initial = configured_settings();
    let replacement = CollaborationMode {
        mode: ModeKind::Plan,
        settings: Settings {
            model: "model-b".to_string(),
            reasoning_effort: Some(ReasoningEffort::Medium),
            developer_instructions: None,
        },
    };
    let replaced = initial
        .apply(
            &StepSettingsUpdate {
                model: Some("ignored-model".to_string()),
                effort: Some(Some(ReasoningEffort::High)),
                collaboration_mode: Some(replacement.clone()),
                ..Default::default()
            },
            &constraints,
        )
        .expect("collaboration mode should replace model and effort edits");
    assert_eq!(
        replaced,
        StepSettings {
            collaboration_mode: replacement.clone(),
            ..initial
        }
    );

    let cleared = replaced
        .apply(
            &StepSettingsUpdate {
                effort: Some(None),
                ..Default::default()
            },
            &constraints,
        )
        .expect("effort clear should apply");
    assert_eq!(
        cleared,
        StepSettings {
            collaboration_mode: replacement.with_updates(
                /*model*/ None,
                Some(None),
                /*developer_instructions*/ None,
            ),
            ..replaced
        }
    );
}

#[test_case(BaseInstructionsProvenance::Custom; "explicit instructions")]
#[test_case(BaseInstructionsProvenance::Model { model: "model-a".to_string() }; "model-derived instructions")]
#[tokio::test]
async fn model_resolution_preserves_startup_overrides_and_instruction_provenance(
    provenance: BaseInstructionsProvenance,
) {
    let configured_instructions = "explicit {{ personality }}";
    let auto_review = AutoReviewMessages {
        policy: Some("catalog review policy".to_string()),
        policy_template: Some("catalog review template".to_string()),
        node_repl_policy: None,
        rejection_instructions: None,
        timeout_instructions: None,
    };
    let guardian_v2 = GuardianV2ModelConfig {
        classifier_instructions: Some("catalog classifier".to_string()),
        review_threshold_basis_points: Some(7_500),
        ..Default::default()
    };
    let mut model = model_info_from_slug("model-b");
    model.context_window = Some(90_000);
    model.max_context_window = Some(100_000);
    model.auto_compact_token_limit = Some(80_000);
    model.truncation_policy = TruncationPolicyConfig::tokens(/*limit*/ 1_000);
    let messages = model
        .model_messages
        .as_mut()
        .expect("test model should have instruction metadata");
    messages.instructions_template =
        Some("Catalog B.\n# Personality\n{{ personality }}\n# Rules\nKeep the rules.".to_string());
    messages.instructions_variables = Some(ModelInstructionsVariables {
        personality_default: Some("default".to_string()),
        personality_friendly: Some("friendly".to_string()),
        personality_pragmatic: Some("pragmatic".to_string()),
    });
    messages.auto_review = Some(auto_review);
    messages.guardian_v2 = Some(guardian_v2);
    let catalog = ModelsResponse {
        models: vec![model],
    };
    let models_manager = StaticModelsManager::new(/*auth_manager*/ None, catalog.clone());

    let mut config = test_config().await;
    config.model = Some("model-a".to_string());
    config.model_catalog = Some(catalog);
    config.model_context_window = Some(160_000);
    config.model_auto_compact_token_limit = Some(70_000);
    config.tool_output_token_limit = Some(777);
    config.base_instructions = Some(configured_instructions.to_string());
    let explicit_instructions = matches!(&provenance, BaseInstructionsProvenance::Custom);
    config.base_instructions_provenance = Some(provenance);

    // Capture the same filtered explicit overrides that session startup owns.
    let overrides = ModelInfoOverrides::from(config.to_models_manager_config());

    for (personality, personality_enabled, catalog_instructions) in [
        (
            Personality::Friendly,
            true,
            "Catalog B.\n# Personality\nfriendly\n# Rules\nKeep the rules.",
        ),
        (
            Personality::None,
            true,
            "Catalog B.\n# Rules\nKeep the rules.",
        ),
        (
            Personality::Friendly,
            false,
            "Catalog B.\n# Personality\ndefault\n# Rules\nKeep the rules.",
        ),
    ] {
        config.personality = Some(personality);
        config
            .features
            .set_enabled(Feature::Personality, personality_enabled)
            .expect("test config should allow personality changes");
        let mut settings = configured_settings();
        settings.collaboration_mode = settings.collaboration_mode.with_updates(
            Some("model-b".to_string()),
            /*effort*/ None,
            /*developer_instructions*/ None,
        );
        settings.personality = config.personality;

        let legacy = models_manager
            .get_model_info("model-b", &config.to_models_manager_config())
            .await;
        let resolved = settings
            .resolve_model_info(
                &models_manager,
                &overrides,
                config.features.enabled(Feature::Personality),
            )
            .await;
        assert_eq!(resolved, legacy);
        assert_eq!(
            resolved.get_model_instructions(settings.personality),
            if explicit_instructions {
                configured_instructions
            } else {
                catalog_instructions
            },
        );
    }
}
