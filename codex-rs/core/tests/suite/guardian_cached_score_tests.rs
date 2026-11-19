//! Exercises async Guardian and observational probe flags through real tool calls and approvals.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_config::FeatureRequirementsToml;
use codex_config::RequirementSource;
use codex_config::Sourced;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_core::TurnInputRequest;
use codex_core::config::Config;
use codex_core::config::Constrained;
use codex_core::config::GuardianInferenceProvider;
use codex_core::config::ManagedFeatures;
use codex_core::context::NodeReplReviewEvidence;
use codex_features::Feature;
use codex_history::CodexHarnessMetadata;
use codex_history::CompactedItem;
use codex_history::InitialHistory;
use codex_history::ResponseItemEnvelope;
use codex_history::ResumedHistory;
use codex_history::RetainedContextEvent;
use codex_history::RolloutItem;
use codex_history::VerifiedAnswer;
use codex_history::VerifiedQuestionAnswer;
use codex_login::CodexAuth;
use codex_otel::MetricsClient;
use codex_otel::MetricsConfig;
use codex_protocol::ResponseItemId;
use codex_protocol::approvals::GuardianAssessmentStatus;
use codex_protocol::approvals::GuardianReviewReason;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::ImageReference;
use codex_protocol::models::NetworkPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::GuardianModelPolicy;
use codex_protocol::openai_models::GuardianReviewMode;
use codex_protocol::openai_models::GuardianV2ModelConfig;
use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::request_permissions::PermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionProfile;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_protocol::user_input::UserInput;
use core_test_support::hooks::trust_discovered_hooks;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_target_windows;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;
use tokio::sync::oneshot;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::method;
use wiremock::matchers::path;

const BYO_ACTOR_MODEL: &str = "external-actor";
const LUNA_MODEL: &str = "gpt-5.6-luna";
const GUARDIAN_SCREENSHOT: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";
const BYO_HEADERS: [(&str, &str); 2] = [
    ("x-byo-key", "actor-only"),
    ("authorization", "Bearer actor-only"),
];

#[test_case(ApprovalsReviewer::User, AskForApproval::OnRequest, PermissionProfile::read_only(); "user_approvals")]
#[test_case(ApprovalsReviewer::AutoReview, AskForApproval::Never, PermissionProfile::Disabled; "full_access")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn compacted_cold_resume_respects_guardian_provider_override(
    reviewer: ApprovalsReviewer,
    approval_policy: AskForApproval,
    profile: PermissionProfile,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let actor = responses::start_mock_server().await;
    let guardian = responses::start_mock_server().await;
    let guardian_url = format!("{}/v1", guardian.uri());
    let builder = || {
        let guardian_url = guardian_url.clone();
        let profile = profile.clone();
        test_codex()
            .with_model_info_override(BYO_ACTOR_MODEL, |model| {
                model.guardian = Some(GuardianModelPolicy {
                    computer_use: Some(GuardianReviewMode::Adaptive),
                    ..Default::default()
                });
            })
            .with_config(move |config| {
                configure_explicit_guardian_provider(config, &guardian_url);
                config
                    .features
                    .enable(Feature::GuardianV2)
                    .expect("enable scoring");
                pin_guardian_parent_compaction_reuse(config);
                config.guardian_policy_config = None;
                config.approvals_reviewer = reviewer;
                config.permissions.approval_policy = Constrained::allow_any(approval_policy);
                config
                    .permissions
                    .set_permission_profile(profile)
                    .expect("set permissions");
            })
    };
    let test = builder().build_with_auto_env(&actor).await?;
    let mut compacted: CompactedItem = serde_json::from_value(json!({"message": ""}))?;
    compacted.replacement_history = Some(vec![ResponseItemEnvelope {
        item: ResponseItem::Compaction {
            id: Some(ResponseItemId::with_suffix("cmp", "checkpoint")),
            encrypted_content: "opaque checkpoint".to_owned(),
            internal_chat_message_metadata_passthrough: None,
        },
        metadata: Some(CodexHarnessMetadata {
            compaction_model_hash: Some("reviewer".to_owned()),
            ..Default::default()
        }),
    }]);
    test.codex.ensure_rollout_materialized().await;
    test.codex
        .append_rollout_items(&[RolloutItem::Compacted(Box::new(compacted))])
        .await?;
    let resumed = builder().restart_with_auto_env(&actor, &test).await?;
    let snapshot = resumed.codex.conversation_history_snapshot().await;
    let checkpoint = snapshot
        .latest_compaction()
        .context("restored opaque checkpoint")?;
    assert!(checkpoint.is_usable());
    assert!(snapshot.uses_independent_review_history());
    assert!(
        resumed
            .codex
            .config()
            .await
            .features
            .enabled(Feature::GuardianReuseParentCompaction),
        "the managed feature must remain enabled",
    );
    resumed.codex.shutdown_and_wait().await?;
    assert!(
        guardian
            .received_requests()
            .await
            .expect("record Guardian requests")
            .is_empty(),
        "compacted cold resume should not request Guardian /models"
    );
    Ok(())
}

#[test_case(AskForApproval::OnRequest, PermissionProfile::read_only(), None, true; "on_request_restricted")]
#[test_case(AskForApproval::Never, PermissionProfile::read_only(), None, true; "never_restricted")]
#[test_case(AskForApproval::Never, PermissionProfile::Disabled, None, false; "full_access")]
#[test_case(AskForApproval::Never, PermissionProfile::read_only(), Some("allowed_approvals_reviewers = ['auto_review']"), false; "mandatory_review")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_guardian_recovers_without_delaying_tool_dispatch(
    approval_policy: AskForApproval,
    profile: PermissionProfile,
    reviewer_requirements: Option<&str>,
    should_recover: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_target_windows!(Ok(()), "exec_command requires a POSIX executor");
    let actor = responses::start_mock_server().await;
    let guardian = responses::start_mock_server().await;
    let guardian_url = format!("{}/v1", guardian.uri());
    let catalog = MockServer::start().await;
    let catalog_url = format!("{}/v1/models", catalog.uri());
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .expect(u64::from(should_recover))
        .mount(&catalog)
        .await;
    let mut classifier = codex_models_manager::model_info::model_info_from_slug(LUNA_MODEL);
    classifier.supported_in_api = true;
    classifier.use_responses_lite = false;
    classifier.guardian = Some(GuardianModelPolicy {
        shell: Some(GuardianReviewMode::Adaptive),
        sandboxed_exec_commands: Some(true),
        ..Default::default()
    });
    let response = ResponseTemplate::new(200).set_body_json(ModelsResponse {
        models: vec![classifier],
    });
    let gate = tokio_util::sync::CancellationToken::new();
    let pending = gate.clone();
    let started = Arc::new(tokio::sync::Notify::new());
    let notify = Arc::clone(&started);
    let _retry = Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(move |_: &Request| {
            notify.notify_one();
            futures::executor::block_on(pending.cancelled());
            response.clone()
        })
        .with_priority(6)
        .expect(u64::from(should_recover))
        .mount_as_scoped(&catalog)
        .await;
    let release = gate.drop_guard();
    let scorer = responses::mount_sse_once(
        &guardian,
        sse(vec![
            ev_response_created("score"),
            ev_assistant_message("score", "low"),
            ev_completed("score"),
        ]),
    )
    .await;
    let builder = test_codex()
        .with_model(BYO_ACTOR_MODEL)
        .with_config(move |config| {
            configure_explicit_guardian_provider(config, &guardian_url);
            let provider = Arc::make_mut(
                config
                    .guardian_model_provider
                    .as_mut()
                    .expect("Guardian provider"),
            );
            provider.info.model_catalog_url = Some(catalog_url.into());
            provider.info.request_max_retries = Some(0);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config
                .features
                .enable(Feature::GuardianApproval)
                .expect("enable Guardian approval");
            config
                .features
                .enable(Feature::GuardianV2)
                .expect("enable Guardian V2");
            config.permissions.approval_policy = Constrained::allow_any(approval_policy);
            config
                .permissions
                .set_permission_profile(profile)
                .expect("set permission profile");
        });
    let mut builder = if let Some(requirements) = reviewer_requirements {
        builder.with_cloud_config_bundle(
            CloudConfigBundleFixture::loader_with_enterprise_requirement(requirements.to_owned()),
        )
    } else {
        builder
    };
    let test = builder.build_with_auto_env(&actor).await?;

    // The first benign tool must finish while discovery's retry remains unanswered.
    for body in [
        sse(vec![
            ev_function_call("first", "exec_command", r#"{"cmd":"printf first"}"#),
            ev_completed("first"),
        ]),
        sse(vec![
            ev_assistant_message("done-first", "Done."),
            ev_completed("done-first"),
        ]),
    ] {
        responses::mount_sse_once(&actor, body).await;
    }
    if !should_recover {
        test.submit_text_turn("Print first.").await?;
        drop(release);
        test.codex.shutdown_and_wait().await?;
        assert!(
            catalog
                .received_requests()
                .await
                .expect("catalog requests")
                .is_empty()
        );
        assert!(scorer.requests().is_empty());
        return Ok(());
    }
    let (turn, retry) = tokio::join!(
        timeout(
            Duration::from_secs(10),
            test.submit_text_turn("Print first.")
        ),
        timeout(Duration::from_secs(10), started.notified()),
    );
    turn.context("tool dispatch waited for /models")??;
    retry.context("Guardian never started recovery")?;
    assert!(
        scorer.requests().is_empty(),
        "unavailable catalog must not start classification"
    );
    drop(release);
    let inference = test
        .codex
        .thread_extension_data()
        .get::<codex_core::GuardianInferenceContext>()
        .expect("Guardian inference");
    inference
        .models_manager
        .ensure_initialized(inference.http_client_factory.clone())
        .await;

    for body in [
        sse(vec![
            ev_function_call("second", "exec_command", r#"{"cmd":"printf second"}"#),
            ev_completed("second"),
        ]),
        sse(vec![
            ev_assistant_message("done-second", "Done."),
            ev_completed("done-second"),
        ]),
    ] {
        responses::mount_sse_once(&actor, body).await;
    }
    test.submit_text_turn("Print second.").await?;
    timeout(Duration::from_secs(10), async {
        while scorer.requests().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    assert_eq!(scorer.single_request().body_json()["model"], LUNA_MODEL);
    test.codex.shutdown_and_wait().await?;
    assert_eq!(
        catalog
            .received_requests()
            .await
            .expect("catalog requests")
            .len(),
        2,
        "a ready catalog must not be retried"
    );
    Ok(())
}

fn pin_guardian_parent_compaction_reuse(config: &mut Config) {
    config.features = ManagedFeatures::from_configured_with_warnings(
        config.features.get().clone(),
        Some(Sourced::new(
            FeatureRequirementsToml {
                entries: BTreeMap::from([(
                    Feature::GuardianReuseParentCompaction.key().to_owned(),
                    true,
                )]),
                ..Default::default()
            },
            RequirementSource::EnterpriseManaged {
                id: "test".to_owned(),
                name: "test".to_owned(),
            },
        )),
        &mut Vec::new(),
    )
    .expect("pin parent compaction reuse");
}

fn configure_explicit_guardian_provider(config: &mut Config, guardian_url: &str) {
    let mut provider = config.model_provider.clone();
    provider.base_url = Some(guardian_url.to_owned());
    provider.model_catalog_url = Some(format!("{guardian_url}/models").into());
    provider.supports_websockets = false;
    provider.http_headers = None;
    config.guardian_model_provider = Some(Arc::new(GuardianInferenceProvider {
        id: "responsesapi".to_owned(),
        info: provider,
    }));
}

#[test_case(BYO_ACTOR_MODEL, None, false; "different_model_names_at_start")]
#[test_case(LUNA_MODEL, Some(11_000), true; "same_model_name_after_start_and_cold_resume")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_guardian_provider_is_separate(
    actor_model_name: &'static str,
    actor_context_window: Option<i64>,
    cold_resume: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let actor = responses::start_mock_server().await;
    let guardian = MockServer::start().await;
    let guardian_url = format!("{}/v1", guardian.uri());
    let chatgpt = actor_model_name != LUNA_MODEL;
    let reviewer_model_name = if chatgpt {
        "codex-auto-review"
    } else {
        LUNA_MODEL
    };
    let guardian_auth = if chatgpt {
        "Bearer Access Token"
    } else {
        "Bearer dummy"
    };
    let guardian_threshold = if chatgpt { 0 } else { 5_000 };
    let mut reviewer_model = codex_models_manager::bundled_models_response()?
        .models
        .into_iter()
        .find(|model| model.slug == LUNA_MODEL)
        .expect("bundled Luna model");
    reviewer_model.guardian = Some(GuardianModelPolicy {
        permissions: Some(GuardianReviewMode::Adaptive),
        ..Default::default()
    });
    reviewer_model.supported_in_api = true;
    reviewer_model.use_responses_lite = false;
    reviewer_model.input_modalities = vec![InputModality::Text, InputModality::Image];
    let messages = reviewer_model.model_messages.get_or_insert_default();
    messages.auto_review = Some(
        serde_json::from_value(json!({"policy": "Trusted Guardian catalog policy."}))
            .expect("catalog policy"),
    );
    messages.guardian_v2 = Some(GuardianV2ModelConfig {
        classifier_instructions: Some("Trusted Guardian classifier instructions.".to_owned()),
        reasoning_effort: Some(ReasoningEffort::Minimal),
        review_threshold_basis_points: Some(guardian_threshold),
        ..Default::default()
    });
    let mut models = vec![reviewer_model.clone()];
    if chatgpt {
        models[0]
            .model_messages
            .as_mut()
            .expect("classifier messages")
            .auto_review
            .as_mut()
            .expect("classifier policy")
            .policy = Some("Classifier-only policy that must not authorize actions.".to_owned());
        reviewer_model.slug = reviewer_model_name.to_owned();
        models.push(reviewer_model);
    }
    let catalog = ModelsResponse { models };
    responses::mount_models_once(&guardian, catalog.clone()).await;

    let builder = |guardian_url: &str| {
        let guardian_url = guardian_url.to_owned();
        test_codex()
            .with_pre_build_hook(move |home| {
                let actor_config = r#"[auto_review]
policy = "Actor config policy."

[features]
guardian_enhanced_node_repl_transcripts = true
guardian_node_repl_transcript_images = true

[features.guardianv2]
enabled = true
classifier_instructions = "Actor config classifier instructions."
reasoning_effort = "high"
review_threshold = ACTOR_THRESHOLD
"#
                .replace(
                    "ACTOR_THRESHOLD",
                    if guardian_threshold == 0 {
                        "0.5"
                    } else {
                        "0.0"
                    },
                );
                std::fs::write(home.join("config.toml"), actor_config)
                    .expect("write Guardian configuration");
            })
            .with_model_info_override(actor_model_name, move |actor_model| {
                actor_model.input_modalities = vec![InputModality::Text];
                actor_model.guardian = Some(GuardianModelPolicy::default());
                let messages = actor_model.model_messages.get_or_insert_default();
                messages.auto_review = Some(
                    serde_json::from_value(json!({"policy": "Actor catalog policy."}))
                        .expect("catalog policy"),
                );
                messages.guardian_v2 = Some(GuardianV2ModelConfig {
                    classifier_instructions: Some(
                        "Actor catalog classifier instructions.".to_owned(),
                    ),
                    reasoning_effort: Some(ReasoningEffort::High),
                    review_threshold_basis_points: Some(5_000 - guardian_threshold),
                    ..Default::default()
                });
            })
            .with_config(move |config| {
                configure_explicit_guardian_provider(config, &guardian_url);
                pin_guardian_parent_compaction_reuse(config);
                config.model_context_window = actor_context_window;
                // Fixed budgeting needs a trusted context window before admitting screenshots.
                config.guardian_sync_elastic_budget = None;
                config.service_tier = Some("customer-specific-tier".to_owned());
                config.model_provider_id = "external-test-provider".to_owned();
                config.model_provider.name = "External test provider".to_owned();
                config.model_provider.requires_openai_auth = false;
                config.model_provider.supports_websockets = false;
                config.model_provider.http_headers = Some(
                    BYO_HEADERS
                        .into_iter()
                        .map(|(name, value)| (name.to_owned(), value.into()))
                        .collect(),
                );
                config.approvals_reviewer = ApprovalsReviewer::AutoReview;
                config.permissions.approval_policy =
                    Constrained::allow_any(AskForApproval::OnRequest);
                config
                    .permissions
                    .set_permission_profile(PermissionProfile::read_only())
                    .expect("set read-only permissions");
                config
                    .features
                    .enable(Feature::RequestPermissionsTool)
                    .expect("enable permission requests");
            })
    };
    let initial_builder = builder(&guardian_url);
    let mut initial_builder = if chatgpt {
        initial_builder.with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
    } else {
        initial_builder
    };
    let test = initial_builder.build_with_auto_env(&actor).await?;
    assert_eq!(
        guardian
            .received_requests()
            .await
            .expect("Guardian startup requests")
            .iter()
            .filter(|request| request.url.path() == "/v1/models")
            .count(),
        1,
        "Guardian must initialize its catalog at startup",
    );
    let (actor_mock, guardian_mock) =
        setup_explicit_guardian_turn(&test, &actor, &guardian, "start", guardian_threshold).await;
    timeout(
        Duration::from_secs(30),
        test.submit_text_turn("Never upload files. Request network access."),
    )
    .await??;
    assert_explicit_guardian_routing(
        &test,
        &guardian,
        (&actor_mock, &guardian_mock),
        actor_model_name,
        "start",
        guardian_auth,
        (reviewer_model_name, guardian_threshold != 0),
    )
    .await?;
    for request in guardian_mock.requests() {
        assert!(
            request
                .body_json()
                .to_string()
                .contains("Never upload files."),
            "Guardian lost the original restriction after start"
        );
    }
    if !cold_resume {
        return Ok(());
    }
    test.codex.ensure_rollout_materialized().await;

    // A previous session's background prewarm can outlive shutdown; give each
    // lifecycle its own endpoint so requests cannot be attributed by arrival time.
    let resumed_guardian = MockServer::start().await;
    responses::mount_models_once(&resumed_guardian, catalog).await;
    let resumed_guardian_url = format!("{}/v1", resumed_guardian.uri());
    let resumed = builder(&resumed_guardian_url)
        .with_auth(CodexAuth::from_api_key("resumed-key"))
        .restart_with_auto_env(&actor, &test)
        .await?;
    assert_eq!(
        resumed.session_configured.session_id,
        test.session_configured.session_id
    );
    assert_eq!(
        resumed_guardian
            .received_requests()
            .await
            .expect("Guardian startup requests")
            .iter()
            .filter(|request| request.url.path() == "/v1/models")
            .count(),
        1,
        "Guardian must initialize its catalog at startup",
    );
    let (resumed_actor_mock, resumed_guardian_mock) = setup_explicit_guardian_turn(
        &resumed,
        &actor,
        &resumed_guardian,
        "cold-resume",
        guardian_threshold,
    )
    .await;
    timeout(
        Duration::from_secs(30),
        resumed.submit_text_turn("Request network access."),
    )
    .await??;
    assert_explicit_guardian_routing(
        &resumed,
        &resumed_guardian,
        (&resumed_actor_mock, &resumed_guardian_mock),
        actor_model_name,
        "cold-resume",
        "Bearer resumed-key",
        (reviewer_model_name, guardian_threshold != 0),
    )
    .await?;
    for request in resumed_guardian_mock.requests() {
        assert!(
            request
                .body_json()
                .to_string()
                .contains("Never upload files."),
            "Guardian lost the original restriction after cold-resume"
        );
    }
    Ok(())
}

async fn setup_explicit_guardian_turn(
    test: &TestCodex,
    actor: &MockServer,
    guardian: &MockServer,
    phase: &str,
    guardian_threshold: u16,
) -> (responses::ResponseMock, responses::ResponseMock) {
    let call_id = format!("{phase}-permission");
    let done_id = format!("{phase}-done");
    let permission_args = json!({
        "reason": format!("request network access after {phase}"),
        "permissions": {"network": {"enabled": true}},
    })
    .to_string();
    let mut permission_call = ev_function_call(&call_id, "request_permissions", &permission_args);
    permission_call["item"]["id"] = json!(&call_id);
    let actor_mock = responses::mount_sse_sequence(
        actor,
        vec![
            sse(vec![
                ev_response_created(&call_id),
                json!({
                    "type": "response.output_item.done",
                    "item": {
                        "type": "compaction",
                        "id": format!("cmp_{phase}"),
                        "encrypted_content": "actor-only-checkpoint",
                    },
                }),
                json!({
                    "type": "response.metadata",
                    "response_id": &call_id,
                    "metadata": {
                        "guardian": {"item_id": &call_id, "requires_review": false}
                    },
                }),
                permission_call,
                ev_completed(&call_id),
            ]),
            sse(vec![
                ev_response_created(&done_id),
                ev_assistant_message(&done_id, "Finished."),
                ev_completed(&done_id),
            ]),
        ],
    )
    .await;
    let guardian_mock = responses::mount_response_handler(guardian, move |request| {
        let body = request.body_json();
        if body["text"]["format"]["type"] == "json_schema" {
            let response = responses::sse_response(sse(vec![
                ev_response_created("reviewer"),
                ev_assistant_message("reviewer", r#"{"risk_level":"high","user_authorization":"low","outcome":"deny","rationale":"Test review."}"#),
                ev_completed("reviewer"),
            ]));
            if guardian_threshold == 0 {
                response
            } else {
                // Keep the reviewer pending beyond the turn timeout so only a LOW score can approve.
                response.set_delay(Duration::from_secs(60))
            }
        } else {
            responses::sse_response(sse(vec![
                ev_response_created("classifier"),
                ev_assistant_message("classifier", "low"),
                ev_completed("classifier"),
            ]))
        }
    })
    .await;

    test.codex
        .thread_extension_data()
        .get_or_init(NodeReplReviewEvidence::default)
        .record(
            "cua_repl.js",
            phase,
            &call_id,
            vec![UserInput::Image {
                image: ImageReference::Inline {
                    image_url: GUARDIAN_SCREENSHOT.to_owned(),
                },
                detail: None,
            }],
        );
    (actor_mock, guardian_mock)
}

async fn assert_explicit_guardian_routing(
    test: &TestCodex,
    guardian: &MockServer,
    (actor_mock, guardian_mock): (&responses::ResponseMock, &responses::ResponseMock),
    actor_model_name: &str,
    phase: &str,
    guardian_auth: &str,
    (reviewer_model_name, fast_approval): (&str, bool),
) -> Result<()> {
    let call_id = format!("{phase}-permission");
    timeout(Duration::from_secs(10), async {
        loop {
            let requests = guardian_mock.requests();
            let has_classifier = requests
                .iter()
                .any(|request| request.body_json()["text"]["format"]["type"] != "json_schema");
            let has_reviewer = requests
                .iter()
                .any(|request| request.body_json()["text"]["format"]["type"] == "json_schema");
            if has_classifier && (fast_approval || has_reviewer) {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await?;
    let snapshot = test.codex.conversation_history_snapshot().await;
    assert!(snapshot.uses_independent_review_history());
    let checkpoint = snapshot
        .latest_compaction()
        .context("actor stream checkpoint was not recorded")?;
    assert!(matches!(
        checkpoint.item,
        ResponseItem::Compaction { encrypted_content, .. }
            if encrypted_content == "actor-only-checkpoint"
    ));
    let actor_requests = actor_mock.requests();
    let followup = actor_requests.last().context("actor followup")?;
    let output = followup
        .function_call_output_text(&call_id)
        .context("permission result")?;
    let permission: RequestPermissionsResponse = serde_json::from_str(&output)?;
    assert_eq!(
        permission.permissions.network,
        fast_approval.then_some(NetworkPermissions {
            enabled: Some(true)
        }),
        "the Guardian threshold must control the actual approval after {phase}",
    );
    for request in &actor_requests {
        let body = request.body_json();
        assert_eq!(body["model"], actor_model_name);
        assert!(!body.to_string().contains("Trusted Guardian"));
        for (name, value) in BYO_HEADERS {
            assert_eq!(request.header(name).as_deref(), Some(value));
        }
    }
    let guardian_requests = guardian
        .received_requests()
        .await
        .expect("record Guardian requests");
    assert_guardian_headers(&guardian_requests, guardian_auth);
    assert_eq!(
        guardian_requests
            .iter()
            .filter(|request| request.url.path() == "/v1/models")
            .count(),
        1,
        "catalog should be fetched once after {phase}"
    );
    for request in guardian_mock.requests() {
        let body = request.body_json();
        let serialized = body.to_string();
        assert!(
            body["service_tier"].is_null(),
            "Guardian inherited the actor tier after {phase}"
        );
        assert!(
            serialized.contains("Trusted Guardian catalog policy."),
            "{serialized}"
        );
        assert!(
            !serialized.contains("Actor config")
                && !serialized.contains("Actor catalog")
                && !serialized.contains("Classifier-only policy"),
            "{serialized}"
        );
        if body["text"]["format"]["type"] == "json_schema" {
            assert_eq!(body["model"], reviewer_model_name);
            if !fast_approval {
                assert_eq!(
                    request.message_input_image_urls("user"),
                    [GUARDIAN_SCREENSHOT],
                    "Guardian image support after {phase}"
                );
            }
        } else {
            assert_eq!(body["model"], LUNA_MODEL);
            assert!(
                serialized.contains("Trusted Guardian classifier instructions."),
                "{serialized}"
            );
            assert_eq!(body["reasoning"]["effort"], "minimal");
        }
        assert!(
            body["input"]
                .as_array()
                .expect("Guardian input")
                .iter()
                .all(|item| !matches!(
                    item["type"].as_str(),
                    Some("compaction" | "context_compaction")
                )),
            "Guardian reused an actor checkpoint after {phase}"
        );
    }
    Ok(())
}

fn assert_guardian_headers(requests: &[Request], expected_auth: &str) {
    for request in requests {
        assert!(!request.headers.contains_key("x-byo-key"));
        assert!(
            request.headers["authorization"] == expected_auth,
            "incorrect Guardian authorization for {} {}",
            request.method,
            request.url.path(),
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delayed_score_only_covers_the_tool_call_it_classified() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let permission_args = json!({
        "reason": "request network access",
        "permissions": { "network": { "enabled": true } },
    })
    .to_string();
    let mut parent_gates = Vec::new();
    let mut classifier_gates = Vec::new();
    let mut parent_responses = Vec::new();
    let mut classifier_responses = Vec::new();
    for (call_id, tool, args) in [
        (
            "before-publication",
            "request_permissions",
            permission_args.clone(),
        ),
        (
            "intervening-call",
            "update_plan",
            json!({ "plan": [{ "step": "Inspect permissions", "status": "in_progress" }] })
                .to_string(),
        ),
        (
            "stale-publication",
            "request_permissions",
            permission_args.clone(),
        ),
        ("fresh-publication", "request_permissions", permission_args),
    ] {
        let (parent_tx, parent_rx) = oneshot::channel();
        parent_gates.push(parent_tx);
        parent_responses.push(vec![StreamingSseChunk {
            gate: Some(parent_rx),
            body: sse(vec![
                ev_response_created(call_id),
                ev_function_call(call_id, tool, &args),
                ev_completed(call_id),
            ]),
        }]);
        let (classifier_tx, classifier_rx) = oneshot::channel();
        classifier_gates.push(classifier_tx);
        classifier_responses.push(vec![StreamingSseChunk {
            gate: Some(classifier_rx),
            body: sse(vec![
                ev_response_created(call_id),
                ev_assistant_message(call_id, "low"),
                ev_completed(call_id),
            ]),
        }]);
    }
    let (parent_server, _) = start_streaming_sse_server(parent_responses).await;
    let (classifier_server, _) = start_streaming_sse_server(classifier_responses).await;

    // Route the concurrent parent and classifier requests to independent gated streams.
    for (model, destination) in [
        ("guardian-publication-parent", parent_server.uri()),
        ("gpt-5.6-luna", classifier_server.uri()),
    ] {
        Mock::given(method("POST"))
            .and(path("/v1/responses"))
            .and(body_partial_json(json!({ "model": model })))
            .respond_with(
                ResponseTemplate::new(/*s*/ 307)
                    .insert_header("location", format!("{destination}/v1/responses")),
            )
            .with_priority(/*p*/ 1)
            .up_to_n_times(/*n*/ 4)
            .mount(&server)
            .await;
    }
    let final_response = responses::mount_sse_once_match(
        &server,
        body_partial_json(json!({ "model": "guardian-publication-parent" })),
        sse(vec![
            ev_response_created("done"),
            ev_assistant_message("done", "done"),
            ev_completed("done"),
        ]),
    )
    .await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({ "model": "gpt-5.5" })))
        .respond_with(responses::sse_response(sse(vec![
            ev_assistant_message("invalid", "invalid assessment"),
            ev_completed("invalid"),
        ])))
        .with_priority(1)
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(body_partial_json(json!({
            "model": "gpt-5.5",
            "client_metadata": { "x-openai-subagent": "guardian" },
        })))
        .respond_with(responses::sse_response(sse(vec![
            ev_response_created("review"),
            ev_assistant_message(
                "review",
                &json!({
                    "risk_level": "high",
                    "user_authorization": "low",
                    "outcome": "deny",
                    "rationale": "Network access is not authorized.",
                })
                .to_string(),
            ),
            ev_completed("review"),
        ])))
        .with_priority(/*p*/ 2)
        .expect(/*r*/ 2)
        .mount(&server)
        .await;

    let mut test = test_codex()
        .with_model("guardian-initial-model")
        .with_pre_build_hook(|home| {
            std::fs::write(
                home.join("config.toml"),
                "[features.guardianv2]\nenabled = true\npersist_scores = true\nmax_tool_call_lag = 1\n\n[features.guardianv2.review_scope]\ncomputer_use_only = false\n",
            )
            .expect("write Guardian configuration");
        })
        .with_model_info_override("guardian-publication-parent", |model| {
            model.guardian = None;
            model.auto_review_model_override = Some("gpt-5.5".to_owned());
        })
        .with_config(|config| {
            config.update_plan_enabled = true;
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config
                .permissions
                .set_permission_profile(PermissionProfile::read_only())
                .expect("set read-only permissions");
            config
                .features
                .enable(Feature::RequestPermissionsTool)
                .expect("enable permission requests");
        })
        .build_with_auto_env(&server)
        .await?;
    let metrics = MetricsClient::new(
        MetricsConfig::in_memory("test", "test", "1", Default::default()).with_runtime_reader(),
    )?;
    let mut options = test.start_thread_options().await;
    options.history_mode = Some(codex_protocol::protocol::ThreadHistoryMode::Legacy);
    options.thread_extension_init.insert(metrics.clone());
    test.codex.shutdown_and_wait().await?;
    test.codex = test.thread_manager.start_thread(options).await?.thread;
    test.codex.ensure_rollout_materialized().await;
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Inspect the available permissions.".to_owned(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                model: Some("guardian-publication-parent".into()),
                ..Default::default()
            }),
        )
        .await?;

    let mut parent_gates = parent_gates.into_iter();
    let mut classifier_gates = classifier_gates.into_iter();
    parent_gates.next().unwrap().send(()).unwrap();
    timeout(Duration::from_secs(30), async {
        classifier_server.wait_for_request_count(/*count*/ 1).await;
        parent_server.wait_for_request_count(/*count*/ 2).await;
    })
    .await?;
    parent_gates.next().unwrap().send(()).unwrap();
    timeout(Duration::from_secs(30), async {
        classifier_server.wait_for_request_count(/*count*/ 2).await;
        parent_server.wait_for_request_count(/*count*/ 3).await;
    })
    .await?;

    // Publishing call 1 after call 2 starts must not claim coverage through call 2.
    // At call 3 it is two calls behind, exceeding the configured lag of one.
    let first_score = classifier_gates.next().unwrap();
    let _held_intervening_score = classifier_gates.next().unwrap();
    let current_score = classifier_gates.next().unwrap();
    let _held_final_score = classifier_gates.next().unwrap();
    let publish_score = async |call_id: &str, release_score: oneshot::Sender<()>| -> Result<()> {
        release_score.send(()).unwrap();
        // Persisted rollout scores are emitted only after publication. Observe that
        // public result instead of reading or modifying the private score state.
        timeout(Duration::from_secs(30), async {
            loop {
                let history = test.codex.load_history(/*include_archived*/ false).await?;
                if history.items.into_iter().any(
                    |item| matches!(item, RolloutItem::SecurityRiskScore(score) if score.call_id.as_deref() == Some(call_id)),
                ) {
                    return Ok::<_, anyhow::Error>(());
                }
                tokio::task::yield_now().await;
            }
        })
        .await??;
        Ok(())
    };
    publish_score("before-publication", first_score).await?;
    parent_gates.next().unwrap().send(()).unwrap();
    timeout(Duration::from_secs(30), async {
        classifier_server.wait_for_request_count(/*count*/ 3).await;
        parent_server.wait_for_request_count(/*count*/ 4).await;
    })
    .await?;

    // Call 3's result is now fresh enough for call 4, whose own score is still held.
    publish_score("stale-publication", current_score).await?;
    parent_gates.next().unwrap().send(()).unwrap();

    let mut review_reasons = Vec::new();
    loop {
        match wait_for_event(&test.codex, |_| true).await {
            EventMsg::GuardianAssessment(event)
                if event.status == GuardianAssessmentStatus::Denied =>
            {
                review_reasons.push(event.review_reason);
            }
            EventMsg::RequestPermissions(event) => panic!("unexpected user prompt: {event:?}"),
            EventMsg::TurnComplete(_) => break,
            _ => {}
        }
    }
    assert_eq!(
        review_reasons,
        vec![
            Some(GuardianReviewReason::MissingScore),
            Some(GuardianReviewReason::StaleScore)
        ],
    );
    let request = final_response.single_request();
    for (call_id, network) in [
        ("before-publication", None),
        ("stale-publication", None),
        (
            "fresh-publication",
            Some(NetworkPermissions {
                enabled: Some(true),
            }),
        ),
    ] {
        let output = request
            .function_call_output_text(call_id)
            .expect("permission result");
        assert_eq!(
            serde_json::from_str::<RequestPermissionsResponse>(&output)?,
            RequestPermissionsResponse {
                permissions: RequestPermissionProfile {
                    network,
                    file_system: None
                },
                scope: PermissionGrantScope::Turn,
                strict_auto_review: false,
            },
            "{call_id}",
        );
    }

    test.codex.shutdown_and_wait().await?;
    let snapshot = metrics.snapshot()?;
    let route = snapshot
        .scope_metrics()
        .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
        .find(|metric| metric.name() == "codex.guardian.review.route")
        .unwrap();
    let AggregatedMetrics::U64(MetricData::Sum(routes)) = route.data() else {
        panic!("expected route counter");
    };
    for point in routes.data_points() {
        let model = point
            .attributes()
            .find(|tag| tag.key.as_str() == "model")
            .unwrap();
        assert_eq!(model.value.as_str(), "guardian-publication-parent");
    }
    let metric = snapshot
        .scope_metrics()
        .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
        .find(|metric| metric.name() == "codex.guardian.review.retry_wait_ms")
        .unwrap();
    let AggregatedMetrics::F64(MetricData::Histogram(histogram)) = metric.data() else {
        panic!("expected retry-wait histogram");
    };
    let points = histogram.data_points().collect::<Vec<_>>();
    assert_eq!(points.len(), 1);
    let point = points[0];
    let tags = point
        .attributes()
        .map(|tag| (tag.key.as_str(), tag.value.as_str().to_string()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(tags["model"], "guardian-publication-parent");
    assert_eq!(tags["failure_reason"], "parse_error");
    assert_eq!(tags["outcome"], "retry");
    assert_eq!(point.count(), 1);
    assert!(point.sum() > 0.0);
    parent_server.shutdown().await;
    classifier_server.shutdown().await;
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProbeCase {
    Standard,
    Escalated,
    Rewrite,
    Interrupted,
    CatalogPolicy,
    IncompleteAuthorization,
}

#[test_case::test_case(Some(false), false, "probe-item", ProbeCase::Standard, 1; "direct_low")]
#[test_case::test_case(Some(true), false, "probe-item", ProbeCase::Standard, 1; "direct_review")]
#[test_case::test_case(Some(false), true, "probe-item", ProbeCase::Standard, 2; "code_cell_low")]
#[test_case::test_case(Some(true), true, "probe-item", ProbeCase::Standard, 2; "code_cell_review")]
#[test_case::test_case(None, false, "probe-item", ProbeCase::Standard, 1; "missing_uses_async")]
#[test_case::test_case(Some(false), false, "other-item", ProbeCase::Standard, 1; "mismatched_uses_async")]
#[test_case::test_case(Some(false), false, "probe-item", ProbeCase::Escalated, 1; "explicit_escalation_still_reviews")]
#[test_case::test_case(Some(false), false, "probe-item", ProbeCase::Rewrite, 1; "direct_hook_rewrite")]
#[test_case::test_case(Some(false), true, "probe-item", ProbeCase::Rewrite, 2; "nested_hook_rewrite")]
#[test_case::test_case(Some(true), false, "probe-item", ProbeCase::Interrupted, 0; "interrupted_direct_review")]
#[test_case::test_case(Some(true), true, "probe-item", ProbeCase::Interrupted, 0; "interrupted_cell_review")]
#[test_case::test_case(Some(false), false, "probe-item", ProbeCase::CatalogPolicy, 1; "catalog_low_without_legacy_flag")]
#[test_case::test_case(Some(true), false, "probe-item", ProbeCase::CatalogPolicy, 1; "catalog_review_without_legacy_flag")]
#[test_case::test_case(Some(false), false, "probe-item", ProbeCase::IncompleteAuthorization, 1; "incomplete_authorization_still_reviews")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn probe_flags_keep_async_scoring(
    requires_review: Option<bool>,
    code_mode: bool,
    assessed_item: &str,
    scenario: ProbeCase,
    expected_reviews: u64,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_target_windows!(Ok(()), "exec_command requires a POSIX executor");
    core_test_support::skip_if_test_condition!(
        Ok(()),
        cfg!(target_os = "windows")
            && matches!(scenario, ProbeCase::Rewrite | ProbeCase::Interrupted),
        "a Windows host",
        "hook fixtures require a POSIX host",
    );
    let server = responses::start_mock_server().await;
    let mut arguments = json!({
        "cmd": "echo probe-policy", "login": false
    });
    if scenario == ProbeCase::Escalated {
        arguments["sandbox_permissions"] = json!("require_escalated");
        arguments["justification"] = json!("Run the requested command outside the sandbox.");
    }
    let mut tool = if code_mode {
        responses::ev_custom_tool_call(
            "call",
            "exec",
            &format!(
                "const first = await tools.exec_command({arguments}); \
             const second = await tools.exec_command({second_arguments}); \
             if (first.exit_code !== 0 || second.exit_code !== 0) throw new Error('command failed'); \
             text(first.output); text(second.output);",
                second_arguments = json!({"cmd":"echo probe-policy-second", "login":false})
            ),
        )
    } else {
        ev_function_call("call", "exec_command", &arguments.to_string())
    };
    tool["item"]["id"] = json!("probe-item");
    let mut events = vec![ev_response_created("probe-response")];
    if let Some(requires_review) = requires_review {
        events.push(json!({
            "type": "response.metadata", "response_id": "probe-response",
            "metadata": {"guardian": {"item_id": assessed_item, "requires_review": requires_review}}
        }));
    }
    events.push(tool);
    if scenario != ProbeCase::Interrupted {
        events.push(ev_completed("probe-response"));
    }
    let mut response = sse(events);
    if scenario == ProbeCase::Interrupted {
        response.push_str(&responses::sse_failed(
            "probe-response",
            "server_error",
            "response interrupted",
        ));
    }
    let parent = responses::mount_sse_sequence(
        &server,
        vec![
            response,
            sse(vec![
                ev_assistant_message("done", "done"),
                ev_completed("done"),
            ]),
        ],
    )
    .await;
    let expected_scores = if scenario == ProbeCase::Interrupted {
        0..=0
    } else {
        // The enclosing code wrapper also remains on the async path.
        1..=if code_mode { 3 } else { 1 }
    };
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"model": "gpt-5.6-luna"})))
        .respond_with(responses::sse_response(sse(vec![
            ev_assistant_message(
                "score",
                if scenario == ProbeCase::Interrupted {
                    "low"
                } else {
                    "high"
                },
            ),
            ev_completed("score"),
        ])))
        .with_priority(/*p*/ 1)
        .expect(expected_scores)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"client_metadata": {"x-openai-subagent": "guardian"}})))
        .and(body_partial_json(json!({"model": "gpt-5.5"})))
        .respond_with(responses::sse_response(sse(vec![
            ev_assistant_message("review", r#"{"risk_level":"low","user_authorization":"high","outcome":"allow","rationale":"requested command"}"#),
            ev_completed("review"),
        ])))
        .with_priority(/*p*/ 1)
        .expect(expected_reviews)
        .mount(&server).await;
    let mut builder = test_codex()
        .with_model("guardian-probe-parent")
        .with_pre_build_hook(move |home| {
            #[cfg(not(target_os = "windows"))]
            match scenario {
                ProbeCase::Rewrite => super::hooks::write_updating_pre_tool_use_hook(
                    home, "^Bash$", &json!({"command": "echo probe-policy-second-rewritten"}),
                ).expect("write input rewrite hook"),
                ProbeCase::Interrupted => super::hooks::write_lifecycle_hooks(
                    home, &["PostModelRequest"],
                ).expect("write synchronous post-request hook"),
                ProbeCase::Standard | ProbeCase::Escalated
                | ProbeCase::CatalogPolicy | ProbeCase::IncompleteAuthorization => {}
            }
            let rules = home.join("rules");
            std::fs::create_dir_all(&rules).expect("create rules directory");
            std::fs::write(rules.join("default.rules"), r#"prefix_rule(pattern=["echo"], decision="prompt")"#)
                .expect("write prompt rule");
            std::fs::write(home.join("config.toml"),
                "[features.guardianv2]\nenabled = true\npersist_scores = true\n[features.guardianv2.review_scope]\ncomputer_use_only = false\nsandboxed_exec_commands = true\n"
            ).expect("write Guardian configuration");
        })
        .with_model_info_override("guardian-probe-parent", move |model| {
            model.guardian = (scenario == ProbeCase::CatalogPolicy).then(|| {
                serde_json::from_value(json!({
                    "shell": "adaptive", "sandboxed_exec_commands": true
                })).expect("catalog Guardian policy")
            });
            model.auto_review_model_override = Some("gpt-5.5".into());
        })
        .with_config(move |config| {
            if scenario == ProbeCase::Interrupted {
                config.response_process_mode = codex_config::config_toml::ResponseProcessMode::AfterCompleteResponse;
            }
            if matches!(scenario, ProbeCase::Rewrite | ProbeCase::Interrupted) {
                trust_discovered_hooks(config);
            }
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.permissions.set_permission_profile(PermissionProfile::read_only()).expect("read-only permissions");
            config.features.set_enabled(Feature::CodeMode, code_mode).expect("configure Code Mode");
        });
    let mut test = builder.build_with_auto_env(&server).await?;
    if scenario == ProbeCase::CatalogPolicy {
        // The fixture installs the extension under the legacy flag. Production always installs it.
        let mut options = test.start_thread_options().await;
        options
            .config
            .features
            .disable(Feature::GuardianV2)
            .expect("disable legacy scorer flag");
        test.codex.shutdown_and_wait().await?;
        test.codex = test.thread_manager.start_thread(options).await?.thread;
    }
    if scenario == ProbeCase::IncompleteAuthorization {
        // Restore enough verified answers to evict an original before the serving request.
        let mut options = test.start_thread_options().await;
        options.history_mode = Some(codex_protocol::protocol::ThreadHistoryMode::Legacy);
        options.initial_history = InitialHistory::Resumed(ResumedHistory {
            history_revision: None,
            conversation_id: test.codex.startup_metadata().thread_id,
            history: Arc::new(
                (0..9)
                    .map(|index| {
                        RolloutItem::RetainedContext(RetainedContextEvent::VerifiedAnswer {
                            answer: VerifiedAnswer {
                                turn_id: "previous-turn".into(),
                                call_id: format!("answer-{index}"),
                                questions: vec![VerifiedQuestionAnswer {
                                    question: "Run the command?".into(),
                                    answer: "Yes".into(),
                                }],
                            },
                            acceptance_order: Some(index),
                        })
                    })
                    .collect(),
            ),
            rollout_path: None,
        });
        test.codex.ensure_rollout_materialized().await;
        test.codex.shutdown_and_wait().await?;
        test.thread_manager
            .remove_thread(&test.codex.startup_metadata().thread_id)
            .await;
        test.codex = test.thread_manager.start_thread(options).await?.thread;
        assert!(
            !test
                .codex
                .conversation_history_snapshot()
                .await
                .retained_context()
                .expect("restored authorization")
                .verified_answers_complete()
        );
    }
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Run the requested echo commands.".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    if scenario != ProbeCase::Interrupted {
        timeout(Duration::from_secs(30), async {
            loop {
                if server
                    .received_requests()
                    .await
                    .unwrap_or_default()
                    .iter()
                    .any(|request| {
                        request
                            .body_json::<serde_json::Value>()
                            .is_ok_and(|body| body["model"] == "gpt-5.6-luna")
                    })
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await?;
    }
    test.codex.shutdown_and_wait().await?;
    let requests = parent.requests();
    assert_eq!(requests.len(), 2);
    if scenario == ProbeCase::Interrupted {
        // Buffered processing holds partial output. A failed stream must
        // not expose the tool call to Guardian or carry a made-up reply into the retry.
        let retry = requests[1].body_json();
        assert!(
            retry["input"]
                .as_array()
                .expect("request input")
                .iter()
                .all(|item| item["call_id"] != "call")
        );
        return Ok(());
    }
    let output = requests[1].call_output(
        "call",
        if code_mode {
            "custom_tool_call_output"
        } else {
            "function_call_output"
        },
    );
    let output = output["output"].to_string();
    if scenario == ProbeCase::Rewrite {
        assert!(
            output.contains("probe-policy-second-rewritten"),
            "rewritten command ran: {output}"
        );
    }
    if code_mode {
        assert!(
            output.contains("probe-policy-second"),
            "both nested commands ran: {output}"
        );
    } else {
        assert!(
            output.contains("Process exited with code 0"),
            "the direct command succeeded: {output}"
        );
    }
    assert!(
        output.contains("probe-policy"),
        "the assessed command ran: {output}"
    );
    Ok(())
}
