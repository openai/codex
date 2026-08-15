use anyhow::Result;
use codex_config::types::Personality;
use codex_core::CodexThread;
use codex_core::ForkSnapshot;
use codex_core::TurnInputRequest;
use codex_core::config::Constrained;
use codex_features::Feature;
use codex_history::RolloutItem;
use codex_history::RolloutLine;
use codex_login::CodexAuth;
use codex_models_manager::bundled_models_response;
use codex_models_manager::manager::RefreshStrategy;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::SERVICE_TIER_DEFAULT_REQUEST_VALUE;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::config_types::Settings;
use codex_protocol::models::BaseInstructionsProvenance;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelServiceTier;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::openai_models::default_input_modalities;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_completed_with_tokens;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_image_generation_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_models_once;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::sse_completed;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashMap;
use test_case::test_case;
use wiremock::MockServer;

fn read_only_user_turn(test: &TestCodex, items: Vec<UserInput>, model: String) -> TurnInputRequest {
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::read_only(), test.cwd_path());
    TurnInputRequest::user_input(items).with_thread_settings(ThreadSettingsOverrides {
        environments: Some(local_selections(test.config.cwd.clone())),
        approval_policy: Some(AskForApproval::Never),
        sandbox_policy: Some(sandbox_policy),
        permission_profile,
        collaboration_mode: Some(CollaborationMode {
            mode: ModeKind::Default,
            settings: Settings {
                model,
                reasoning_effort: test.config.model_reasoning_effort.clone(),
                developer_instructions: None,
            },
        }),
        ..Default::default()
    })
}

async fn submit_model_turn(
    thread: &CodexThread,
    model: &str,
    mut thread_settings: ThreadSettingsOverrides,
) -> Result<()> {
    thread_settings.model = Some(model.to_string());
    thread
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "switch models".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(thread_settings),
        )
        .await?;
    wait_for_event(thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    Ok(())
}

fn test_model_info(
    slug: &str,
    display_name: &str,
    description: &str,
    input_modalities: Vec<InputModality>,
) -> ModelInfo {
    ModelInfo {
        slug: slug.to_string(),
        display_name: display_name.to_string(),
        description: Some(description.to_string()),
        default_reasoning_level: Some(ReasoningEffort::Medium),
        supported_reasoning_levels: vec![ReasoningEffortPreset {
            effort: ReasoningEffort::Medium,
            description: ReasoningEffort::Medium.to_string(),
        }],
        shell_type: ConfigShellToolType::ShellCommand,
        visibility: ModelVisibility::List,
        supported_in_api: true,
        input_modalities,
        used_fallback_model_metadata: false,
        supports_search_tool: false,
        use_responses_lite: false,
        node_repl_auto_review_required: false,
        node_repl_disabled: false,
        auto_review_model_override: None,
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
        apply_patch_tool_type: None,
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

#[test_case(None; "model only")]
#[test_case(Some(Personality::Pragmatic); "model and personality")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn first_turn_model_change_appends_model_instructions_developer_message(
    personality: Option<Personality>,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = MockServer::start().await;
    let resp_mock = mount_sse_once(&server, sse_completed("resp-1")).await;

    let mut builder = test_codex().with_model("gpt-5.2").with_config(|config| {
        config
            .features
            .enable(Feature::Personality)
            .expect("test config should allow feature update");
    });
    let test = builder.build_with_auto_env(&server).await?;
    let next_model = "gpt-5.4";

    submit_model_turn(
        &test.codex,
        next_model,
        ThreadSettingsOverrides {
            personality,
            ..Default::default()
        },
    )
    .await?;

    let request = resp_mock.single_request();
    assert_eq!(request.body_json()["model"], next_model);
    let developer_texts = request.message_input_texts("developer");
    let expected_instructions = bundled_models_response()?
        .models
        .into_iter()
        .find(|model| model.slug == next_model)
        .expect("expected target model in bundled catalog")
        .get_model_instructions(personality.or(test.config.personality));
    assert!(
        developer_texts.iter().any(|text| {
            text.contains("<model_switch>") && text.contains(&expected_instructions)
        })
    );
    assert!(
        developer_texts
            .iter()
            .all(|text| !text.contains("<personality_spec>")),
        "model instructions already include the selected personality"
    );

    Ok(())
}

#[test_case(None, "gpt-5.2"; "model-generated base instructions and original model")]
#[test_case(None, "gpt-5.4"; "model-generated base instructions and fork model")]
#[test_case(Some("inherited custom base instructions"), "gpt-5.2"; "custom base instructions and original model")]
#[test_case(Some("inherited custom base instructions"), "gpt-5.4"; "custom base instructions and fork model")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn first_turn_after_empty_prefix_fork_preserves_inherited_base_instructions(
    custom_base_instructions: Option<&'static str>,
    turn_model: &'static str,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = MockServer::start().await;
    let resp_mock = mount_sse_once(&server, sse_completed("resp-fork")).await;

    let initial_model = "gpt-5.2";
    let mut builder = test_codex()
        .with_model(initial_model)
        .with_config(move |config| {
            config.base_instructions = custom_base_instructions.map(str::to_string);
        });
    let test = builder.build_with_auto_env(&server).await?;
    test.codex.ensure_rollout_materialized().await;
    test.codex.flush_rollout().await?;
    let source_rollout_path = test.codex.rollout_path().expect("rollout path");
    let source_history =
        codex_rollout::RolloutRecorder::get_rollout_history(&source_rollout_path).await?;
    let expected_provenance = match custom_base_instructions {
        Some(_) => BaseInstructionsProvenance::Custom,
        None => BaseInstructionsProvenance::Model {
            model: initial_model.to_string(),
        },
    };
    assert_eq!(
        source_history
            .get_base_instructions()
            .and_then(|instructions| instructions.provenance),
        Some(expected_provenance)
    );

    let mut fork_config = test.config.clone();
    fork_config.model = Some("gpt-5.4".to_string());
    fork_config.base_instructions = None;
    let fork = test
        .thread_manager
        .fork_thread(
            ForkSnapshot::TruncateBeforeNthUserMessage(0),
            fork_config,
            source_rollout_path,
            /*thread_source*/ None,
            /*parent_trace*/ None,
        )
        .await?;
    submit_model_turn(&fork.thread, turn_model, ThreadSettingsOverrides::default()).await?;

    let request = resp_mock.single_request();
    assert_eq!(request.body_json()["model"], turn_model);
    if let Some(instructions) = custom_base_instructions {
        assert_eq!(request.instructions_text(), instructions);
    }
    let model_switch_count = request
        .message_input_texts("developer")
        .iter()
        .filter(|text| text.contains("<model_switch>"))
        .count();
    assert_eq!(
        model_switch_count,
        usize::from(custom_base_instructions.is_none() && turn_model != initial_model),
        "only inherited model-generated instructions should change models"
    );

    Ok(())
}

#[derive(Clone, Copy)]
enum RollbackFollowup {
    StartupModel,
    SwitchedModel,
    ColdResume,
}

#[test_case(RollbackFollowup::StartupModel; "return to startup model")]
#[test_case(RollbackFollowup::SwitchedModel; "retry switched model")]
#[test_case(RollbackFollowup::ColdResume; "retry switched model after cold resume")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rollback_first_turn_model_change_removes_its_instructions(
    followup: RollbackFollowup,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = MockServer::start().await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![sse_completed("resp-first"), sse_completed("resp-followup")],
    )
    .await;

    let initial_model = "gpt-5.2";
    let switched_model = "gpt-5.4";
    let mut builder = test_codex().with_model(initial_model);
    let test = builder.build_with_auto_env(&server).await?;

    submit_model_turn(
        &test.codex,
        switched_model,
        ThreadSettingsOverrides::default(),
    )
    .await?;

    test.codex
        .submit(Op::ThreadRollback { num_turns: 1 })
        .await?;
    wait_for_event(&test.codex, |ev| {
        matches!(ev, EventMsg::ThreadRolledBack(_))
    })
    .await;

    let test = match followup {
        RollbackFollowup::ColdResume => {
            let mut resume_builder = test_codex().with_model(switched_model);
            resume_builder.restart(&server, &test).await?
        }
        RollbackFollowup::StartupModel | RollbackFollowup::SwitchedModel => test,
    };
    let followup_model = match followup {
        RollbackFollowup::StartupModel => initial_model,
        RollbackFollowup::SwitchedModel | RollbackFollowup::ColdResume => switched_model,
    };
    submit_model_turn(
        &test.codex,
        followup_model,
        ThreadSettingsOverrides::default(),
    )
    .await?;

    let request = &response_mock.requests()[1];
    assert_eq!(request.body_json()["model"], followup_model);
    let model_switch_count = request
        .message_input_texts("developer")
        .iter()
        .filter(|text| text.contains("<model_switch>"))
        .count();
    assert_eq!(
        model_switch_count,
        usize::from(followup_model == switched_model),
        "rolled-back model instructions must not survive or be duplicated"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_change_appends_model_instructions_developer_message() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = MockServer::start().await;
    let resp_mock = mount_sse_sequence(
        &server,
        vec![sse_completed("resp-1"), sse_completed("resp-2")],
    )
    .await;

    let mut builder = test_codex().with_model("gpt-5.2");
    let test = builder.build(&server).await?;
    let next_model = "gpt-5.4";

    test.codex
        .start_or_steer_turn(read_only_user_turn(
            &test,
            vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }],
            test.session_configured.model.clone(),
        ))
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            model: Some(next_model.to_string()),
            ..Default::default()
        },
    )
    .await?;

    test.codex
        .start_or_steer_turn(read_only_user_turn(
            &test,
            vec![UserInput::Text {
                text: "switch models".into(),
                text_elements: Vec::new(),
            }],
            next_model.to_string(),
        ))
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = resp_mock.requests();
    assert_eq!(requests.len(), 2, "expected two model requests");

    let second_request = requests.last().expect("expected second request");
    let developer_texts = second_request.message_input_texts("developer");
    let model_switch_text = developer_texts
        .iter()
        .find(|text| text.contains("<model_switch>"))
        .expect("expected model switch message in developer input");
    assert!(
        model_switch_text.contains("The user was previously using a different model."),
        "expected model switch preamble, got: {model_switch_text:?}"
    );

    test.codex.ensure_rollout_materialized().await;
    test.codex.flush_rollout().await?;
    let rollout_path = test.codex.rollout_path().expect("rollout path");
    let model_states = std::fs::read_to_string(rollout_path)?
        .lines()
        .map(serde_json::from_str::<RolloutLine>)
        .collect::<serde_json::Result<Vec<_>>>()?
        .into_iter()
        .filter_map(|line| match line.item {
            RolloutItem::WorldState(item) => item
                .state
                .get("model")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        model_states,
        vec![
            test.session_configured.model.clone(),
            next_model.to_string()
        ]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_and_personality_change_only_appends_model_instructions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let resp_mock = mount_sse_sequence(
        &server,
        vec![sse_completed("resp-1"), sse_completed("resp-2")],
    )
    .await;

    let mut builder = test_codex().with_model("gpt-5.4").with_config(|config| {
        config
            .features
            .enable(Feature::Personality)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;
    let next_model = "exp-codex-personality";

    test.codex
        .start_or_steer_turn(read_only_user_turn(
            &test,
            vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }],
            test.session_configured.model.clone(),
        ))
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            model: Some(next_model.to_string()),
            personality: Some(Personality::Pragmatic),
            ..Default::default()
        },
    )
    .await?;

    test.codex
        .start_or_steer_turn(read_only_user_turn(
            &test,
            vec![UserInput::Text {
                text: "switch model and personality".into(),
                text_elements: Vec::new(),
            }],
            next_model.to_string(),
        ))
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = resp_mock.requests();
    assert_eq!(requests.len(), 2, "expected two model requests");

    let second_request = requests.last().expect("expected second request");
    let developer_texts = second_request.message_input_texts("developer");
    assert!(
        developer_texts
            .iter()
            .any(|text| text.contains("<model_switch>")),
        "expected model switch message when model changes"
    );
    assert!(
        !developer_texts
            .iter()
            .any(|text| text.contains("<personality_spec>")),
        "did not expect personality update message when model changed in same turn"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn settings_update_during_active_turn_applies_to_next_turn_only() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    "pause-turn",
                    "request_user_input",
                    &json!({
                        "questions": [{
                            "id": "continue",
                            "header": "Continue",
                            "question": "Continue after settings update?",
                            "options": [{
                                "label": "Yes (Recommended)",
                                "description": "Continue the current turn."
                            }, {
                                "label": "No",
                                "description": "Stop the current turn."
                            }]
                        }]
                    })
                    .to_string(),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "first turn done"),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_assistant_message("msg-3", "second turn done"),
                ev_completed("resp-3"),
            ]),
        ],
    )
    .await;
    let mut builder = test_codex().with_model("gpt-5.2").with_config(|config| {
        config
            .features
            .enable(Feature::DefaultModeRequestUserInput)
            .expect("test config should allow feature update");
        config.model_reasoning_effort = Some(ReasoningEffort::Low);
        config.model_reasoning_summary = Some(ReasoningSummary::Concise);
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
        config.approvals_reviewer = ApprovalsReviewer::User;
    });
    let test = builder.build_with_auto_env(&server).await?;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "pause before continuing".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let request = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RequestUserInput(request) => Some(request.clone()),
        _ => None,
    })
    .await;

    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            model: Some("gpt-5.4".to_string()),
            effort: Some(Some(ReasoningEffort::High)),
            summary: Some(ReasoningSummary::Detailed),
            service_tier: Some(Some(ServiceTier::Fast.request_value().to_string())),
            approval_policy: Some(AskForApproval::Never),
            approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
            ..Default::default()
        },
    )
    .await?;

    test.codex
        .submit(Op::UserInputAnswer {
            id: request.turn_id,
            response: RequestUserInputResponse {
                answers: HashMap::from([(
                    "continue".to_string(),
                    RequestUserInputAnswer {
                        answers: vec!["Yes (Recommended)".to_string()],
                    },
                )]),
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.submit_text_turn("start the next turn").await?;

    let requests = response_mock.requests();
    let request_settings = requests
        .iter()
        .map(|request| {
            let body = request.body_json();
            json!({
                "model": body["model"],
                "reasoning": body["reasoning"],
                "service_tier": body.get("service_tier"),
                "approval_policy_never": request
                    .message_input_texts("developer")
                    .iter()
                    .any(|text| text.contains("Approval policy is currently never")),
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        request_settings,
        vec![
            json!({
                "model": "gpt-5.2",
                "reasoning": { "effort": "low", "summary": "concise" },
                "service_tier": null,
                "approval_policy_never": false,
            }),
            json!({
                "model": "gpt-5.2",
                "reasoning": { "effort": "low", "summary": "concise" },
                "service_tier": null,
                "approval_policy_never": false,
            }),
            json!({
                "model": "gpt-5.4",
                "reasoning": { "effort": "high", "summary": "detailed" },
                "service_tier": "priority",
                "approval_policy_never": true,
            }),
        ]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn service_tier_change_is_applied_on_next_http_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let resp_mock = mount_sse_sequence(
        &server,
        vec![sse_completed("resp-1"), sse_completed("resp-2")],
    )
    .await;

    let test = test_codex().build(&server).await?;

    test.submit_turn_with_service_tier("fast turn", Some(ServiceTier::Fast.request_value()))
        .await?;
    test.submit_turn_with_service_tier("standard turn", /*service_tier*/ None)
        .await?;

    let requests = resp_mock.requests();
    assert_eq!(requests.len(), 2, "expected two model requests");

    let first_body = requests[0].body_json();
    let second_body = requests[1].body_json();

    assert_eq!(first_body["service_tier"].as_str(), Some("priority"));
    assert_eq!(second_body.get("service_tier"), None);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn flex_service_tier_is_applied_to_http_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let model_slug = "test-flex-model";
    let mut flex_model = test_model_info(
        model_slug,
        model_slug,
        "supports flex tier",
        default_input_modalities(),
    );
    flex_model.service_tiers = vec![ModelServiceTier {
        id: ServiceTier::Flex.request_value().to_string(),
        name: "flex".to_string(),
        description: "Flexible processing.".to_string(),
    }];
    let resp_mock = mount_sse_once(&server, sse_completed("resp-1")).await;

    let mut builder = test_codex()
        .with_model(model_slug)
        .with_config(move |config| {
            config.model_catalog = Some(ModelsResponse {
                models: vec![flex_model],
            });
        });
    let test = builder.build(&server).await?;

    test.submit_turn_with_service_tier("flex turn", Some(ServiceTier::Flex.request_value()))
        .await?;

    let request = resp_mock.single_request();
    let body = request.body_json();
    assert_eq!(body["service_tier"].as_str(), Some("flex"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unsupported_service_tier_is_omitted_from_http_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let model_slug = "test-no-tier-model";
    let model = test_model_info(
        model_slug,
        model_slug,
        "no service tiers",
        default_input_modalities(),
    );
    let resp_mock = mount_sse_once(&server, sse_completed("resp-1")).await;

    let mut builder = test_codex()
        .with_model(model_slug)
        .with_config(move |config| {
            config.model_catalog = Some(ModelsResponse {
                models: vec![model],
            });
        });
    let test = builder.build(&server).await?;

    test.submit_turn_with_service_tier("fast turn", Some(ServiceTier::Fast.request_value()))
        .await?;

    let request = resp_mock.single_request();
    let body = request.body_json();
    assert_eq!(body.get("service_tier"), None);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unsupported_configured_service_tier_warns_at_session_start() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let model_slug = "test-no-tier-model";
    let model = test_model_info(
        model_slug,
        model_slug,
        "no service tiers",
        default_input_modalities(),
    );
    let mut builder = test_codex()
        .with_model(model_slug)
        .with_config(move |config| {
            config.service_tier = Some(ServiceTier::Flex.request_value().to_string());
            config.model_catalog = Some(ModelsResponse {
                models: vec![model],
            });
        });
    let test = builder.build(&server).await?;

    let warning = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::Warning(warning)
                if warning.message.contains("will be omitted from requests")
        )
    })
    .await;
    let EventMsg::Warning(warning) = warning else {
        unreachable!("wait_for_event matched a warning")
    };
    assert_eq!(
        warning.message,
        "Configured service tier `flex` is not advertised as supported for model `test-no-tier-model` and will be omitted from requests."
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn default_service_tier_override_is_omitted_from_http_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let model_slug = "test-default-tier-model";
    let mut model = test_model_info(
        model_slug,
        model_slug,
        "has catalog default service tier",
        default_input_modalities(),
    );
    model.service_tiers = vec![ModelServiceTier {
        id: ServiceTier::Fast.request_value().to_string(),
        name: "fast".to_string(),
        description: "Fast processing.".to_string(),
    }];
    model.default_service_tier = Some(ServiceTier::Fast.request_value().to_string());
    let resp_mock = mount_sse_once(&server, sse_completed("resp-1")).await;

    let mut builder = test_codex()
        .with_model(model_slug)
        .with_config(move |config| {
            config.model_catalog = Some(ModelsResponse {
                models: vec![model],
            });
        });
    let test = builder.build(&server).await?;

    test.submit_turn_with_service_tier("default turn", Some(SERVICE_TIER_DEFAULT_REQUEST_VALUE))
        .await?;

    let request = resp_mock.single_request();
    let body = request.body_json();
    assert_eq!(body.get("service_tier"), None);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn null_service_tier_override_is_omitted_from_http_turn_with_catalog_default() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let model_slug = "test-null-default-tier-model";
    let mut model = test_model_info(
        model_slug,
        model_slug,
        "has catalog default service tier",
        default_input_modalities(),
    );
    model.service_tiers = vec![ModelServiceTier {
        id: ServiceTier::Fast.request_value().to_string(),
        name: "fast".to_string(),
        description: "Fast processing.".to_string(),
    }];
    model.default_service_tier = Some(ServiceTier::Fast.request_value().to_string());
    let resp_mock = mount_sse_once(&server, sse_completed("resp-1")).await;

    let mut builder = test_codex()
        .with_model(model_slug)
        .with_config(move |config| {
            config.model_catalog = Some(ModelsResponse {
                models: vec![model],
            });
        });
    let test = builder.build(&server).await?;

    test.submit_turn_with_service_tier("standard turn", /*service_tier*/ None)
        .await?;

    let request = resp_mock.single_request();
    let body = request.body_json();
    assert_eq!(body.get("service_tier"), None);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_change_from_multimodal_to_text_strips_prior_media_content() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = MockServer::start().await;
    let multimodal_model_slug = "test-multimodal-model";
    let text_model_slug = "test-text-only-model";
    let multimodal_model = test_model_info(
        multimodal_model_slug,
        "Test Multimodal Model",
        "supports image and audio input",
        vec![
            InputModality::Text,
            InputModality::Image,
            InputModality::Audio,
        ],
    );
    let text_model = test_model_info(
        text_model_slug,
        "Test Text Model",
        "text only",
        vec![InputModality::Text],
    );
    mount_models_once(
        &server,
        ModelsResponse {
            models: vec![multimodal_model, text_model],
        },
    )
    .await;

    let responses = mount_sse_sequence(
        &server,
        vec![sse_completed("resp-1"), sse_completed("resp-2")],
    )
    .await;

    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| {
            config.model = Some(multimodal_model_slug.to_string());
        });
    let test = builder.build(&server).await?;
    let models_manager = test.thread_manager.get_models_manager();
    let _ = models_manager
        .list_models(
            RefreshStrategy::OnlineIfUncached,
            codex_core::test_support::default_http_client_factory(),
        )
        .await;
    let image_url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg=="
        .to_string();

    test.codex
        .start_or_steer_turn(read_only_user_turn(
            &test,
            vec![
                UserInput::Image {
                    image_url: image_url.clone(),
                    detail: None,
                },
                UserInput::Audio {
                    audio_url: "data:audio/wav;base64,YXVkaW8=".to_string(),
                },
                UserInput::Text {
                    text: "first turn".to_string(),
                    text_elements: Vec::new(),
                },
            ],
            multimodal_model_slug.to_string(),
        ))
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    test.codex
        .start_or_steer_turn(read_only_user_turn(
            &test,
            vec![UserInput::Text {
                text: "second turn".to_string(),
                text_elements: Vec::new(),
            }],
            text_model_slug.to_string(),
        ))
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2, "expected two model requests");

    let first_request = requests.first().expect("expected first request");
    assert!(
        !first_request.message_input_image_urls("user").is_empty(),
        "first request should include the uploaded image"
    );
    assert_eq!(
        first_request.message_input_audio_urls("user"),
        vec!["data:audio/wav;base64,YXVkaW8=".to_string()]
    );

    let second_request = requests.last().expect("expected second request");
    assert!(
        second_request.message_input_image_urls("user").is_empty(),
        "second request should strip unsupported image content"
    );
    assert!(
        second_request.message_input_audio_urls("user").is_empty(),
        "second request should strip unsupported audio content"
    );
    let second_user_texts = second_request.message_input_texts("user");
    assert!(
        second_user_texts
            .iter()
            .any(|text| text == "image content omitted because you do not support image input"),
        "second request should include the image-omitted placeholder text"
    );
    assert!(
        second_user_texts
            .iter()
            .any(|text| text == "audio content omitted because you do not support audio input"),
        "second request should include the audio-omitted placeholder text"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn generated_image_is_replayed_for_image_capable_models() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = MockServer::start().await;
    let image_model_slug = "test-image-model";
    let image_model = test_model_info(
        image_model_slug,
        "Test Image Model",
        "supports image input",
        default_input_modalities(),
    );
    mount_models_once(
        &server,
        ModelsResponse {
            models: vec![image_model],
        },
    )
    .await;

    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_image_generation_call("ig_123", "completed", "lobster", "Zm9v"),
                ev_completed_with_tokens("resp-1", /*total_tokens*/ 10),
            ]),
            sse_completed("resp-2"),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| {
            config.model = Some(image_model_slug.to_string());
        });
    let test = builder.build(&server).await?;
    let models_manager = test.thread_manager.get_models_manager();
    let _ = models_manager
        .list_models(
            RefreshStrategy::OnlineIfUncached,
            codex_core::test_support::default_http_client_factory(),
        )
        .await;

    test.codex
        .start_or_steer_turn(read_only_user_turn(
            &test,
            vec![UserInput::Text {
                text: "generate a lobster".to_string(),
                text_elements: Vec::new(),
            }],
            image_model_slug.to_string(),
        ))
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    test.codex
        .start_or_steer_turn(read_only_user_turn(
            &test,
            vec![UserInput::Text {
                text: "describe the generated image".to_string(),
                text_elements: Vec::new(),
            }],
            image_model_slug.to_string(),
        ))
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2, "expected two model requests");

    let second_request = requests.last().expect("expected second request");
    let image_generation_calls = second_request.inputs_of_type("image_generation_call");
    assert_eq!(
        image_generation_calls.len(),
        1,
        "expected generated image history to be replayed as an image_generation_call"
    );
    assert_eq!(
        image_generation_calls[0]["result"].as_str(),
        Some("Zm9v"),
        "expected the original generated image payload to be preserved"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_change_from_generated_image_to_text_preserves_prior_generated_image_call()
-> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = MockServer::start().await;
    let image_model_slug = "test-image-model";
    let text_model_slug = "test-text-only-model";
    let image_model = test_model_info(
        image_model_slug,
        "Test Image Model",
        "supports image input",
        default_input_modalities(),
    );
    let text_model = test_model_info(
        text_model_slug,
        "Test Text Model",
        "text only",
        vec![InputModality::Text],
    );
    mount_models_once(
        &server,
        ModelsResponse {
            models: vec![image_model, text_model],
        },
    )
    .await;

    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_image_generation_call("ig_123", "completed", "lobster", "Zm9v"),
                ev_completed_with_tokens("resp-1", /*total_tokens*/ 10),
            ]),
            sse_completed("resp-2"),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| {
            config.model = Some(image_model_slug.to_string());
        });
    let test = builder.build(&server).await?;
    let models_manager = test.thread_manager.get_models_manager();
    let _ = models_manager
        .list_models(
            RefreshStrategy::OnlineIfUncached,
            codex_core::test_support::default_http_client_factory(),
        )
        .await;

    test.codex
        .start_or_steer_turn(read_only_user_turn(
            &test,
            vec![UserInput::Text {
                text: "generate a lobster".to_string(),
                text_elements: Vec::new(),
            }],
            image_model_slug.to_string(),
        ))
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    test.codex
        .start_or_steer_turn(read_only_user_turn(
            &test,
            vec![UserInput::Text {
                text: "describe the generated image".to_string(),
                text_elements: Vec::new(),
            }],
            text_model_slug.to_string(),
        ))
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2, "expected two model requests");

    let second_request = requests.last().expect("expected second request");
    let image_generation_calls = second_request.inputs_of_type("image_generation_call");
    assert!(
        second_request.message_input_image_urls("user").is_empty(),
        "second request should not rewrite generated images into message input images"
    );
    assert!(
        image_generation_calls.len() == 1,
        "second request should preserve the generated image call for text-only models"
    );
    assert_eq!(
        image_generation_calls[0]["result"].as_str(),
        Some(""),
        "second request should strip generated image bytes for text-only models"
    );
    assert!(
        second_request
            .message_input_texts("user")
            .iter()
            .all(|text| text != "image content omitted because you do not support image input"),
        "second request should not inject the image-omitted placeholder text"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn thread_rollback_after_generated_image_drops_entire_image_turn_history() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = MockServer::start().await;
    let image_model_slug = "test-image-model";
    let image_model = test_model_info(
        image_model_slug,
        "Test Image Model",
        "supports image input",
        default_input_modalities(),
    );
    mount_models_once(
        &server,
        ModelsResponse {
            models: vec![image_model],
        },
    )
    .await;

    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_image_generation_call("ig_rollback", "completed", "lobster", "Zm9v"),
                ev_completed_with_tokens("resp-1", /*total_tokens*/ 10),
            ]),
            sse_completed("resp-2"),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| {
            config.model = Some(image_model_slug.to_string());
        });
    let test = builder.build(&server).await?;
    let models_manager = test.thread_manager.get_models_manager();
    let _ = models_manager
        .list_models(
            RefreshStrategy::OnlineIfUncached,
            codex_core::test_support::default_http_client_factory(),
        )
        .await;

    test.codex
        .start_or_steer_turn(read_only_user_turn(
            &test,
            vec![UserInput::Text {
                text: "generate a lobster".to_string(),
                text_elements: Vec::new(),
            }],
            image_model_slug.to_string(),
        ))
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    test.codex
        .submit(Op::ThreadRollback { num_turns: 1 })
        .await?;
    wait_for_event(&test.codex, |ev| {
        matches!(ev, EventMsg::ThreadRolledBack(_))
    })
    .await;

    test.codex
        .start_or_steer_turn(read_only_user_turn(
            &test,
            vec![UserInput::Text {
                text: "after rollback".to_string(),
                text_elements: Vec::new(),
            }],
            image_model_slug.to_string(),
        ))
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2, "expected two model requests");

    let second_request = requests.last().expect("expected second request");
    assert!(
        !second_request
            .message_input_texts("user")
            .iter()
            .any(|text| text == "generate a lobster"),
        "rollback should remove the rolled-back image-generation user turn"
    );
    assert!(
        second_request
            .inputs_of_type("image_generation_call")
            .is_empty(),
        "rollback should remove the generated image call with the rolled-back turn"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_switch_to_smaller_model_updates_token_context_window() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let large_model_slug = "test-image-model";
    let smaller_model_slug = "test-text-only-model";
    let large_context_window = 272_000;
    let smaller_context_window = 128_000;
    let effective_context_window_percent = 95;
    let large_effective_window = (large_context_window * effective_context_window_percent) / 100;
    let smaller_effective_window =
        (smaller_context_window * effective_context_window_percent) / 100;

    let base_model = ModelInfo {
        slug: large_model_slug.to_string(),
        display_name: "Larger Model".to_string(),
        description: Some("larger context window model".to_string()),
        default_reasoning_level: Some(ReasoningEffort::Medium),
        supported_reasoning_levels: vec![ReasoningEffortPreset {
            effort: ReasoningEffort::Medium,
            description: ReasoningEffort::Medium.to_string(),
        }],
        shell_type: ConfigShellToolType::ShellCommand,
        visibility: ModelVisibility::List,
        supported_in_api: true,
        input_modalities: default_input_modalities(),
        used_fallback_model_metadata: false,
        supports_search_tool: false,
        use_responses_lite: false,
        node_repl_auto_review_required: false,
        node_repl_disabled: false,
        auto_review_model_override: None,
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
        apply_patch_tool_type: None,
        web_search_tool_type: Default::default(),
        truncation_policy: TruncationPolicyConfig::bytes(/*limit*/ 10_000),
        supports_image_detail_original: false,
        context_window: Some(large_context_window),
        max_context_window: None,
        auto_compact_token_limit: None,
        comp_hash: None,
        effective_context_window_percent,
        experimental_supported_tools: Vec::new(),
    };
    let mut smaller_model = base_model.clone();
    smaller_model.slug = smaller_model_slug.to_string();
    smaller_model.display_name = "Smaller Model".to_string();
    smaller_model.description = Some("smaller context window model".to_string());
    smaller_model.context_window = Some(smaller_context_window);

    mount_models_once(
        &server,
        ModelsResponse {
            models: vec![base_model, smaller_model],
        },
    )
    .await;

    mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_completed_with_tokens("resp-1", /*total_tokens*/ 100),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_completed_with_tokens("resp-2", /*total_tokens*/ 120),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(|config| {
            config.model = Some(large_model_slug.to_string());
        });
    let test = builder.build(&server).await?;

    let models_manager = test.thread_manager.get_models_manager();
    let available_models = models_manager
        .list_models(
            RefreshStrategy::Online,
            codex_core::test_support::default_http_client_factory(),
        )
        .await;
    assert!(
        available_models
            .iter()
            .any(|model| model.model == smaller_model_slug),
        "expected {smaller_model_slug} to be available in remote model list"
    );
    let large_model_info = models_manager
        .get_model_info(large_model_slug, &test.config.to_models_manager_config())
        .await;
    assert_eq!(large_model_info.context_window, Some(large_context_window));
    let smaller_model_info = models_manager
        .get_model_info(smaller_model_slug, &test.config.to_models_manager_config())
        .await;
    assert_eq!(
        smaller_model_info.context_window,
        Some(smaller_context_window)
    );

    test.codex
        .start_or_steer_turn(read_only_user_turn(
            &test,
            vec![UserInput::Text {
                text: "use larger model".into(),
                text_elements: Vec::new(),
            }],
            large_model_slug.to_string(),
        ))
        .await?;

    let large_window_event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::TokenCount(token_count)
                if token_count
                    .info
                    .as_ref()
                    .is_some_and(|info| info.last_token_usage.total_tokens == 100)
        )
    })
    .await;
    let EventMsg::TokenCount(large_token_count) = large_window_event else {
        unreachable!("wait_for_event returned unexpected event");
    };
    assert_eq!(
        large_token_count
            .info
            .as_ref()
            .and_then(|info| info.model_context_window),
        Some(large_effective_window)
    );
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            model: Some(smaller_model_slug.to_string()),
            ..Default::default()
        },
    )
    .await?;

    test.codex
        .start_or_steer_turn(read_only_user_turn(
            &test,
            vec![UserInput::Text {
                text: "switch to smaller model".into(),
                text_elements: Vec::new(),
            }],
            smaller_model_slug.to_string(),
        ))
        .await?;

    let smaller_turn_started_event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::TurnStarted(started)
                if started.model_context_window == Some(smaller_effective_window)
        )
    })
    .await;
    let EventMsg::TurnStarted(smaller_turn_started) = smaller_turn_started_event else {
        unreachable!("wait_for_event returned unexpected event");
    };
    assert_eq!(
        smaller_turn_started.model_context_window,
        Some(smaller_effective_window)
    );

    let smaller_window_event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::TokenCount(token_count)
                if token_count
                    .info
                    .as_ref()
                    .is_some_and(|info| info.last_token_usage.total_tokens == 120)
        )
    })
    .await;
    let EventMsg::TokenCount(smaller_token_count) = smaller_window_event else {
        unreachable!("wait_for_event returned unexpected event");
    };
    let smaller_window = smaller_token_count
        .info
        .as_ref()
        .and_then(|info| info.model_context_window);
    assert_eq!(smaller_window, Some(smaller_effective_window));
    assert_ne!(smaller_window, Some(large_effective_window));
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    Ok(())
}
