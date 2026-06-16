use anyhow::Result;
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
use codex_protocol::request_permissions::PermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_protocol::user_input::UserInput;
use core_test_support::TempDirExt;
use core_test_support::responses::ev_apply_patch_custom_tool_call;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_models_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_sandbox;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::MockServer;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_model_override_uses_catalog_model_for_strict_auto_review() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = MockServer::start().await;
    let model = "remote-auto-review-parent";
    let review_model = "remote-auto-review-reviewer";
    mount_models_once(
        &server,
        ModelsResponse {
            models: vec![remote_model_with_auto_review_override(model, review_model)],
        },
    )
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
    models_manager
        .list_models(RefreshStrategy::OnlineIfUncached)
        .await;
    let model_info = models_manager
        .get_model_info(model, &config.to_models_manager_config())
        .await;
    assert_eq!(
        model_info.auto_review_model_override,
        Some(review_model.to_string())
    );

    core_test_support::submit_thread_settings(
        &codex,
        codex_protocol::protocol::ThreadSettingsOverrides {
            model: Some(model.to_string()),
            ..Default::default()
        },
    )
    .await?;

    let cwd_path = cwd.abs();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::read_only(), cwd_path.as_path());
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "run the Guardian model override check".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                environments: Some(local_selections(cwd_path)),
                approval_policy: Some(AskForApproval::OnRequest),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                ..Default::default()
            },
        })
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

    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

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
        shell_type: ConfigShellToolType::ShellCommand,
        visibility: ModelVisibility::List,
        supported_in_api: true,
        input_modalities: default_input_modalities(),
        used_fallback_model_metadata: false,
        supports_search_tool: false,
        use_responses_lite: false,
        auto_review_model_override: Some(review_model.to_string()),
        tool_mode: None,
        multi_agent_version: None,
        priority: 1,
        additional_speed_tiers: Vec::new(),
        service_tiers: Vec::new(),
        default_service_tier: None,
        upgrade: None,
        base_instructions: "base instructions".to_string(),
        model_messages: None,
        supports_reasoning_summaries: false,
        default_reasoning_summary: ReasoningSummary::Auto,
        support_verbosity: false,
        default_verbosity: None,
        availability_nux: None,
        apply_patch_tool_type: Some(ApplyPatchToolType::Freeform),
        web_search_tool_type: Default::default(),
        truncation_policy: TruncationPolicyConfig::bytes(/*limit*/ 10_000),
        supports_parallel_tool_calls: false,
        supports_image_detail_original: false,
        context_window: Some(272_000),
        max_context_window: None,
        auto_compact_token_limit: None,
        comp_hash: None,
        effective_context_window_percent: 95,
        experimental_supported_tools: Vec::new(),
    }
}
