use super::*;
use crate::guardian::BUNDLED_GUARDIAN_POLICY;
use crate::session::handlers::submission_loop;
use crate::session::step_context::StepContext;
use crate::session::step_settings::StepSettings;
use crate::session::tests::HeldStepTask;
use crate::session::tests::make_session_and_context;
use crate::session::tests::update_selected_settings_for_test;
use crate::session::tests::update_turn_settings_for_test;
use crate::state::TaskKind;
use codex_config::AutoReviewRequirementsToml;
use codex_config::ConfigLayerStack;
use codex_config::ConfigRequirements;
use codex_config::ConfigRequirementsToml;
use codex_config::ConfigRequirementsWithSources;
use codex_config::RequirementSource;
use codex_config::Sourced;
use codex_http_client::HttpClientFactory;
use codex_login::AuthManager;
use codex_models_manager::ModelsManagerConfig;
use codex_models_manager::bundled_models_response;
use codex_models_manager::manager::ModelsManager;
use codex_models_manager::manager::ModelsManagerFuture;
use codex_models_manager::manager::RefreshStrategy;
use codex_models_manager::manager::StaticModelsManager;
use codex_models_manager::model_info::with_config_overrides;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::openai_models::AutoReviewMessages;
use codex_protocol::openai_models::GuardianV2ModelConfig;
use codex_protocol::openai_models::GuardianV2TranscriptModelConfig;
use codex_protocol::openai_models::ModelTokenBudgetConfig;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::Submission;
use codex_protocol::protocol::TurnAbortReason;
use pretty_assertions::assert_eq;
use std::collections::BTreeSet;
use std::sync::Mutex as StdMutex;
use std::time::Duration;
use test_case::test_case;
use tokio::sync::Notify;
use tokio::sync::TryLockError;
use tokio::sync::oneshot;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

const MODEL_A: &str = "step-activation-a";
const MODEL_B: &str = "step-activation-b";

fn activation_models() -> Vec<ModelInfo> {
    let model = bundled_models_response()
        .expect("bundled models")
        .models
        .into_iter()
        .find(|model| model.slug == "gpt-5.4")
        .expect("bundled gpt-5.4");
    [MODEL_A, MODEL_B]
        .into_iter()
        .map(|slug| ModelInfo {
            slug: slug.to_string(),
            ..model.clone()
        })
        .collect()
}

#[derive(Debug, Default)]
struct ModelLookupGate {
    started: Notify,
    resume: Notify,
}

impl ModelLookupGate {
    async fn wait_until_blocked(&self) {
        timeout(Duration::from_secs(/*secs*/ 10), self.started.notified())
            .await
            .expect("model lookup started");
    }

    fn release(&self) {
        self.resume.notify_one();
    }
}

#[derive(Debug)]
struct GatedModelsManager {
    inner: StaticModelsManager,
    first_b_lookup: StdMutex<Option<Arc<ModelLookupGate>>>,
}

impl GatedModelsManager {
    fn new(models: Vec<ModelInfo>) -> (Arc<Self>, Arc<ModelLookupGate>) {
        let lookup = Arc::new(ModelLookupGate::default());
        (
            Arc::new(Self {
                inner: StaticModelsManager::new(
                    /*auth_manager*/ None,
                    ModelsResponse { models },
                ),
                first_b_lookup: StdMutex::new(Some(Arc::clone(&lookup))),
            }),
            lookup,
        )
    }
}

impl ModelsManager for GatedModelsManager {
    fn raw_model_catalog(
        &self,
        strategy: RefreshStrategy,
        factory: HttpClientFactory,
    ) -> ModelsManagerFuture<'_, ModelsResponse> {
        self.inner.raw_model_catalog(strategy, factory)
    }

    fn get_remote_models(&self) -> ModelsManagerFuture<'_, Vec<ModelInfo>> {
        self.inner.get_remote_models()
    }

    fn try_get_remote_models(&self) -> Result<Vec<ModelInfo>, TryLockError> {
        self.inner.try_get_remote_models()
    }

    fn auth_manager(&self) -> Option<&AuthManager> {
        self.inner.auth_manager()
    }

    fn list_collaboration_modes(&self) -> Vec<CollaborationModeMask> {
        self.inner.list_collaboration_modes()
    }

    fn refresh_if_new_etag(
        &self,
        etag: String,
        factory: HttpClientFactory,
    ) -> ModelsManagerFuture<'_, ()> {
        self.inner.refresh_if_new_etag(etag, factory)
    }

    fn get_model_info<'a>(
        &'a self,
        model: &'a str,
        config: &'a ModelsManagerConfig,
    ) -> ModelsManagerFuture<'a, ModelInfo> {
        Box::pin(async move {
            let pause = if model == MODEL_B {
                self.first_b_lookup.lock().expect("lookup gate").take()
            } else {
                None
            };
            if let Some(pause) = pause {
                pause.started.notify_one();
                pause.resume.notified().await;
            }
            self.inner.get_model_info(model, config).await
        })
    }
}

struct ActivationFixture {
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    finish: Arc<Notify>,
    lookup: Arc<ModelLookupGate>,
}

async fn activation_fixture(models: Vec<ModelInfo>) -> ActivationFixture {
    let (session, _) = make_session_and_context().await;
    let mut session = Arc::new(session);
    let mutable = Arc::get_mut(&mut session).expect("unshared test session");
    let (models, lookup) = GatedModelsManager::new(models);
    mutable.services.models_manager = models;
    for feature in [
        Feature::StepModelSwitching,
        Feature::FastMode,
        Feature::TokenBudget,
    ] {
        mutable
            .features
            .enable(feature)
            .expect("enable test feature");
    }
    let configuration = &mut mutable.state.get_mut().session_configuration;
    let config = Arc::make_mut(&mut configuration.original_config_do_not_use);
    config.model = Some(MODEL_A.to_string());
    config.features = mutable.features.clone();
    let settings = Arc::make_mut(&mut configuration.step_settings);
    settings.collaboration_mode = settings.collaboration_mode.with_updates(
        Some(MODEL_A.to_string()),
        Some(Some(ReasoningEffort::Low)),
        /*developer_instructions*/ None,
    );
    settings.reasoning_summary = Some(ReasoningSummary::Concise);
    settings.service_tier = None;
    let prepared = session
        .new_turn_with_default_settings("step-activation-turn".to_string(), Default::default())
        .await;
    let turn = Arc::clone(&prepared);
    let finish = Arc::new(Notify::new());
    session
        .spawn_task(
            prepared,
            Vec::new(),
            HeldStepTask {
                kind: TaskKind::Compact,
                finish: Arc::clone(&finish),
            },
        )
        .await;
    ActivationFixture {
        session,
        turn,
        finish,
        lookup,
    }
}

fn step_values(
    step: &StepContext,
) -> (
    &str,
    Option<ReasoningEffort>,
    ReasoningSummary,
    Option<&str>,
) {
    (
        step.settings.model_info.slug.as_str(),
        step.settings.reasoning_effort().cloned(),
        step.settings.reasoning_summary,
        step.settings.service_tier.as_deref(),
    )
}

async fn desired_step_settings(session: &Session) -> Arc<StepSettings> {
    Arc::clone(
        &session
            .state
            .lock()
            .await
            .session_configuration
            .step_settings,
    )
}

fn settings_submission(
    id: &str,
    turn_id: &str,
    update: TurnSettingsUpdate,
) -> (Submission, oneshot::Receiver<TurnSettingsUpdateOutcome>) {
    let (reply, receiver) = oneshot::channel();
    (
        Submission {
            id: id.to_string(),
            op: Op::TurnSettings {
                turn_id: turn_id.to_string(),
                update,
                reply,
            },
            trace: None,
            parent_turn_id: None,
            root_turn_id: None,
        },
        receiver,
    )
}

#[tokio::test]
async fn submitted_sparse_updates_preserve_captured_steps_and_ordering() {
    let mut models = activation_models();
    for model in &mut models {
        model
            .model_messages
            .as_mut()
            .expect("model messages")
            .token_budget = Some(ModelTokenBudgetConfig {
            enabled: false,
            use_history_notes_extension: false,
            reminder_threshold_tokens: 2_000,
            reminder_message_template: "{n_remaining} tokens remain.".to_string(),
            guidance_message: format!("Guidance for {}.", model.slug),
            auto_compact_fallback_prompt: "Save state before rollover.".to_string(),
            auto_compact_fallback_buffer_tokens: 4_000,
        });
    }
    let initial_model = models
        .iter_mut()
        .find(|model| model.slug == MODEL_A)
        .expect("initial model");
    initial_model.context_window = Some(272_000);
    initial_model.max_context_window = Some(272_000);
    initial_model.auto_compact_token_limit = None;
    initial_model.effective_context_window_percent = 95;
    let destination = models
        .iter_mut()
        .find(|model| model.slug == MODEL_B)
        .expect("destination model");
    destination.context_window = Some(190_000);
    destination.max_context_window = Some(190_000);
    destination.auto_compact_token_limit = Some(150_000);
    destination.effective_context_window_percent = 80;
    destination.default_reasoning_summary = ReasoningSummary::Detailed;
    destination
        .model_messages
        .as_mut()
        .expect("model messages")
        .instructions_template = Some("Destination-model instructions.".to_string());
    let expected_destination = destination.clone();
    let ActivationFixture {
        session,
        turn,
        lookup,
        finish,
    } = activation_fixture(models).await;
    let model_manager_config = {
        let state = session.state.lock().await;
        let configuration = &state.session_configuration;
        configuration.model_info_overrides.models_manager_config(
            configuration.step_settings.personality,
            session.features.enabled(Feature::Personality),
        )
    };
    let expected_destination = with_config_overrides(expected_destination, &model_manager_config);
    let desired = desired_step_settings(&session).await;
    let (submissions, receiver) = async_channel::unbounded();
    let loop_task = tokio::spawn(submission_loop(
        Arc::clone(&session),
        session.get_config().await,
        receiver,
    ));
    let before = session
        .capture_step_context(Arc::clone(&turn), &CancellationToken::new())
        .await
        .expect("capture initial step");
    let (submission, first_reply) = settings_submission(
        "activate-model",
        &turn.sub_id,
        TurnSettingsUpdate {
            model: Some(MODEL_B.to_string()),
            effort: Some(Some(ReasoningEffort::Low)),
            ..Default::default()
        },
    );
    submissions
        .send(submission)
        .await
        .expect("submit model update");
    lookup.wait_until_blocked().await;
    let refresh = session
        .mcp_refresh
        .acquire()
        .await
        .expect("MCP refresh lock");
    session.mark_mcp_runtime_dirty();
    let capture_cancel = CancellationToken::new();
    let mut during = Box::pin(tokio::task::unconstrained(
        session.capture_step_context(Arc::clone(&turn), &capture_cancel),
    ));
    // Capture A, then hold asynchronous planning across publication of B.
    {
        let mut context = std::task::Context::from_waker(futures::task::noop_waker_ref());
        assert!(std::future::Future::poll(during.as_mut(), &mut context).is_pending());
    }
    let priority = ServiceTier::Fast.request_value();
    let mut replies = vec![first_reply];
    for (id, update) in [
        (
            "activate-reasoning",
            TurnSettingsUpdate {
                effort: Some(Some(ReasoningEffort::High)),
                summary: Some(ReasoningSummary::Detailed),
                ..Default::default()
            },
        ),
        (
            "activate-tier",
            TurnSettingsUpdate {
                service_tier: Some(Some(priority.to_string())),
                ..Default::default()
            },
        ),
    ] {
        let (submission, reply) = settings_submission(id, &turn.sub_id, update);
        submissions.send(submission).await.expect("queue update");
        replies.push(reply);
    }
    lookup.release();
    // Await each operation's publication result, including the patches queued
    // behind the blocked model lookup.
    for reply in replies {
        assert_eq!(
            timeout(Duration::from_secs(/*secs*/ 10), reply)
                .await
                .expect("settings completion")
                .expect("settings reply"),
            TurnSettingsUpdateOutcome::Applied,
        );
    }
    drop(refresh);
    let during = during.await.expect("capture spanning activation");
    assert!(Arc::ptr_eq(
        &before.settings.model_info,
        &during.settings.model_info
    ));
    let after = session
        .capture_step_context(Arc::clone(&turn), &CancellationToken::new())
        .await
        .expect("capture published settings");
    let initial = (
        MODEL_A,
        Some(ReasoningEffort::Low),
        ReasoningSummary::Concise,
        None,
    );
    assert_eq!(
        [
            step_values(&before),
            step_values(&during),
            step_values(&after)
        ],
        [
            initial.clone(),
            initial,
            (
                MODEL_B,
                Some(ReasoningEffort::High),
                ReasoningSummary::Detailed,
                Some(priority),
            ),
        ]
    );
    assert!(Arc::ptr_eq(&before.turn, &after.turn));
    assert_eq!(after.settings.model_info.as_ref(), &expected_destination);
    let initial_budget = turn
        .config
        .token_budget
        .clone()
        .expect("initial model budget");
    let destination_budget = crate::config::TokenBudgetConfig {
        guidance_message: Some(format!("Guidance for {MODEL_B}.")),
        ..initial_budget.clone()
    };
    assert_eq!(
        [&before, &during, &after].map(|step| step.token_budget.clone()),
        [
            Some(initial_budget.clone()),
            Some(initial_budget),
            Some(destination_budget),
        ]
    );
    assert_eq!(
        [&before, &during, &after].map(|step| {
            (
                step.settings.model_info.resolved_context_window(),
                step.settings.model_info.usable_context_window(),
                step.settings.model_info.auto_compact_token_limit(),
            )
        }),
        [
            (Some(272_000), Some(258_400), Some(244_800)),
            (Some(272_000), Some(258_400), Some(244_800)),
            (Some(190_000), Some(152_000), Some(150_000)),
        ]
    );
    assert_eq!(desired_step_settings(&session).await, desired);
    assert!(Arc::ptr_eq(&before.settings, &during.settings));
    assert!(Arc::ptr_eq(&before.settings, &turn.initial_settings));
    assert_eq!(turn.model_info().slug, MODEL_A);

    let done = {
        let active = session.active_turn.lock().await;
        Arc::clone(&active.as_ref().unwrap().task.as_ref().unwrap().done)
    };
    let completed = done.notified();
    finish.notify_one();
    timeout(Duration::from_secs(/*secs*/ 10), completed)
        .await
        .expect("original task completed");
    assert!(session.active_turn.lock().await.is_none());

    // A retained context owns its last published snapshot even after the task
    // is unregistered. Capture must not fall back to the initial turn model.
    let retained = session
        .capture_step_context(Arc::clone(&turn), &CancellationToken::new())
        .await
        .expect("capture retained context after task completion");
    assert!(Arc::ptr_eq(&retained.settings, &after.settings));
    assert_eq!(step_values(&retained), step_values(&after));
    assert_eq!(before.settings.model_info.slug, MODEL_A);
    assert_eq!(turn.model_info().slug, MODEL_A);
    drop(submissions);
    loop_task.await.expect("submission loop teardown");
}

#[derive(Clone, Copy)]
enum TaskChangeDuringLookup {
    CancelledWithRejectedDestination,
    FinishedAndReplaced,
    FinishedAndReusedContext,
}

#[test_case(TaskChangeDuringLookup::CancelledWithRejectedDestination; "cancelled task is unavailable even when destination is rejected")]
#[test_case(TaskChangeDuringLookup::FinishedAndReplaced; "completed task is replaced")]
#[test_case(TaskChangeDuringLookup::FinishedAndReusedContext; "completed context is reused by another task")]
#[tokio::test]
async fn delayed_activation_does_not_retarget_a_task(change: TaskChangeDuringLookup) {
    let mut models = activation_models();
    if matches!(
        change,
        TaskChangeDuringLookup::CancelledWithRejectedDestination
    ) {
        models
            .iter_mut()
            .find(|model| model.slug == MODEL_B)
            .expect("destination model")
            .node_repl_disabled = true;
    }
    let ActivationFixture {
        session,
        turn,
        finish,
        lookup,
    } = activation_fixture(models).await;
    let desired = desired_step_settings(&session).await;
    let original = turn.current_settings.load_full();
    let update_session = Arc::clone(&session);
    let turn_id = turn.sub_id.clone();
    let update = tokio::spawn(async move {
        update_session
            .apply_turn_settings(
                &turn_id,
                TurnSettingsUpdate {
                    model: Some(MODEL_B.to_string()),
                    ..Default::default()
                },
            )
            .await
    });
    lookup.wait_until_blocked().await;
    assert_eq!(desired_step_settings(&session).await, desired);
    let (cancellation_token, done) = {
        let active = session.active_turn.lock().await;
        let task = active
            .as_ref()
            .and_then(|active| active.task.as_ref())
            .expect("active task");
        (task.cancellation_token.clone(), Arc::clone(&task.done))
    };
    let (expected_turn, expected_settings) = match change {
        TaskChangeDuringLookup::CancelledWithRejectedDestination => {
            cancellation_token.cancel();
            (Arc::clone(&turn), original)
        }
        TaskChangeDuringLookup::FinishedAndReplaced
        | TaskChangeDuringLookup::FinishedAndReusedContext => {
            let completed = done.notified();
            finish.notify_one();
            timeout(Duration::from_secs(/*secs*/ 10), completed)
                .await
                .expect("original task completed");
            let replacement = match change {
                TaskChangeDuringLookup::FinishedAndReplaced => {
                    session
                        .new_turn_with_default_settings(
                            "replacement-turn".to_string(),
                            Default::default(),
                        )
                        .await
                }
                TaskChangeDuringLookup::FinishedAndReusedContext => Arc::clone(&turn),
                TaskChangeDuringLookup::CancelledWithRejectedDestination => unreachable!(),
            };
            let settings = replacement.current_settings.load_full();
            session
                .spawn_task(
                    Arc::clone(&replacement),
                    Vec::new(),
                    HeldStepTask {
                        kind: TaskKind::Compact,
                        finish: Arc::new(Notify::new()),
                    },
                )
                .await;
            assert!(Arc::ptr_eq(&turn.current_settings.load_full(), &original));
            (replacement, settings)
        }
    };
    lookup.release();
    assert_eq!(
        update.await.expect("activation task"),
        TurnSettingsUpdateOutcome::TargetUnavailable
    );
    assert!(Arc::ptr_eq(
        &expected_turn.current_settings.load_full(),
        &expected_settings,
    ));
    assert_eq!(desired_step_settings(&session).await, desired);
    session.abort_all_tasks(TurnAbortReason::Replaced).await;
}

#[derive(Clone, Copy)]
enum ManagedAuthorizationChange {
    ApprovalPolicy,
    ApprovalsReviewer,
}

#[test_case(ManagedAuthorizationChange::ApprovalPolicy; "approval policy refreshed during lookup")]
#[test_case(ManagedAuthorizationChange::ApprovalsReviewer; "approvals reviewer refreshed during lookup")]
#[tokio::test]
async fn delayed_activation_rechecks_live_managed_authorization(
    change: ManagedAuthorizationChange,
) {
    let ActivationFixture {
        session,
        turn,
        lookup,
        ..
    } = activation_fixture(activation_models()).await;
    let original = turn.current_settings.load_full();
    let desired = desired_step_settings(&session).await;
    let update_session = Arc::clone(&session);
    let turn_id = turn.sub_id.clone();
    let update = tokio::spawn(async move {
        update_session
            .apply_turn_settings(
                &turn_id,
                TurnSettingsUpdate {
                    model: Some(MODEL_B.to_string()),
                    ..Default::default()
                },
            )
            .await
    });
    lookup.wait_until_blocked().await;
    assert_eq!(desired_step_settings(&session).await, desired);

    // Refresh only a live constraint, leaving the admitted settings and the
    // legacy model-authority classifications unchanged.
    let source = RequirementSource::EnterpriseManaged {
        id: "refreshed-policy".to_string(),
        name: "Refreshed policy".to_string(),
    };
    let sourced = match change {
        ManagedAuthorizationChange::ApprovalPolicy => {
            let allowed = if original.approval_policy() == AskForApproval::Never {
                AskForApproval::OnRequest
            } else {
                AskForApproval::Never
            };
            ConfigRequirementsWithSources {
                allowed_approval_policies: Some(Sourced::new(vec![allowed], source)),
                ..Default::default()
            }
        }
        ManagedAuthorizationChange::ApprovalsReviewer => {
            let allowed = if original.approvals_reviewer() == ApprovalsReviewer::User {
                ApprovalsReviewer::AutoReview
            } else {
                ApprovalsReviewer::User
            };
            ConfigRequirementsWithSources {
                allowed_approvals_reviewers: Some(Sourced::new(vec![allowed], source)),
                ..Default::default()
            }
        }
    };
    let requirements =
        ConfigRequirements::try_from(sourced.clone()).expect("normalize refreshed requirements");
    let expected_error = {
        let mut state = session.state.lock().await;
        let config = Arc::make_mut(&mut state.session_configuration.original_config_do_not_use);
        config.config_layer_stack = ConfigLayerStack::new(
            config
                .config_layer_stack
                .all_layers_low_to_high()
                .cloned()
                .collect(),
            requirements,
            sourced.into_toml(),
        )
        .expect("build refreshed requirements");
        let requirements = config.config_layer_stack.requirements();
        match change {
            ManagedAuthorizationChange::ApprovalPolicy => requirements
                .approval_policy
                .can_set(&original.approval_policy()),
            ManagedAuthorizationChange::ApprovalsReviewer => requirements
                .approvals_reviewer
                .can_set(&original.approvals_reviewer()),
        }
        .expect_err("the refreshed constraint rejects the admitted value")
    };
    lookup.release();
    assert_eq!(
        update.await.expect("activation task"),
        TurnSettingsUpdateOutcome::Rejected {
            reason: expected_error.to_string(),
        }
    );
    assert!(Arc::ptr_eq(&turn.current_settings.load_full(), &original,));
    assert_eq!(desired_step_settings(&session).await, desired);
    session.abort_all_tasks(TurnAbortReason::Replaced).await;
}

#[derive(Clone, Copy)]
enum ManagedPolicyChange {
    RequireReview,
    IgnorePrefixRules,
}

#[test_case(ManagedPolicyChange::RequireReview; "required review changed since admission")]
#[test_case(ManagedPolicyChange::IgnorePrefixRules; "prefix rule policy changed since admission")]
#[tokio::test]
async fn activation_must_match_the_retained_turn_authority(change: ManagedPolicyChange) {
    let (mut session, mut turn) = make_session_and_context().await;
    session
        .features
        .enable(Feature::GuardianApproval)
        .expect("enable Guardian for the test");
    let config = Arc::make_mut(&mut turn.config);
    config.approvals_reviewer = ApprovalsReviewer::AutoReview;
    config.config_layer_stack = ConfigLayerStack::new(
        config
            .config_layer_stack
            .all_layers_low_to_high()
            .cloned()
            .collect(),
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("build admitted requirements");
    update_turn_settings_for_test(&mut turn, |settings| {
        update_selected_settings_for_test(settings, |selected| {
            selected.approvals_reviewer = ApprovalsReviewer::AutoReview;
        });
        let model_info = Arc::make_mut(&mut settings.model_info);
        model_info.model_specialty = None;
        model_info.used_fallback_model_metadata = false;
    });
    assert!(
        !turn
            .file_system_sandbox_policy()
            .has_full_disk_write_access()
    );

    let prepared = Arc::new(turn);
    let current = &prepared.initial_settings;
    let mut model_info = current.model_info.as_ref().clone();
    model_info.slug = "step-settings-policy-destination".to_string();
    let mut selected = current.selected().clone();
    selected.collaboration_mode.settings.model = model_info.slug.clone();
    let destination = ResolvedStepSettings::new(
        Arc::new(selected),
        Arc::new(model_info),
        session.features.enabled(Feature::FastMode),
    );
    let mut live = session.state.lock().await.session_configuration.clone();
    live.original_config_do_not_use = Arc::clone(&prepared.config);
    assert_eq!(
        session.validate_active_step_settings(&prepared, &destination, &live,),
        Ok(())
    );
    assert_eq!(
        check_legacy_turn_safety(
            &prepared,
            current,
            &destination,
            &live.original_config_do_not_use,
        ),
        Ok(())
    );

    // Both model names acquire the same live classification. Comparing only
    // those names in the latest requirements would therefore accept the switch,
    // even though remaining consumers still use the admitted turn's authority.
    let models = vec![
        current.model_info.slug.clone(),
        destination.model_info.slug.clone(),
    ];
    let mut requirements = ConfigRequirements::default();
    let auto_review = match change {
        ManagedPolicyChange::RequireReview => {
            requirements.auto_review_required_models = Some(Sourced::new(
                BTreeSet::from_iter(models.iter().cloned()),
                RequirementSource::Unknown,
            ));
            AutoReviewRequirementsToml {
                required_on_models: Some(models),
                ..Default::default()
            }
        }
        ManagedPolicyChange::IgnorePrefixRules => AutoReviewRequirementsToml {
            ignore_rules: Some(models),
            ..Default::default()
        },
    };
    let config = Arc::make_mut(&mut live.original_config_do_not_use);
    config.config_layer_stack = ConfigLayerStack::new(
        config
            .config_layer_stack
            .all_layers_low_to_high()
            .cloned()
            .collect(),
        requirements,
        ConfigRequirementsToml {
            auto_review: Some(auto_review),
            ..Default::default()
        },
    )
    .expect("build refreshed requirements");
    // The destination satisfies today's managed policy. Only the temporary
    // compatibility check rejects the mismatch with retained turn consumers.
    assert_eq!(
        session.validate_active_step_settings(&prepared, &destination, &live,),
        Ok(())
    );
    assert_eq!(
        check_legacy_turn_safety(
            &prepared,
            current,
            &destination,
            &live.original_config_do_not_use,
        ),
        Err(match change {
            ManagedPolicyChange::RequireReview => {
                "the destination changes model-required approval authority".to_string()
            }
            ManagedPolicyChange::IgnorePrefixRules => {
                "the destination changes the admitted prefix-rule policy".to_string()
            }
        })
    );
}

fn safety_models() -> (ModelInfo, ModelInfo) {
    let [mut admitted, mut destination]: [ModelInfo; 2] = activation_models()
        .try_into()
        .expect("two activation models");
    for model in [&mut admitted, &mut destination] {
        model.model_specialty = None;
        model.node_repl_auto_review_required = false;
        model.node_repl_disabled = false;
        model.auto_review_model_override = None;
        model.used_fallback_model_metadata = false;
        let messages = model.model_messages.as_mut().expect("model messages");
        messages.auto_review = None;
        messages.guardian_v2 = None;
    }
    (admitted, destination)
}

fn parent_review_messages(model: &mut ModelInfo) -> &mut AutoReviewMessages {
    model
        .model_messages
        .as_mut()
        .expect("model messages")
        .auto_review
        .get_or_insert(AutoReviewMessages {
            policy: None,
            policy_template: None,
            node_repl_policy: None,
            rejection_instructions: None,
            timeout_instructions: None,
        })
}

#[derive(Clone, Copy)]
enum ModelSafetyChange {
    Cyber,
    NodeReplReview,
    NodeReplDisabled,
    ReviewerOverride,
}

#[test_case(ModelSafetyChange::Cyber, "the destination changes the admitted Guardian rejection policy"; "Cyber rejection policy")]
#[test_case(ModelSafetyChange::NodeReplReview, "the destination changes the admitted node REPL review requirement"; "node REPL review")]
#[test_case(ModelSafetyChange::NodeReplDisabled, "the destination changes the admitted node REPL availability restriction"; "node REPL availability")]
#[test_case(ModelSafetyChange::ReviewerOverride, "the destination changes the explicit Guardian reviewer model"; "explicit reviewer")]
#[tokio::test]
async fn model_safety_changes_must_match_the_admitted_authority(
    change: ModelSafetyChange,
    reason: &str,
) {
    let (_, turn) = make_session_and_context().await;
    let (admitted, mut destination) = safety_models();
    match change {
        ModelSafetyChange::Cyber => {
            destination.model_specialty = Some(MODEL_SPECIALTY_CYBER.to_string());
        }
        ModelSafetyChange::NodeReplReview => destination.node_repl_auto_review_required = true,
        ModelSafetyChange::NodeReplDisabled => destination.node_repl_disabled = true,
        ModelSafetyChange::ReviewerOverride => {
            destination.auto_review_model_override = Some("required-reviewer".to_string());
        }
    }
    for current in [&admitted, &destination] {
        assert_eq!(
            check_legacy_model_safety(&admitted, current, &destination, &turn.config, &turn.config,),
            Err(reason.to_string())
        );
    }
}

#[tokio::test]
async fn model_safety_rejects_fallback_metadata() {
    let (_, turn) = make_session_and_context().await;
    let (admitted, destination) = safety_models();
    let mut fallback = admitted.clone();
    fallback.used_fallback_model_metadata = true;
    for (old, current, next, reason) in [
        (
            &fallback,
            &admitted,
            &destination,
            "the active model has only fallback metadata",
        ),
        (
            &admitted,
            &fallback,
            &destination,
            "the active model has only fallback metadata",
        ),
        (
            &admitted,
            &admitted,
            &fallback,
            "the destination model has only fallback metadata",
        ),
    ] {
        assert_eq!(
            check_legacy_model_safety(old, current, next, &turn.config, &turn.config),
            Err(reason.to_string())
        );
    }
}

#[test_case(None, None, false; "both catalog policies")]
#[test_case(Some("admitted policy"), None, false; "live config unmasks catalog")]
#[test_case(None, Some("live policy"), false; "admitted config unmasks catalog")]
#[test_case(Some("admitted policy"), Some("live policy"), true; "both configured policies mask catalog")]
#[tokio::test]
async fn parent_fallback_policy_uses_both_config_lifetimes(
    admitted_policy: Option<&str>,
    live_policy: Option<&str>,
    allowed: bool,
) {
    let (_, turn) = make_session_and_context().await;
    let mut admitted_config = turn.config.as_ref().clone();
    admitted_config.guardian_policy_config = admitted_policy.map(str::to_string);
    let mut live_config = admitted_config.clone();
    live_config.guardian_policy_config = live_policy.map(str::to_string);
    let (mut admitted, mut destination) = safety_models();
    parent_review_messages(&mut admitted).policy = Some("catalog policy A".to_string());
    parent_review_messages(&mut destination).policy = Some("catalog policy B".to_string());
    assert_eq!(
        check_legacy_model_safety(
            &admitted,
            &admitted,
            &destination,
            &admitted_config,
            &live_config,
        ),
        if allowed {
            Ok(())
        } else {
            Err("the destination changes the Guardian parent-fallback policy".to_string())
        }
    );
}

#[tokio::test]
async fn parent_fallback_preserves_explicit_empty_and_bundled_defaults() {
    let (_, turn) = make_session_and_context().await;
    let mut config = turn.config.as_ref().clone();
    config.guardian_policy_config = None;
    let (admitted, mut destination) = safety_models();
    let check = |destination: &ModelInfo| {
        check_legacy_model_safety(&admitted, &admitted, destination, &config, &config)
    };
    parent_review_messages(&mut destination).policy = Some(BUNDLED_GUARDIAN_POLICY.to_string());
    parent_review_messages(&mut destination).policy_template =
        Some(BUNDLED_GUARDIAN_POLICY_TEMPLATE.to_string());
    assert_eq!(check(&destination), Ok(()));
    parent_review_messages(&mut destination).policy_template = Some(String::new());
    assert_eq!(
        check(&destination),
        Err("the destination changes the Guardian parent-fallback policy template".to_string())
    );
    parent_review_messages(&mut destination).policy_template = None;
    parent_review_messages(&mut destination).policy = Some(String::new());
    assert_eq!(
        check(&destination),
        Err("the destination changes the Guardian parent-fallback policy".to_string())
    );
    parent_review_messages(&mut destination).policy = None;
    assert_eq!(check(&destination), Ok(()));
    parent_review_messages(&mut destination).node_repl_policy = Some(String::new());
    assert_eq!(
        check(&destination),
        Err("the destination changes the Guardian parent-fallback node REPL policy".to_string())
    );
}

#[tokio::test]
async fn unchanged_explicit_reviewer_does_not_use_parent_policy() {
    let (_, turn) = make_session_and_context().await;
    let (mut admitted, mut destination) = safety_models();
    for model in [&mut admitted, &mut destination] {
        model.auto_review_model_override = Some("shared-reviewer".to_string());
    }
    parent_review_messages(&mut admitted).policy = Some("catalog policy A".to_string());
    parent_review_messages(&mut destination).policy = Some("catalog policy B".to_string());
    parent_review_messages(&mut destination).policy_template = Some(String::new());
    parent_review_messages(&mut destination).node_repl_policy = Some(String::new());
    assert_eq!(
        check_legacy_model_safety(
            &admitted,
            &admitted,
            &destination,
            &turn.config,
            &turn.config,
        ),
        Ok(())
    );
}

#[test_case(GuardianV2ModelConfig {
    classifier_instructions: Some("Review the complete action.".to_string()),
    ..Default::default()
}; "classifier instructions")]
#[test_case(GuardianV2ModelConfig {
    review_threshold_basis_points: Some(6_000),
    ..Default::default()
}; "review threshold")]
#[test_case(GuardianV2ModelConfig {
    reasoning_effort: Some(ReasoningEffort::High),
    ..Default::default()
}; "classifier effort")]
#[test_case(GuardianV2ModelConfig {
    transcript: Some(GuardianV2TranscriptModelConfig {
        sources: Some(vec!["reasoning".to_string()]),
        ..Default::default()
    }),
    ..Default::default()
}; "transcript sources")]
#[test_case(GuardianV2ModelConfig {
    transcript: Some(GuardianV2TranscriptModelConfig {
        max_message_entry_tokens: Some(128),
        ..Default::default()
    }),
    ..Default::default()
}; "transcript bounds")]
#[test_case(GuardianV2ModelConfig {
    max_action_tokens: Some(128),
    ..Default::default()
}; "action limit")]
#[test_case(GuardianV2ModelConfig {
    max_classifier_instruction_tokens: Some(256),
    ..Default::default()
}; "classifier instruction limit")]
#[test_case(GuardianV2ModelConfig {
    max_parent_compaction_tokens: Some(384),
    ..Default::default()
}; "parent compaction limit")]
#[tokio::test]
async fn guardian_v2_classification_settings_match_the_admitted_authority(
    destination_settings: GuardianV2ModelConfig,
) {
    let (_, turn) = make_session_and_context().await;
    let mut config = turn.config.as_ref().clone();
    for feature in [Feature::GuardianV2, Feature::GuardianApproval] {
        config
            .features
            .set_enabled(feature, /*enabled*/ true)
            .expect("enable Guardian V2");
    }
    // V2 classification does not depend on the default approval reviewer or
    // the explicitly selected model used to resolve the security policy.
    config.approvals_reviewer = ApprovalsReviewer::User;
    let (mut admitted, mut destination) = safety_models();
    destination
        .model_messages
        .as_mut()
        .expect("model messages")
        .guardian_v2 = Some(destination_settings);
    for reviewer_override in [None, Some("shared-reviewer".to_string())] {
        admitted.auto_review_model_override = reviewer_override.clone();
        destination.auto_review_model_override = reviewer_override;
        for current in [&admitted, &destination] {
            assert_eq!(
                check_legacy_model_safety(&admitted, current, &destination, &config, &config,),
                Err(
                    "the destination changes the admitted Guardian V2 classification settings"
                        .to_string()
                )
            );
        }
    }
}

#[test_case(false, true; "Guardian V2 disabled")]
#[test_case(true, false; "Guardian approval disabled")]
#[tokio::test]
async fn inactive_guardian_v2_does_not_restrict_model_defaults(
    guardian_v2_enabled: bool,
    guardian_approval_enabled: bool,
) {
    let (_, turn) = make_session_and_context().await;
    let mut admitted_config = turn.config.as_ref().clone();
    admitted_config
        .features
        .set_enabled(Feature::GuardianV2, guardian_v2_enabled)
        .expect("configure admitted Guardian V2");
    admitted_config
        .features
        .set_enabled(Feature::GuardianApproval, guardian_approval_enabled)
        .expect("configure admitted Guardian approval");
    let mut live_config = admitted_config.clone();
    for feature in [Feature::GuardianV2, Feature::GuardianApproval] {
        live_config
            .features
            .set_enabled(feature, /*enabled*/ true)
            .expect("enable live Guardian V2");
    }
    let (admitted, mut destination) = safety_models();
    let destination_settings = GuardianV2ModelConfig {
        review_threshold_basis_points: Some(6_000),
        ..Default::default()
    };
    destination
        .model_messages
        .as_mut()
        .expect("model messages")
        .guardian_v2 = Some(destination_settings);
    assert_eq!(
        check_legacy_model_safety(
            &admitted,
            &admitted,
            &destination,
            &admitted_config,
            &live_config,
        ),
        Ok(())
    );
}

#[tokio::test]
async fn guardian_v2_empty_model_defaults_are_equivalent() {
    let (_, turn) = make_session_and_context().await;
    let mut config = turn.config.as_ref().clone();
    for feature in [Feature::GuardianV2, Feature::GuardianApproval] {
        config
            .features
            .set_enabled(feature, /*enabled*/ true)
            .expect("enable Guardian V2");
    }
    let (admitted, mut destination) = safety_models();
    destination.context_window = Some(123_456);
    for settings in [
        GuardianV2ModelConfig::default(),
        GuardianV2ModelConfig {
            transcript: Some(GuardianV2TranscriptModelConfig::default()),
            ..Default::default()
        },
    ] {
        destination
            .model_messages
            .as_mut()
            .expect("model messages")
            .guardian_v2 = Some(settings);
        assert_eq!(
            check_legacy_model_safety(&admitted, &admitted, &destination, &config, &config,),
            Ok(())
        );
    }
}
