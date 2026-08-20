use codex_core::TurnInputRequest;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_core::config::Constrained;
use codex_extension_api::ApprovalReviewContributor;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionMetrics;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_models_manager::manager::RefreshStrategy;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::ApplyPatchToolType;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::openai_models::default_input_modalities;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::request_permissions::PermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionProfile;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_protocol::user_input::UserInput;
use core_test_support::TempDirExt;
use core_test_support::responses::ev_apply_patch_custom_tool_call;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_sandbox;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_with_timeout;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

struct ApprovedReviewContributor;

impl ApprovalReviewContributor for ApprovedReviewContributor {
    fn contribute<'a>(
        &'a self,
        _session_store: &'a ExtensionData,
        _thread_store: &'a ExtensionData,
        prompt: &'a str,
        extension_metrics: Option<Arc<dyn ExtensionMetrics>>,
    ) -> ExtensionFuture<'a, Option<ReviewDecision>> {
        Box::pin(async move {
            assert!(extension_metrics.is_some());
            assert!(prompt.contains("\"tool\":\"request_permissions\""));
            Some(ReviewDecision::Approved)
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn approval_review_contributor_skips_existing_guardian_model_call() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(Ok(()), "request_permissions requires host-native paths");

    let server = MockServer::start().await;
    let permissions_call_id = "extension-approved-permissions";
    let permissions_args = json!({
        "reason": "grant low-risk network access",
        "permissions": {
            "network": {
                "enabled": true,
            },
        },
    });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-parent-1"),
                ev_function_call(
                    permissions_call_id,
                    "request_permissions",
                    &serde_json::to_string(&permissions_args)?,
                ),
                ev_completed("resp-parent-1"),
            ]),
            sse(vec![
                ev_response_created("resp-parent-2"),
                ev_assistant_message("msg-parent", "done"),
                ev_completed("resp-parent-2"),
            ]),
        ],
    )
    .await;

    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.approval_review_contributor(Arc::new(ApprovedReviewContributor));
    let mut builder = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config
                .features
                .enable(Feature::RequestPermissionsTool)
                .expect("test config should allow feature update");
        });
    let test = builder.build_with_auto_env(&server).await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "request low-risk network access".into(),
            text_elements: Vec::new(),
        }]))
        .await?;

    loop {
        match wait_for_event(&test.codex, |_| true).await {
            EventMsg::GuardianAssessment(event) => {
                panic!("approved extension review should not start Guardian: {event:?}")
            }
            EventMsg::RequestPermissions(event) => {
                panic!("approved extension review should not prompt the user: {event:?}")
            }
            EventMsg::TurnComplete(_) => break,
            _ => {}
        }
    }

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[1]
            .function_call_output(permissions_call_id)
            .to_string()
            .contains("enabled")
    );

    Ok(())
}

#[derive(Clone, Copy)]
enum GuardianV2DisableSource {
    Config,
    ManagedRequirements,
}

#[test_case(GuardianV2DisableSource::Config; "disabled in config")]
#[test_case(GuardianV2DisableSource::ManagedRequirements; "disabled by managed policy")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn required_model_bypasses_extension_approval_when_guardian_v2_is_disabled(
    disable_source: GuardianV2DisableSource,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(Ok(()), "request_permissions requires host-native paths");

    let server = MockServer::start().await;
    let model = "gpt-5.4";
    let call_id = "required-model-permissions";
    let reason = "request network access under mandatory Guardian review";
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-required-model-parent"),
                ev_function_call(
                    call_id,
                    "request_permissions",
                    &json!({
                        "reason": reason,
                        "permissions": { "network": { "enabled": true } },
                    })
                    .to_string(),
                ),
                ev_completed("resp-required-model-parent"),
            ]),
            sse(vec![
                ev_response_created("resp-required-model-review"),
                ev_assistant_message(
                    "msg-required-model-review",
                    &json!({
                        "risk_level": "high",
                        "user_authorization": "low",
                        "outcome": "deny",
                        "rationale": "The requested network access is not authorized.",
                    })
                    .to_string(),
                ),
                ev_completed("resp-required-model-review"),
            ]),
            sse(vec![
                ev_response_created("resp-required-model-followup"),
                ev_assistant_message("msg-required-model-followup", "done"),
                ev_completed("resp-required-model-followup"),
            ]),
        ],
    )
    .await;

    let (guardian_v2_config, reviewer_requirements) = match disable_source {
        GuardianV2DisableSource::Config => ("false", ""),
        GuardianV2DisableSource::ManagedRequirements => {
            ("true", "allowed_approvals_reviewers = [\"auto_review\"]\n")
        }
    };
    let requirements =
        format!("{reviewer_requirements}[auto_review]\nrequired_on_models = [\"{model}\"]\n");
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.approval_review_contributor(Arc::new(ApprovedReviewContributor));
    let mut builder = test_codex()
        .with_model(model)
        .with_extensions(Arc::new(extensions.build()))
        .with_pre_build_hook(move |home| {
            std::fs::write(
                home.join("config.toml"),
                format!("[features]\nguardianv2 = {guardian_v2_config}\n"),
            )
            .expect("Guardian v2 configuration should be written");
        })
        .with_cloud_config_bundle(
            CloudConfigBundleFixture::loader_with_enterprise_requirement(requirements),
        )
        .with_config(|config| {
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config
                .permissions
                .set_permission_profile(PermissionProfile::read_only())
                .expect("set read-only permission profile");
            config
                .features
                .enable(Feature::RequestPermissionsTool)
                .expect("test config should allow feature update");
        });
    let test = builder.build_with_auto_env(&server).await?;
    assert!(!test.config.features.enabled(Feature::GuardianV2));

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: reason.into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::RequestPermissions(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;
    assert!(
        matches!(event, EventMsg::TurnComplete(_)),
        "required-model review should not prompt the user: {event:?}"
    );

    let output = responses
        .function_call_output_text(call_id)
        .context("expected request_permissions output")?;
    let actual: RequestPermissionsResponse = serde_json::from_str(&output)?;
    assert_eq!(
        actual,
        RequestPermissionsResponse {
            permissions: RequestPermissionProfile::default(),
            scope: PermissionGrantScope::Turn,
            strict_auto_review: false,
        }
    );
    let requests = responses.requests();
    let guardian_request = requests
        .iter()
        .find(|request| {
            request.body_json()["client_metadata"]["x-openai-subagent"].as_str() == Some("guardian")
        })
        .context("expected full Guardian review for the required model")?;
    assert!(guardian_request.body_contains_text(reason));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_model_override_uses_catalog_model_for_strict_auto_review() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = MockServer::start().await;
    let model = "remote-auto-review-parent";
    let review_model = "remote-auto-review-reviewer";
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ModelsResponse {
            models: vec![remote_model_with_auto_review_override(model, review_model)],
        }))
        .expect(1..)
        .mount(&server)
        .await;

    let permissions_call_id = "auto-review-permissions-call";
    let permissions_args = json!({
        "reason": "exercise strict Guardian model selection",
        "permissions": {
            "network": {
                "enabled": true,
            },
        },
    });
    let patch_call_id = "auto-review-patch-call";
    let patch = "*** Begin Patch\n*** Add File: auto-review-model-override.txt\n+exercise Guardian model selection\n*** End Patch\n";
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-parent-1"),
                ev_function_call(
                    permissions_call_id,
                    "request_permissions",
                    &serde_json::to_string(&permissions_args)?,
                ),
                ev_completed("resp-parent-1"),
            ]),
            sse(vec![
                ev_response_created("resp-parent-2"),
                ev_apply_patch_custom_tool_call(patch_call_id, patch),
                ev_completed("resp-parent-2"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian"),
                ev_assistant_message(
                    "msg-guardian",
                    &json!({
                        "risk_level": "low",
                        "user_authorization": "high",
                        "outcome": "allow",
                        "rationale": "The patch only exercises Guardian model selection.",
                    })
                    .to_string(),
                ),
                ev_completed("resp-guardian"),
            ]),
            sse(vec![
                ev_response_created("resp-parent-3"),
                ev_assistant_message("msg-parent", "done"),
                ev_completed("resp-parent-3"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(|config| {
            config.model = Some("gpt-5.4".to_string());
            config.approvals_reviewer = ApprovalsReviewer::User;
            config
                .features
                .enable(Feature::ExecPermissionApprovals)
                .expect("test config should allow feature update");
            config
                .features
                .enable(Feature::RequestPermissionsTool)
                .expect("test config should allow feature update");
        });
    let TestCodex {
        codex,
        cwd,
        config,
        thread_manager,
        ..
    } = builder.build(&server).await?;

    let models_manager = thread_manager.get_models_manager();
    timeout(
        Duration::from_secs(10),
        models_manager.list_models(
            RefreshStrategy::Online,
            codex_core::test_support::default_http_client_factory(),
        ),
    )
    .await?;
    assert!(
        server
            .received_requests()
            .await
            .expect("mock server should retain received requests")
            .iter()
            .any(|request| request.method == "GET" && request.url.path() == "/v1/models"),
        "expected the model catalog to be fetched remotely"
    );
    let model_info = models_manager
        .get_model_info(model, &config.to_models_manager_config())
        .await;
    assert_eq!(
        model_info.auto_review_model_override,
        Some(review_model.to_string())
    );

    core_test_support::submit_thread_settings(
        &codex,
        ThreadSettingsOverrides {
            model: Some(model.to_string()),
            ..Default::default()
        },
    )
    .await?;

    let cwd_path = cwd.abs();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::read_only(), cwd_path.as_path());
    codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "run the Guardian model override check".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(local_selections(cwd_path)),
                approval_policy: Some(AskForApproval::OnRequest),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                ..Default::default()
            }),
        )
        .await?;

    let permissions_request = wait_for_event(&codex, |event| {
        matches!(
            event,
            EventMsg::RequestPermissions(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;
    let EventMsg::RequestPermissions(permissions_request) = permissions_request else {
        panic!("expected request_permissions before completion");
    };
    assert_eq!(permissions_request.call_id, permissions_call_id);
    codex
        .submit(Op::RequestPermissionsResponse {
            id: permissions_request.call_id,
            response: RequestPermissionsResponse {
                permissions: permissions_request.permissions,
                scope: PermissionGrantScope::Turn,
                strict_auto_review: true,
            },
        })
        .await?;

    wait_for_event_with_timeout(
        &codex,
        |event| matches!(event, EventMsg::TurnComplete(_)),
        Duration::from_secs(15),
    )
    .await;

    let guardian_request = responses
        .requests()
        .into_iter()
        .find(|request| {
            request.body_contains_text("auto-review-model-override.txt")
                && request
                    .instructions_text()
                    .starts_with("You are judging one planned coding-agent action.")
        })
        .expect("expected Guardian request for apply_patch");
    assert_eq!(
        guardian_request.body_json()["model"].as_str(),
        Some(review_model)
    );

    timeout(Duration::from_secs(10), codex.shutdown_and_wait()).await??;

    Ok(())
}

fn remote_model_with_auto_review_override(slug: &str, review_model: &str) -> ModelInfo {
    ModelInfo {
        slug: slug.to_string(),
        display_name: format!("{slug} display"),
        description: Some(format!("{slug} description")),
        default_reasoning_level: Some(ReasoningEffort::Medium),
        supported_reasoning_levels: vec![ReasoningEffortPreset {
            effort: ReasoningEffort::Medium,
            description: ReasoningEffort::Medium.to_string(),
        }],
        shell_type: ConfigShellToolType::UnifiedExec,
        visibility: ModelVisibility::List,
        supported_in_api: true,
        input_modalities: default_input_modalities(),
        used_fallback_model_metadata: false,
        supports_search_tool: false,
        use_responses_lite: false,
        node_repl_auto_review_required: false,
        node_repl_disabled: false,
        auto_review_model_override: Some(review_model.to_string()),
        model_specialty: None,
        tool_mode: None,
        multi_agent_version: None,
        priority: 1,
        additional_speed_tiers: Vec::new(),
        service_tiers: Vec::new(),
        default_service_tier: None,
        upgrade: None,
        model_messages: None,
        include_skills_usage_instructions: false,
        include_plugin_usage_instructions: false,
        include_apps_usage_instructions: false,
        supports_reasoning_summary_parameter: true,
        default_reasoning_summary: ReasoningSummary::Auto,
        support_verbosity: false,
        default_verbosity: None,
        availability_nux: None,
        apply_patch_tool_type: Some(ApplyPatchToolType::Freeform),
        web_search_tool_type: Default::default(),
        truncation_policy: TruncationPolicyConfig::bytes(/*limit*/ 10_000),
        supports_image_detail_original: false,
        context_window: Some(272_000),
        max_context_window: None,
        auto_compact_token_limit: None,
        comp_hash: None,
        effective_context_window_percent: 95,
        experimental_supported_tools: Vec::new(),
    }
}
