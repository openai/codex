use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_config::McpServerConfig;
use codex_core::CodexThread;
use codex_core::ForkSnapshot;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_core::config::Config;
use codex_core::config::Constrained;
use codex_core::config::TokenBudgetConfig;
use codex_extension_api::ContentItemKind;
use codex_extension_api::ContextContributor;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::PromptFragment;
use codex_extension_api::ToolContributor;
use codex_extension_api::TurnContextContributionInput;
use codex_features::Feature;
use codex_history::RolloutItem;
use codex_login::CodexAuth;
use codex_models_manager::bundled_models_response;
use codex_models_manager::manager::RefreshStrategy;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem;
use codex_protocol::dynamic_tools::DynamicToolFunctionSpec;
use codex_protocol::dynamic_tools::DynamicToolResponse;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::mcp::ClientMcpExtensions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ApplyPatchToolType;
use codex_protocol::openai_models::ApprovalMessages;
use codex_protocol::openai_models::CollaborationModeMessages;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::ConfirmationPolicies;
use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelTokenBudgetConfig;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::MultiAgentMessages;
use codex_protocol::openai_models::MultiAgentModeMessages;
use codex_protocol::openai_models::MultiAgentRoleMessages;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::openai_models::ToolMessage;
use codex_protocol::openai_models::ToolMessages;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::CONTEXT_WINDOW_GUIDANCE_CLOSE_TAG;
use codex_protocol::protocol::CONTEXT_WINDOW_GUIDANCE_OPEN_TAG;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SafetyBufferingEvent;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::protocol::TurnSettingsUpdate;
use codex_protocol::protocol::TurnSettingsUpdateOutcome;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputEvent;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_protocol::user_input::UserInput;
use codex_tools::JsonToolOutput;
use codex_tools::ToolCall;
use codex_tools::ToolExecutor;
use codex_tools::ToolExecutorFuture;
use codex_tools::ToolName;
use codex_tools::ToolOutput;
use codex_tools::ToolSpec;
use codex_utils_image::data_url_from_bytes;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::truncate_text;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::recorded_apps_tool_calls;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_apply_patch_custom_tool_call;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_custom_tool_call;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_models_once;
use core_test_support::responses::mount_response_sequence;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::sse_completed;
use core_test_support::responses::sse_response;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use core_test_support::wait_for_mcp_server;
use image::GenericImageView;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use test_case::test_case;

use super::rmcp_client::remote_aware_environment_id;
use super::rmcp_client::remote_aware_stdio_server_bin;

mod code_mode_notifications;

const MODEL_A: &str = "step-settings-a";
const MODEL_B: &str = "step-settings-b";
const MODEL_C: &str = "step-settings-c";
const TURN_STATE_HEADER: &str = "x-codex-turn-state";

fn step_settings_models() -> Vec<ModelInfo> {
    let model = bundled_models_response()
        .expect("bundled models should parse")
        .models
        .into_iter()
        .find(|model| model.slug == "gpt-5.5")
        .expect("bundled gpt-5.5 model");
    [MODEL_A, MODEL_B, MODEL_C]
        .into_iter()
        .map(|slug| {
            let mut model = model.clone();
            // Tests add model-owned differences when they need to exercise
            // activation or an explicit safety restriction.
            model.slug = slug.to_string();
            model
        })
        .collect()
}

fn step_settings_test() -> TestCodexBuilder {
    test_codex().with_model(MODEL_A).with_config(move |config| {
        for feature in [
            Feature::StepModelSwitching,
            Feature::DefaultModeRequestUserInput,
            Feature::FastMode,
        ] {
            config
                .features
                .enable(feature)
                .expect("test config should allow feature update");
        }
        config.model_catalog = Some(ModelsResponse {
            models: step_settings_models(),
        });
        config.model_reasoning_effort = Some(ReasoningEffort::Low);
        config.model_reasoning_summary = Some(ReasoningSummary::Concise);
        config.service_tier = None;
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
        config.approvals_reviewer = ApprovalsReviewer::User;
    })
}

fn direct_tool_settings_test() -> TestCodexBuilder {
    step_settings_test().with_config(|config| {
        for model in &mut config.model_catalog.as_mut().expect("test models").models {
            model.tool_mode = Some(ToolMode::Direct);
            model.use_responses_lite = false;
            model.apply_patch_tool_type =
                (model.slug != MODEL_A).then_some(ApplyPatchToolType::Freeform);
        }
    })
}

fn advertises_apply_patch(request: &Value) -> bool {
    request["tools"]
        .as_array()
        .expect("provider tools")
        .iter()
        .any(|tool| tool["type"] == "custom" && tool["name"] == "apply_patch")
}

fn paused_response(response_id: &str, call_id: &str) -> String {
    sse(vec![
        ev_response_created(response_id),
        pause_call(call_id),
        ev_completed(response_id),
    ])
}

fn pause_call(call_id: &str) -> Value {
    ev_function_call(
        call_id,
        "request_user_input",
        &json!({
            "questions": [{
                "id": "continue",
                "header": "Continue",
                "question": "Continue after the settings update?",
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
    )
}

async fn start_paused_turn(thread: &CodexThread) -> Result<RequestUserInputEvent> {
    thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "pause before continuing".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    Ok(wait_for_event_match(thread, |event| match event {
        EventMsg::RequestUserInput(request) => Some(request.clone()),
        _ => None,
    })
    .await)
}

async fn answer_paused_turn(thread: &CodexThread, turn_id: &str) -> Result<()> {
    thread
        .submit(Op::UserInputAnswer {
            id: turn_id.to_string(),
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
    Ok(())
}

async fn submit_turn_settings(
    thread: &CodexThread,
    turn_id: &str,
    update: TurnSettingsUpdate,
) -> Result<TurnSettingsUpdateOutcome> {
    let (reply, outcome) = tokio::sync::oneshot::channel();
    thread
        .submit(Op::TurnSettings {
            turn_id: turn_id.to_string(),
            update,
            reply,
        })
        .await?;
    Ok(tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 10), outcome).await??)
}

async fn apply_turn_settings(
    thread: &CodexThread,
    turn_id: &str,
    update: TurnSettingsUpdate,
) -> Result<()> {
    assert_eq!(
        submit_turn_settings(thread, turn_id, update).await?,
        TurnSettingsUpdateOutcome::Applied
    );
    Ok(())
}

fn request_settings(request: &ResponsesRequest) -> Value {
    let body = request.body_json();
    json!({
        "model": body["model"],
        "reasoning": body["reasoning"],
        "service_tier": body.get("service_tier"),
    })
}

fn request_turn_id(request: &ResponsesRequest) -> String {
    let metadata: Value = serde_json::from_str(
        &request
            .header("x-codex-turn-metadata")
            .expect("request should include turn metadata"),
    )
    .expect("valid turn metadata");
    metadata["turn_id"]
        .as_str()
        .expect("request should include turn_id")
        .to_string()
}

// Dynamic tools return the original payload, so handler truncation cannot hide a recorder bug.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_result_history_keeps_originating_model_across_switch_and_replay() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let mut test = step_settings_test()
        .with_config(|config| {
            for model in &mut config.model_catalog.as_mut().expect("models").models {
                model.truncation_policy =
                    TruncationPolicyConfig::tokens(if model.slug == MODEL_B { 400 } else { 100 });
                model.supports_image_detail_original = model.slug == MODEL_B;
                model.use_responses_lite = false;
                model.input_modalities = if model.slug == MODEL_C {
                    vec![InputModality::Text]
                } else {
                    vec![InputModality::Text, InputModality::Image]
                };
            }
            config
                .features
                .enable(Feature::UnifiedImageBudget)
                .expect("enable unified image budget");
        })
        .build_with_auto_env(&server)
        .await?;
    let started = test
        .thread_manager
        .start_thread(StartThreadOptions {
            dynamic_tools: vec![DynamicToolSpec::Function(DynamicToolFunctionSpec {
                name: "diagnostics".to_string(),
                description: "Returns diagnostic text and a screenshot.".to_string(),
                input_schema: json!({"type": "object", "properties": {}}),
                defer_loading: false,
            })],
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?;
    test.codex = started.thread;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-a"),
                ev_function_call("call-a", "diagnostics", "{}"),
                ev_completed("resp-a"),
            ]),
            sse(vec![
                ev_response_created("resp-b"),
                ev_function_call("call-b", "diagnostics", "{}"),
                ev_completed("resp-b"),
            ]),
            paused_response("resp-before-a", "pause-before-a"),
            sse_completed("resp-a-again"),
            sse_completed("resp-next-turn-b"),
            sse_completed("resp-next-turn-a"),
            sse_completed("resp-text-only"),
            sse_completed("resp-images-again"),
        ],
    )
    .await;
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgba8(/*w*/ 2048, /*h*/ 2048)
        .write_to(&mut png, image::ImageFormat::Png)?;
    let text = "diagnostic line\n".repeat(500);
    let content_items = vec![
        DynamicToolCallOutputContentItem::InputText { text: text.clone() },
        DynamicToolCallOutputContentItem::InputImage {
            image_url: data_url_from_bytes("image/png", &png.into_inner()),
        },
    ];
    let response = DynamicToolResponse {
        content_items,
        success: true,
    };
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "collect diagnostics".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let mut raw_outputs = Vec::new();
    for call_id in ["call-a", "call-b"] {
        let call = wait_for_event_match(&test.codex, |event| match event {
            EventMsg::DynamicToolCallRequest(request) => Some(request.clone()),
            _ => None,
        })
        .await;
        assert_eq!(call.call_id, call_id);
        if call_id == "call-a" {
            // Apply B while A's result is pending in the same turn.
            assert_eq!(
                submit_turn_settings(
                    &test.codex,
                    &call.turn_id,
                    TurnSettingsUpdate {
                        model: Some(MODEL_B.to_string()),
                        ..Default::default()
                    },
                )
                .await?,
                TurnSettingsUpdateOutcome::Applied
            );
        }
        test.codex
            .submit(Op::DynamicToolResponse {
                id: call.call_id,
                response: response.clone(),
            })
            .await?;
        raw_outputs.push(
            wait_for_event_match(&test.codex, |event| match event {
                EventMsg::RawResponseItem(event)
                    if matches!(&event.item, ResponseItem::FunctionCallOutput { .. }) =>
                {
                    Some(serde_json::to_value(&event.item).expect("raw output"))
                }
                _ => None,
            })
            .await,
        );
    }
    let paused_request = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RequestUserInput(request) => Some(request.clone()),
        _ => None,
    })
    .await;

    let requests = responses.requests();
    assert_eq!(
        requests
            .iter()
            .map(|request| request.body_json()["model"].clone())
            .collect::<Vec<_>>(),
        vec![json!(MODEL_A), json!(MODEL_B), json!(MODEL_B)]
    );
    let turn_id = request_turn_id(&requests[0]);
    for request in &requests[1..] {
        assert_eq!(request_turn_id(request), turn_id);
    }
    for (index, (call_id, limit, dimensions)) in
        [("call-a", 100, (1600, 1600)), ("call-b", 400, (2048, 2048))]
            .into_iter()
            .enumerate()
    {
        let raw = &raw_outputs[index];
        assert_eq!(raw["output"][0]["text"], text);
        assert!(raw["id"].is_string());
        assert!(raw["internal_chat_message_metadata_passthrough"]["create_time"].is_number());
        let url = raw["output"][1]["image_url"].as_str().expect("tool image");
        let (_, data) = url.split_once(',').expect("image data URL");
        assert_eq!(
            image::load_from_memory(&BASE64_STANDARD.decode(data)?)?.dimensions(),
            dimensions
        );
        let mut expected = raw.clone();
        expected["output"][0]["text"] =
            json!(truncate_text(&text, TruncationPolicy::Tokens(limit) * 1.2));
        assert_eq!(requests[2].function_call_output(call_id), expected);
    }
    assert_eq!(
        requests[1].function_call_output("call-a"),
        requests[2].function_call_output("call-a")
    );

    apply_turn_settings(
        &test.codex,
        &paused_request.turn_id,
        TurnSettingsUpdate {
            model: Some(MODEL_A.to_string()),
            ..Default::default()
        },
    )
    .await?;
    answer_paused_turn(&test.codex, &paused_request.turn_id).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let outputs = ["call-a", "call-b"].map(|call_id| requests[2].function_call_output(call_id));
    assert_eq!(outputs[1]["output"][1]["detail"], "original");

    // Project for each receiving model without rewriting the prepared images.
    for model in [MODEL_B, MODEL_A, MODEL_C, MODEL_B] {
        test.codex
            .submit(Op::ThreadSettings {
                thread_settings: ThreadSettingsOverrides {
                    model: Some(model.to_string()),
                    ..Default::default()
                },
            })
            .await?;
        test.submit_text_turn("review previous diagnostics").await?;
    }
    let requests = responses.requests();
    let mut outputs_for_a = outputs.clone();
    outputs_for_a[1]["output"][1]["detail"] = json!("high");
    for (index, call_id) in ["call-a", "call-b"].iter().enumerate() {
        for request_index in [3, 5] {
            assert_eq!(
                requests[request_index].function_call_output(call_id),
                outputs_for_a[index]
            );
        }
        for request_index in [4, 7] {
            assert_eq!(
                requests[request_index].function_call_output(call_id),
                outputs[index]
            );
        }
        let text_output = requests[6].function_call_output(call_id);
        assert_eq!(
            text_output["output"][1],
            json!({"type": "input_text", "text": "image content omitted because you do not support image input"})
        );
    }
    assert_eq!(
        requests[3..]
            .iter()
            .map(|request| request.body_json()["model"].clone())
            .collect::<Vec<_>>(),
        vec![
            json!(MODEL_A),
            json!(MODEL_B),
            json!(MODEL_A),
            json!(MODEL_C),
            json!(MODEL_B)
        ]
    );
    assert_eq!(request_turn_id(&requests[3]), turn_id);
    assert_ne!(request_turn_id(&requests[5]), request_turn_id(&requests[4]));

    // Persistence and raw notifications retain the prepared, untruncated payload in append order.
    test.codex.shutdown_and_wait().await?;
    let rollout_path = test.codex.rollout_path().expect("rollout path");
    async fn saved_outputs(path: &std::path::Path) -> Result<Vec<(Value, Option<usize>)>> {
        let history = codex_rollout::RolloutRecorder::get_rollout_history(path).await?;
        Ok(history
            .get_rollout_items()
            .iter()
            .filter_map(|item| match item {
                RolloutItem::ResponseItem(envelope)
                    if matches!(&envelope.item, ResponseItem::FunctionCallOutput { call_id, .. }
                        if matches!(call_id.as_deref(), Some("call-a") | Some("call-b"))) =>
                {
                    Some((
                        serde_json::to_value(&envelope.item).expect("saved output"),
                        envelope
                            .metadata
                            .as_ref()
                            .and_then(|metadata| metadata.history_truncation_token_limit),
                    ))
                }
                _ => None,
            })
            .collect())
    }
    let recorded_outputs = saved_outputs(&rollout_path).await?;
    assert_eq!(
        recorded_outputs,
        raw_outputs
            .clone()
            .into_iter()
            .zip([Some(120), Some(480)])
            .collect::<Vec<_>>()
    );

    // Resume and fork under both limits. A's output must not grow under B, and
    // B's output must not shrink under A; image preparation and item IDs also survive.
    for model in [MODEL_A, MODEL_B] {
        let mut replay_config = test.config.clone();
        replay_config.model = Some(model.to_string());
        let resumed = test
            .thread_manager
            .resume_thread_from_rollout(
                replay_config.clone(),
                rollout_path.clone(),
                codex_core::test_support::auth_manager_from_auth(CodexAuth::from_api_key("dummy")),
                /*parent_trace*/ None,
                ClientMcpExtensions::default(),
            )
            .await?
            .thread;
        let forked = test
            .thread_manager
            .fork_thread(
                ForkSnapshot::Interrupted,
                StartThreadOptions::new(replay_config),
                rollout_path.clone(),
            )
            .await?
            .thread;
        for thread in [resumed, forked] {
            let replay = mount_sse_once(&server, sse_completed("resp-replay")).await;
            thread
                .submit(Op::ThreadSettings {
                    thread_settings: ThreadSettingsOverrides {
                        model: Some(model.to_string()),
                        ..Default::default()
                    },
                })
                .await?;
            thread
                .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                    text: "review saved diagnostics".to_string(),
                    text_elements: Vec::new(),
                }]))
                .await?;
            wait_for_event(&thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;
            let request = replay.single_request();
            assert_eq!(request.body_json()["model"], model);
            for (index, call_id) in ["call-a", "call-b"].iter().enumerate() {
                let expected = if model == MODEL_A {
                    &outputs_for_a[index]
                } else {
                    &outputs[index]
                };
                assert_eq!(request.function_call_output(call_id), *expected);
            }
            thread.shutdown_and_wait().await?;
            assert_eq!(
                saved_outputs(&thread.rollout_path().expect("replayed rollout")).await?,
                recorded_outputs
            );
        }
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn custom_tool_output_replay_preserves_originating_budget() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let test = step_settings_test()
        .with_model(MODEL_B)
        .with_config(|config| {
            config.features.enable(Feature::CodeMode).unwrap();
            for model in &mut config.model_catalog.as_mut().expect("models").models {
                model.truncation_policy =
                    TruncationPolicyConfig::tokens(if model.slug == MODEL_B { 400 } else { 100 });
                model.tool_mode = Some(ToolMode::CodeModeOnly);
                model.use_responses_lite = false;
            }
        })
        .build_with_auto_env(&server)
        .await?;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-custom"),
                ev_custom_tool_call(
                    "call-custom",
                    "exec",
                    "text('diagnostic line\\n'.repeat(500));",
                ),
                ev_completed("resp-custom"),
            ]),
            sse_completed("resp-done"),
        ],
    )
    .await;
    test.submit_text_turn("collect diagnostics").await?;
    let live_output = responses.requests()[1].custom_tool_call_output("call-custom");
    let text = "diagnostic line\n".repeat(500);
    let live_text = live_output["output"][1]["text"]
        .as_str()
        .expect("bounded custom output");
    assert!(live_text.len() < text.len());
    // A's smaller history budget would truncate this result again without the saved budget.
    assert_ne!(
        truncate_text(live_text, TruncationPolicy::Tokens(120)),
        live_text
    );

    test.codex.shutdown_and_wait().await?;
    let rollout_path = test.codex.rollout_path().expect("rollout path");
    let history = codex_rollout::RolloutRecorder::get_rollout_history(&rollout_path).await?;
    let saved = history
        .get_rollout_items()
        .iter()
        .find_map(|item| match item {
            RolloutItem::ResponseItem(envelope)
                if matches!(&envelope.item, ResponseItem::CustomToolCallOutput { call_id, .. } if call_id == "call-custom") =>
            {
                Some(envelope)
            }
            _ => None,
        })
        .expect("saved custom output");
    assert_eq!(
        saved
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.history_truncation_token_limit),
        Some(480)
    );
    assert_eq!(
        serde_json::to_value(&saved.item)?["output"][1]["text"],
        text
    );

    let mut replay_config = test.config.clone();
    replay_config.model = Some(MODEL_A.to_string());
    let resumed = test
        .thread_manager
        .resume_thread_from_rollout(
            replay_config.clone(),
            rollout_path.clone(),
            codex_core::test_support::auth_manager_from_auth(CodexAuth::from_api_key("dummy")),
            /*parent_trace*/ None,
            ClientMcpExtensions::default(),
        )
        .await?
        .thread;
    let forked = test
        .thread_manager
        .fork_thread(
            ForkSnapshot::Interrupted,
            StartThreadOptions::new(replay_config),
            rollout_path,
        )
        .await?
        .thread;
    for thread in [resumed, forked] {
        let replay = mount_sse_once(&server, sse_completed("resp-replay")).await;
        thread
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                text: "review saved diagnostics".to_string(),
                text_elements: Vec::new(),
            }]))
            .await?;
        wait_for_event(&thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;
        let request = replay.single_request();
        assert_eq!(request.body_json()["model"], MODEL_A);
        assert_eq!(request.custom_tool_call_output("call-custom"), live_output);
        thread.shutdown_and_wait().await?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum SettingsTarget {
    Thread,
    Turn,
}

#[test_case(SettingsTarget::Thread; "thread updates stay next-turn-only with the feature enabled")]
#[test_case(SettingsTarget::Turn; "turn updates leave future settings unchanged")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn settings_updates_preserve_turn_identity_and_target(target: SettingsTarget) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response_mock = mount_response_sequence(
        &server,
        vec![
            sse_response(paused_response("resp-1", "pause-turn"))
                .insert_header(TURN_STATE_HEADER, "original-turn-state"),
            sse_response(sse_completed("resp-2")),
            sse_response(sse_completed("resp-3")),
        ],
    )
    .await;
    let test = step_settings_test().build_with_auto_env(&server).await?;
    let original_settings = test.codex.thread_settings_snapshot().await;
    let request = start_paused_turn(&test.codex).await?;

    match target {
        SettingsTarget::Thread => {
            test.codex
                .submit(Op::ThreadSettings {
                    thread_settings: ThreadSettingsOverrides {
                        model: Some(MODEL_B.to_string()),
                        effort: Some(Some(ReasoningEffort::High)),
                        summary: Some(ReasoningSummary::Detailed),
                        service_tier: Some(Some(ServiceTier::Fast.request_value().to_string())),
                        ..Default::default()
                    },
                })
                .await?;
        }
        SettingsTarget::Turn => {
            let update = TurnSettingsUpdate {
                model: Some(MODEL_B.to_string()),
                effort: Some(Some(ReasoningEffort::High)),
                summary: Some(ReasoningSummary::Detailed),
                service_tier: Some(Some(ServiceTier::Fast.request_value().to_string())),
                ..Default::default()
            };
            assert_eq!(
                submit_turn_settings(&test.codex, "different-turn", update.clone()).await?,
                TurnSettingsUpdateOutcome::TargetUnavailable
            );
            assert_eq!(
                submit_turn_settings(&test.codex, &request.turn_id, update).await?,
                TurnSettingsUpdateOutcome::Applied
            );
        }
    }
    answer_paused_turn(&test.codex, &request.turn_id).await?;
    let mut settings_events = Vec::new();
    let mut new_turns = Vec::new();
    let completion = wait_for_event(&test.codex, |event| match event {
        EventMsg::ThreadSettingsApplied(event) => {
            settings_events.push(event.thread_settings.clone());
            false
        }
        EventMsg::TurnStarted(event) => {
            new_turns.push(event.turn_id.clone());
            false
        }
        EventMsg::Error(error) => panic!("settings activation failed: {}", error.message),
        EventMsg::TurnComplete(_) => true,
        _ => false,
    })
    .await;
    let EventMsg::TurnComplete(completion) = completion else {
        unreachable!("waited for turn completion")
    };
    assert_eq!(completion.turn_id, request.turn_id);
    assert_eq!(new_turns, Vec::<String>::new());

    let mut changed_settings = original_settings.clone();
    changed_settings.model = MODEL_B.to_string();
    changed_settings.reasoning_effort = Some(ReasoningEffort::High);
    changed_settings.reasoning_summary = Some(ReasoningSummary::Detailed);
    changed_settings.service_tier = Some(ServiceTier::Fast.request_value().to_string());
    changed_settings.collaboration_mode = changed_settings.collaboration_mode.with_updates(
        Some(MODEL_B.to_string()),
        Some(Some(ReasoningEffort::High)),
        /*developer_instructions*/ None,
    );
    let expected_future_settings = match target {
        SettingsTarget::Turn => {
            assert_eq!(settings_events, Vec::new());
            original_settings
        }
        SettingsTarget::Thread => {
            assert_eq!(settings_events, vec![changed_settings.clone()]);
            changed_settings
        }
    };
    assert_eq!(
        test.codex.thread_settings_snapshot().await,
        expected_future_settings
    );
    test.submit_text_turn("start the next turn").await?;

    let original_request_settings = json!({
        "model": MODEL_A,
        "reasoning": { "effort": "low", "summary": "concise" },
        "service_tier": null,
    });
    let changed_request_settings = json!({
        "model": MODEL_B,
        "reasoning": { "effort": "high", "summary": "detailed" },
        "service_tier": "priority",
    });
    let (continued_settings, next_turn_settings) = match target {
        SettingsTarget::Thread => (original_request_settings.clone(), changed_request_settings),
        SettingsTarget::Turn => (changed_request_settings, original_request_settings.clone()),
    };
    let requests = response_mock.requests();
    assert_eq!(
        requests.iter().map(request_settings).collect::<Vec<_>>(),
        vec![
            original_request_settings,
            continued_settings,
            next_turn_settings,
        ]
    );
    assert_eq!(request_turn_id(&requests[0]), request.turn_id);
    assert_eq!(request_turn_id(&requests[1]), request.turn_id);
    assert_ne!(request_turn_id(&requests[2]), request.turn_id);
    let session_id = requests[0]
        .header("session-id")
        .expect("initial request session id");
    assert_eq!(requests[1].header("session-id"), Some(session_id));
    assert_eq!(requests[0].header(TURN_STATE_HEADER), None);
    assert_eq!(
        requests[1].header(TURN_STATE_HEADER),
        Some("original-turn-state".to_string())
    );
    assert_eq!(requests[2].header(TURN_STATE_HEADER), None);
    let expected_switch_counts = match target {
        SettingsTarget::Thread => vec![0, 0, 1],
        SettingsTarget::Turn => vec![0, 1, 2],
    };
    assert_eq!(
        requests
            .iter()
            .map(|request| request
                .message_input_texts("developer")
                .iter()
                .filter(|text| text.contains("<model_switch>"))
                .count())
            .collect::<Vec<_>>(),
        expected_switch_counts
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn new_context_after_model_switch_uses_captured_extension_window() -> Result<()> {
    struct WindowContributor;

    impl ContextContributor for WindowContributor {
        fn contribute_turn_context<'a>(
            &'a self,
            input: TurnContextContributionInput<'a>,
        ) -> ExtensionFuture<'a, Vec<PromptFragment>> {
            Box::pin(async move {
                input
                    .model_context_window
                    .map(|window| {
                        PromptFragment::developer_policy(
                            format!("Extension window: {window} tokens."),
                            ContentItemKind("test.turn_context".to_string()),
                        )
                    })
                    .into_iter()
                    .collect()
            })
        }
    }

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            paused_response("resp-a", "pause-before-new-context"),
            sse(vec![
                ev_response_created("resp-b"),
                ev_function_call("new-window", "new_context", "{}"),
                ev_completed("resp-b"),
            ]),
            sse_completed("resp-new-window"),
        ],
    )
    .await;
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.prompt_contributor(Arc::new(WindowContributor));
    let test = step_settings_test()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            config.features.enable(Feature::TokenBudget).unwrap();
            config.model_context_window = None;
            for model in &mut config.model_catalog.as_mut().unwrap().models {
                model.context_window = None;
                model.max_context_window = Some(if model.slug == MODEL_A {
                    128_000
                } else {
                    256_000
                });
                model.effective_context_window_percent =
                    if model.slug == MODEL_A { 75 } else { 50 };
            }
        })
        .build_with_auto_env(&server)
        .await?;
    let paused = start_paused_turn(&test.codex).await?;
    assert_eq!(
        submit_turn_settings(
            &test.codex,
            &paused.turn_id,
            TurnSettingsUpdate {
                model: Some(MODEL_B.to_string()),
                ..Default::default()
            },
        )
        .await?,
        TurnSettingsUpdateOutcome::Applied
    );
    answer_paused_turn(&test.codex, &paused.turn_id).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = responses.requests();
    assert_eq!(
        requests
            .iter()
            .map(|request| request.body_json()["model"].clone())
            .collect::<Vec<_>>(),
        vec![json!(MODEL_A), json!(MODEL_B), json!(MODEL_B)]
    );
    assert!(requests[0].body_contains_text("Extension window: 96000 tokens."));
    assert!(requests[2].body_contains_text("Extension window: 128000 tokens."));
    assert!(!requests[2].body_contains_text("Extension window: 96000 tokens."));
    Ok(())
}

#[derive(Clone, Copy)]
enum TokenBudgetScenario {
    ModelDefaults,
    ExplicitDefaultTemplate,
    ReloadPreferences,
    DestinationWindowOnly,
    InitialWindowOnly,
    DestinationWithoutGuidance,
}

#[test_case(TokenBudgetScenario::ModelDefaults)]
#[test_case(TokenBudgetScenario::ExplicitDefaultTemplate)]
#[test_case(TokenBudgetScenario::ReloadPreferences)]
#[test_case(TokenBudgetScenario::DestinationWindowOnly)]
#[test_case(TokenBudgetScenario::InitialWindowOnly)]
#[test_case(TokenBudgetScenario::DestinationWithoutGuidance)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn active_model_switch_updates_core_context_from_captured_settings(
    scenario: TokenBudgetScenario,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let explicit_default_template =
        matches!(scenario, TokenBudgetScenario::ExplicitDefaultTemplate);
    let reload_user_config = matches!(scenario, TokenBudgetScenario::ReloadPreferences);
    let context_window_model = match scenario {
        TokenBudgetScenario::DestinationWindowOnly => Some(MODEL_B),
        TokenBudgetScenario::InitialWindowOnly => Some(MODEL_A),
        TokenBudgetScenario::ModelDefaults
        | TokenBudgetScenario::ExplicitDefaultTemplate
        | TokenBudgetScenario::ReloadPreferences
        | TokenBudgetScenario::DestinationWithoutGuidance => None,
    };
    let destination_has_guidance =
        !matches!(scenario, TokenBudgetScenario::DestinationWithoutGuidance);
    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            paused_response("resp-1", "pause-before-model-budget-switch"),
            paused_response("resp-2", "pause-after-model-budget-switch"),
            sse_completed("resp-3"),
        ],
    )
    .await;
    let test = step_settings_test()
        .with_pre_build_hook(move |home| {
            let config = if explicit_default_template {
                let default_template = TokenBudgetConfig::default().reminder_message_template;
                format!(
                    "[features.token_budget]\nenabled = true\nreminder_message_template = {default_template:?}\n"
                )
            } else {
                "[features.token_budget]\nenabled = true\n".to_string()
            };
            std::fs::write(home.join("config.toml"), config)
                .expect("write token-budget preferences");
        })
        .with_config(move |config| {
            config
                .features
                .enable(Feature::TokenBudget)
                .expect("enable token-budget feature");
            config
                .features
                .enable(Feature::MultiAgentV2)
                .expect("enable multi-agent V2");
            if context_window_model.is_some() {
                config.model_context_window = None;
            }
            for model in &mut config
                .model_catalog
                .as_mut()
                .expect("controlled model catalog")
                .models
            {
                let slug = model.slug.clone();
                let initial_model = slug == MODEL_A;
                if let Some(context_window_model) = context_window_model {
                    model.context_window = (slug == context_window_model).then_some(128_000);
                    model.max_context_window = None;
                }
                let messages = model.model_messages.as_mut().expect("model messages");
                messages.instructions_template = Some(format!("Instructions for {slug}."));
                messages.instructions_variables = None;
                messages.collaboration_modes = Some(CollaborationModeMessages {
                    default: Some(format!("Default collaboration for {slug}.")),
                    plan: None,
                });
                messages.multi_agent = Some(MultiAgentMessages {
                    role: Some(MultiAgentRoleMessages {
                        root: Some(format!("Root role for {slug}.")),
                        subagent: None,
                    }),
                    mode: Some(MultiAgentModeMessages {
                        explicit: Some(format!("Delegation policy for {slug}.")),
                        proactive: None,
                        hint_text: None,
                    }),
                });
                messages.approvals = Some(ApprovalMessages {
                    on_request: Some(format!("Approval instructions for {slug}.")),
                    on_request_auto_review: None,
                    never: None,
                    unless_trusted: None,
                });
                messages.token_budget =
                    (initial_model || destination_has_guidance).then(|| ModelTokenBudgetConfig {
                    enabled: false,
                    use_history_notes_extension: false,
                    reminder_threshold_tokens: if initial_model { 8_000 } else { 2_000 },
                    reminder_message_template: format!(
                        "Reminder for {slug}: {{n_remaining}} tokens remain."
                    ),
                    guidance_message: format!("Use {slug} token-budget guidance."),
                    auto_compact_fallback_prompt: format!("Save {slug} state before rollover."),
                    auto_compact_fallback_buffer_tokens: if initial_model {
                        16_000
                    } else {
                        4_000
                    },
                });
            }
        })
        .build_with_auto_env(&server)
        .await?;
    let request = start_paused_turn(&test.codex).await?;

    if reload_user_config {
        std::fs::write(
            test.codex_home_path().join("config.toml"),
            "[features.token_budget]\nenabled = true\nreminder_message_template = \"Reloaded reminder\"\n",
        )?;
        test.codex.submit(Op::ReloadUserConfig).await?;
    }

    apply_turn_settings(
        &test.codex,
        &request.turn_id,
        TurnSettingsUpdate {
            model: Some(MODEL_B.to_string()),
            ..Default::default()
        },
    )
    .await?;
    answer_paused_turn(&test.codex, &request.turn_id).await?;
    let request = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RequestUserInput(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    answer_paused_turn(&test.codex, &request.turn_id).await?;
    wait_for_event(&test.codex, |event| match event {
        EventMsg::Error(error) => panic!("settings activation failed: {}", error.message),
        EventMsg::TurnComplete(_) => true,
        _ => false,
    })
    .await;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(
        requests
            .iter()
            .map(|request| request.body_json()["model"].clone())
            .collect::<Vec<_>>(),
        vec![json!(MODEL_A), json!(MODEL_B), json!(MODEL_B)]
    );
    let initial_instructions = format!("Instructions for {MODEL_A}.");
    assert_eq!(requests[0].instructions_text(), initial_instructions);
    assert!(!requests[0].body_contains_text("<model_switch>"));
    for text in [
        format!("Root role for {MODEL_A}."),
        format!("Delegation policy for {MODEL_A}."),
    ] {
        assert!(requests[0].body_contains_text(&text));
    }
    for pair in requests.windows(/*size*/ 2) {
        assert!(
            pair[1].input().starts_with(&pair[0].input()),
            "context updates must be append-only"
        );
        assert_eq!(pair[1].instructions_text(), initial_instructions);
        assert_eq!(pair[1].body_json()["tools"], pair[0].body_json()["tools"]);
        assert_eq!(
            pair[1].message_input_texts("user"),
            pair[0].message_input_texts("user")
        );
    }
    for request in &requests[1..] {
        let developer_texts = request.message_input_texts("developer");
        let switches = developer_texts
            .iter()
            .filter(|text| text.contains("<model_switch>"))
            .collect::<Vec<_>>();
        assert_eq!(switches.len(), 1);
        assert!(switches[0].contains(&format!("Instructions for {MODEL_B}.")));
        assert!(
            !request.body_contains_text("<personality_spec>"),
            "model-switch instructions should not contain a personality update"
        );
        for text in [
            format!("Default collaboration for {MODEL_B}."),
            format!("Approval instructions for {MODEL_B}."),
            format!("Root role for {MODEL_B}."),
            format!("Delegation policy for {MODEL_B}."),
        ] {
            assert_eq!(
                developer_texts
                    .iter()
                    .filter(|message| message.contains(&text))
                    .count(),
                1
            );
        }
    }
    let initial_guidance = format!("Use {MODEL_A} token-budget guidance.");
    let initial_guidance_expected =
        !explicit_default_template && context_window_model != Some(MODEL_B);
    assert_eq!(
        requests[0].body_contains_text(&initial_guidance),
        initial_guidance_expected
    );
    let mut expected_guidance = Vec::new();
    if initial_guidance_expected {
        expected_guidance.push(format!(
            "{CONTEXT_WINDOW_GUIDANCE_OPEN_TAG}\n{initial_guidance}\n{CONTEXT_WINDOW_GUIDANCE_CLOSE_TAG}"
        ));
    }
    if !explicit_default_template
        && destination_has_guidance
        && context_window_model != Some(MODEL_A)
    {
        let replacement_notice = if initial_guidance_expected {
            "This context-window guidance replaces all previously provided context-window guidance.\n\n"
        } else {
            ""
        };
        expected_guidance.push(format!(
            "{CONTEXT_WINDOW_GUIDANCE_OPEN_TAG}\n{replacement_notice}Use {MODEL_B} token-budget guidance.\n{CONTEXT_WINDOW_GUIDANCE_CLOSE_TAG}"
        ));
    } else if initial_guidance_expected {
        expected_guidance.push(format!(
            "{CONTEXT_WINDOW_GUIDANCE_OPEN_TAG}\nThe previously provided context-window guidance no longer applies.\n{CONTEXT_WINDOW_GUIDANCE_CLOSE_TAG}"
        ));
    }
    for request in &requests[1..] {
        let guidance = request
            .message_input_texts("developer")
            .into_iter()
            .filter(|text| text.starts_with(CONTEXT_WINDOW_GUIDANCE_OPEN_TAG))
            .collect::<Vec<_>>();
        assert_eq!(
            guidance, expected_guidance,
            "preserve history and append the guidance transition only once"
        );
    }

    Ok(())
}

#[test_case(Some(ReasoningEffort::Ultra); "selected effort")]
#[test_case(None; "captured model default effort")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn active_model_switch_updates_multi_agent_policy_from_captured_effort(
    effort: Option<ReasoningEffort>,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let use_model_default = effort.is_none();
    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            paused_response("resp-1", "pause-before-effort-switch"),
            sse_completed("resp-2"),
        ],
    )
    .await;
    let test = step_settings_test()
        .with_config(move |config| {
            config
                .features
                .enable(Feature::MultiAgentV2)
                .expect("enable multi-agent V2");
            for model in &mut config
                .model_catalog
                .as_mut()
                .expect("controlled model catalog")
                .models
            {
                model
                    .model_messages
                    .as_mut()
                    .expect("model messages")
                    .multi_agent = None;
                model
                    .supported_reasoning_levels
                    .push(ReasoningEffortPreset {
                        effort: ReasoningEffort::Ultra,
                        description: "Ultra".to_string(),
                    });
                model.default_reasoning_level =
                    Some(if model.slug == MODEL_B && use_model_default {
                        ReasoningEffort::Ultra
                    } else {
                        ReasoningEffort::Low
                    });
            }
        })
        .build_with_auto_env(&server)
        .await?;
    let request = start_paused_turn(&test.codex).await?;
    assert_eq!(
        submit_turn_settings(
            &test.codex,
            &request.turn_id,
            TurnSettingsUpdate {
                model: Some(MODEL_B.to_string()),
                effort: Some(effort),
                ..Default::default()
            },
        )
        .await?,
        TurnSettingsUpdateOutcome::Applied
    );
    answer_paused_turn(&test.codex, &request.turn_id).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].body_json()["model"], MODEL_A);
    assert_eq!(requests[1].body_json()["model"], MODEL_B);
    assert_eq!(requests[1].body_json()["reasoning"]["effort"], "xhigh");
    let proactive_text = "Proactive multi-agent delegation is active.";
    assert!(!requests[0].body_contains_text(proactive_text));
    assert!(requests[1].body_contains_text(proactive_text));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_confirmation_policy_follows_step_model_changes() -> Result<()> {
    skip_if_no_network!(Ok(()));

    const BROWSER_POLICY_A: &str = "  # Browser policy A\r\n{literal}\n";
    const BROWSER_POLICY_B: &str = "\t# Browser policy B\n<raw> & café\r\n ";
    const COMPUTER_POLICY_A: &str = "\t# Native policy A\n{{literal}}\r\n";
    const COMPUTER_POLICY_B: &str = "  # Native policy B\r\n<computer> ${native}\n ";
    const BROWSER_ONLY_MODEL: &str = "policy-browser-only";
    const COMPUTER_ONLY_MODEL: &str = "policy-computer-only";
    let server = start_mock_server().await;
    AppsTestServer::mount(&server).await?;
    let policy_call = |response_id: &str, call_id: &str| {
        sse(vec![
            ev_response_created(response_id),
            ev_function_call_with_namespace(
                call_id,
                "mcp__node_repl",
                "calendar_list_events",
                "{}",
            ),
            ev_completed(response_id),
        ])
    };
    mount_sse_sequence(
        &server,
        vec![
            policy_call("resp-1", "policy-a"),
            sse_completed("resp-2"),
            sse(vec![
                ev_response_created("resp-3"),
                ev_function_call_with_namespace(
                    "policy-a-pending",
                    "mcp__node_repl",
                    "calendar_create_event",
                    &json!({
                        "title": "Policy snapshot test",
                        "starts_at": "2026-08-26T12:00:00Z",
                    })
                    .to_string(),
                ),
                ev_completed("resp-3"),
            ]),
            policy_call("resp-4", "policy-b"),
            sse_completed("resp-5"),
            policy_call("resp-6", "browser-only"),
            sse_completed("resp-7"),
            policy_call("resp-8", "computer-only"),
            sse_completed("resp-9"),
            policy_call("resp-10", "no-policy"),
            sse_completed("resp-11"),
        ],
    )
    .await;
    let mcp_url = format!("{}/api/codex/ps/mcp", server.uri());
    let test = step_settings_test()
        .with_config(move |config| {
            config
                .features
                .disable(Feature::ToolCallMcpElicitation)
                .expect("disable MCP elicitation for the approval barrier");
            let models = &mut config.model_catalog.as_mut().expect("test models").models;
            for slug in [BROWSER_ONLY_MODEL, COMPUTER_ONLY_MODEL] {
                let mut model = models[0].clone();
                model.slug = slug.to_string();
                models.push(model);
            }
            for model in models {
                let messages = model
                    .model_messages
                    .as_mut()
                    .expect("bundled model messages");
                messages.confirmation_policies = match model.slug.as_str() {
                    MODEL_A => Some(ConfirmationPolicies {
                        browser_use: Some(BROWSER_POLICY_A.to_string()),
                        computer_use: Some(COMPUTER_POLICY_A.to_string()),
                    }),
                    MODEL_B => Some(ConfirmationPolicies {
                        browser_use: Some(BROWSER_POLICY_B.to_string()),
                        computer_use: Some(COMPUTER_POLICY_B.to_string()),
                    }),
                    BROWSER_ONLY_MODEL => Some(ConfirmationPolicies {
                        browser_use: Some(BROWSER_POLICY_B.to_string()),
                        computer_use: None,
                    }),
                    COMPUTER_ONLY_MODEL => Some(ConfirmationPolicies {
                        browser_use: None,
                        computer_use: Some(COMPUTER_POLICY_B.to_string()),
                    }),
                    MODEL_C => None,
                    _ => unreachable!("unexpected test model"),
                };
            }
            let node_repl: McpServerConfig = serde_json::from_value(json!({
                "url": mcp_url,
                "tools": {
                    "calendar_create_event": {
                        "approval_mode": "prompt",
                    },
                },
            }))
            .expect("valid test MCP server");
            config
                .mcp_servers
                .set(HashMap::from([("node_repl".to_string(), node_repl)]))
                .expect("configure test MCP server");
        })
        .build_with_auto_env(&server)
        .await?;
    wait_for_mcp_server(&test.codex, "node_repl").await?;
    test.submit_text_turn("call the tool with model A").await?;

    let request = start_paused_turn(&test.codex).await?;
    assert_eq!(request.call_id, "policy-a-pending");
    // The pending call must retain model A's policies after this settings update.
    apply_turn_settings(
        &test.codex,
        &request.turn_id,
        TurnSettingsUpdate {
            model: Some(MODEL_B.to_string()),
            ..Default::default()
        },
    )
    .await?;
    test.codex
        .submit(Op::UserInputAnswer {
            id: request.turn_id.clone(),
            response: RequestUserInputResponse {
                answers: HashMap::from([(
                    request.questions[0].id.clone(),
                    RequestUserInputAnswer {
                        answers: vec!["Allow".to_string()],
                    },
                )]),
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    for model in [BROWSER_ONLY_MODEL, COMPUTER_ONLY_MODEL, MODEL_C] {
        core_test_support::submit_thread_settings(
            &test.codex,
            ThreadSettingsOverrides {
                model: Some(model.to_string()),
                ..Default::default()
            },
        )
        .await?;
        test.submit_text_turn("call the tool").await?;
    }

    assert_eq!(
        recorded_apps_tool_calls(&server)
            .await
            .into_iter()
            .map(|call| {
                let meta = &call["params"]["_meta"];
                (
                    meta["callId"].clone(),
                    meta["openai/confirmation_policies"].clone(),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (
                json!("policy-a"),
                json!({
                    "browser_use": BROWSER_POLICY_A,
                    "computer_use": COMPUTER_POLICY_A,
                })
            ),
            (
                json!("policy-a-pending"),
                json!({
                    "browser_use": BROWSER_POLICY_A,
                    "computer_use": COMPUTER_POLICY_A,
                })
            ),
            (
                json!("policy-b"),
                json!({
                    "browser_use": BROWSER_POLICY_B,
                    "computer_use": COMPUTER_POLICY_B,
                })
            ),
            (
                json!("browser-only"),
                json!({"browser_use": BROWSER_POLICY_B})
            ),
            (
                json!("computer-only"),
                json!({"computer_use": COMPUTER_POLICY_B})
            ),
            (json!("no-policy"), json!({})),
        ],
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn captured_model_replans_tools_and_retains_the_issuing_request_router() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let call_id = "delayed-b-patch";
    let file_name = "retained-model.txt";
    let patch =
        format!("*** Begin Patch\n*** Add File: {file_name}\n+written by B\n*** End Patch\n");
    let (release_patch, patch_gate) = tokio::sync::oneshot::channel();
    let (streaming_server, _) = start_streaming_sse_server(vec![
        vec![StreamingSseChunk {
            gate: None,
            body: paused_response("resp-a-initial", "pause-a"),
        }],
        vec![
            StreamingSseChunk {
                gate: None,
                body: sse(vec![ev_response_created("resp-b"), pause_call("pause-b")]),
            },
            StreamingSseChunk {
                gate: Some(patch_gate),
                body: sse(vec![
                    ev_apply_patch_custom_tool_call(call_id, &patch),
                    ev_completed("resp-b"),
                ]),
            },
        ],
        vec![StreamingSseChunk {
            gate: None,
            body: sse_completed("resp-a"),
        }],
    ])
    .await;
    let server = start_mock_server().await;
    let model_api_url = format!("{}/v1", streaming_server.uri());
    let test = direct_tool_settings_test()
        .with_config(move |config| {
            for feature in [Feature::ShellTool, Feature::UnifiedExec] {
                config.features.enable(feature).expect("enable shell tools");
            }
            for model in &mut config.model_catalog.as_mut().expect("models").models {
                model.shell_type = if model.slug == MODEL_B {
                    ConfigShellToolType::Disabled
                } else {
                    ConfigShellToolType::UnifiedExec
                };
            }
            // Use the gated model response with the automatically selected executor.
            config.model_provider.base_url = Some(model_api_url);
            config
                .permissions
                .set_permission_profile(PermissionProfile::Disabled)
                .expect("test permissions");
        })
        .build_with_auto_env(&server)
        .await?;
    let paused = start_paused_turn(&test.codex).await?;
    apply_turn_settings(
        &test.codex,
        &paused.turn_id,
        TurnSettingsUpdate {
            model: Some(MODEL_B.to_string()),
            ..Default::default()
        },
    )
    .await?;
    answer_paused_turn(&test.codex, &paused.turn_id).await?;
    let paused = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RequestUserInput(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    assert_eq!(paused.call_id, "pause-b");
    apply_turn_settings(
        &test.codex,
        &paused.turn_id,
        TurnSettingsUpdate {
            model: Some(MODEL_A.to_string()),
            ..Default::default()
        },
    )
    .await?;
    // B's response issues its patch after A is active, so dispatch must retain B's router.
    release_patch.send(()).expect("release B's patch call");
    answer_paused_turn(&test.codex, &paused.turn_id).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = streaming_server
        .requests()
        .await
        .iter()
        .map(|body| serde_json::from_slice::<Value>(body))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        requests
            .iter()
            .map(|request| request["model"].clone())
            .collect::<Vec<_>>(),
        vec![json!(MODEL_A), json!(MODEL_B), json!(MODEL_A)],
    );
    assert_eq!(
        requests
            .iter()
            .map(advertises_apply_patch)
            .collect::<Vec<_>>(),
        vec![false, true, false],
    );
    assert_eq!(
        requests
            .iter()
            .map(|request| {
                request["tools"]
                    .as_array()
                    .expect("provider tools")
                    .iter()
                    .filter_map(|tool| tool["name"].as_str())
                    .filter(|name| matches!(*name, "exec_command" | "write_stdin"))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>(),
        vec![
            vec!["exec_command", "write_stdin"],
            vec![],
            vec!["exec_command", "write_stdin"]
        ],
    );
    assert!(
        requests[2]["input"]
            .as_array()
            .expect("provider input")
            .iter()
            .any(|item| {
                item["type"] == "custom_tool_call_output" && item["call_id"] == call_id
            })
    );
    assert_eq!(
        test.fs()
            .read_file_text(
                &test.workspace_path_uri(file_name)?,
                Default::default(),
                /*sandbox*/ None,
            )
            .await?,
        "written by B\n",
    );
    streaming_server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn captured_model_enables_and_executes_code_mode() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            paused_response("resp-a", "pause-a"),
            sse(vec![
                ev_response_created("resp-b"),
                ev_custom_tool_call("code-b", "exec", "const result = await tools.view_image({path: 'step.png', detail: 'original'}); image(result)"),
                ev_completed("resp-b"),
            ]),
            sse_completed("resp-b-result"),
        ],
    )
    .await;
    let test = direct_tool_settings_test()
        .with_config(|config| {
            config
                .permissions
                .set_permission_profile(PermissionProfile::Disabled)
                .expect("test permissions");
            for model in &mut config.model_catalog.as_mut().expect("models").models {
                model.input_modalities = vec![codex_protocol::openai_models::InputModality::Text];
                model.supports_image_detail_original = false;
                if model.slug == MODEL_B {
                    model
                        .input_modalities
                        .push(codex_protocol::openai_models::InputModality::Image);
                    model.supports_image_detail_original = true;
                    model.tool_mode = Some(ToolMode::CodeModeOnly);
                }
            }
        })
        .build_with_auto_env(&server)
        .await?;
    use base64::Engine;
    let png = base64::engine::general_purpose::STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==")?;
    test.fs()
        .write_file(
            &test.workspace_path_uri("step.png")?,
            png,
            Default::default(),
            /*sandbox*/ None,
        )
        .await?;
    let paused = start_paused_turn(&test.codex).await?;
    apply_turn_settings(
        &test.codex,
        &paused.turn_id,
        TurnSettingsUpdate {
            model: Some(MODEL_B.to_string()),
            ..Default::default()
        },
    )
    .await?;
    answer_paused_turn(&test.codex, &paused.turn_id).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = responses.requests();
    assert_eq!(
        requests
            .iter()
            .map(|request| request.body_json()["model"].clone())
            .collect::<Vec<_>>(),
        vec![json!(MODEL_A), json!(MODEL_B), json!(MODEL_B)],
    );
    assert_eq!(
        requests
            .iter()
            .map(|request| {
                request.body_json()["tools"]
                    .as_array()
                    .expect("provider tools")
                    .iter()
                    .any(|tool| tool["type"] == "custom" && tool["name"] == "exec")
            })
            .collect::<Vec<_>>(),
        vec![false, true, true],
    );
    let output = requests[2].custom_tool_call_output("code-b");
    assert_eq!(
        output["output"]
            .as_array()
            .expect("Code Mode output items")
            .last()
            .expect("Code Mode image output")["detail"],
        json!("original"),
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn response_metadata_uses_the_captured_model_after_a_turn_update() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut created = ev_response_created("resp-2");
    created["safety_buffering"] = json!({
        "use_cases": ["cyber"],
        "reasons": ["policy-check"],
        "retry_model": MODEL_C,
    });
    let response_mock = mount_response_sequence(
        &server,
        vec![
            sse_response(paused_response("resp-1", "pause-turn")),
            sse_response(sse(vec![created, ev_completed("resp-2")]))
                .insert_header("OpenAI-Model", MODEL_B),
        ],
    )
    .await;
    let test = step_settings_test().build_with_auto_env(&server).await?;
    let request = start_paused_turn(&test.codex).await?;
    apply_turn_settings(
        &test.codex,
        &request.turn_id,
        TurnSettingsUpdate {
            model: Some(MODEL_B.to_string()),
            ..Default::default()
        },
    )
    .await?;
    answer_paused_turn(&test.codex, &request.turn_id).await?;

    let mut reroutes = Vec::new();
    let mut buffering_events = Vec::new();
    wait_for_event(&test.codex, |event| match event {
        EventMsg::ModelReroute(event) => {
            reroutes.push(event.clone());
            false
        }
        EventMsg::SafetyBuffering(event) => {
            buffering_events.push(event.clone());
            false
        }
        EventMsg::Error(error) => panic!("sampling failed: {}", error.message),
        EventMsg::TurnComplete(_) => true,
        _ => false,
    })
    .await;

    assert_eq!(
        response_mock
            .requests()
            .iter()
            .map(|request| request.body_json()["model"].clone())
            .collect::<Vec<_>>(),
        vec![json!(MODEL_A), json!(MODEL_B)]
    );
    // B's matching response header is not a reroute from the turn's initial A.
    // Buffering metadata likewise belongs to the captured B step.
    assert_eq!(
        (reroutes, buffering_events),
        (
            vec![],
            vec![SafetyBufferingEvent {
                model: MODEL_B.to_string(),
                use_cases: vec!["cyber".to_string()],
                reasons: vec!["policy-check".to_string()],
                show_buffering_ui: true,
                faster_model: Some(MODEL_C.to_string()),
            }],
        )
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sparse_updates_preserve_divergent_active_and_future_models() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            paused_response("resp-1", "pause-first-step"),
            paused_response("resp-2", "pause-second-step"),
            sse_completed("resp-3"),
            sse_completed("resp-4"),
        ],
    )
    .await;
    let test = step_settings_test().build_with_auto_env(&server).await?;
    let request = start_paused_turn(&test.codex).await?;

    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            model: Some(MODEL_B.to_string()),
            ..Default::default()
        },
    )
    .await?;
    apply_turn_settings(
        &test.codex,
        &request.turn_id,
        TurnSettingsUpdate {
            model: Some(MODEL_C.to_string()),
            effort: Some(Some(ReasoningEffort::High)),
            ..Default::default()
        },
    )
    .await?;
    answer_paused_turn(&test.codex, &request.turn_id).await?;
    let second_request = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RequestUserInput(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    assert_eq!(second_request.turn_id, request.turn_id);
    assert_eq!(second_request.call_id, "pause-second-step");

    test.codex
        .submit(Op::ThreadSettings {
            thread_settings: ThreadSettingsOverrides {
                service_tier: Some(Some(ServiceTier::Fast.request_value().to_string())),
                ..Default::default()
            },
        })
        .await?;
    apply_turn_settings(
        &test.codex,
        &request.turn_id,
        TurnSettingsUpdate {
            service_tier: Some(Some(ServiceTier::Fast.request_value().to_string())),
            ..Default::default()
        },
    )
    .await?;
    let durable_settings = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::ThreadSettingsApplied(event) => Some(event.thread_settings.clone()),
        _ => None,
    })
    .await;
    assert_eq!(durable_settings.model, MODEL_B);
    assert_eq!(
        durable_settings.reasoning_effort,
        Some(ReasoningEffort::Low)
    );
    assert_eq!(
        durable_settings.service_tier,
        Some(ServiceTier::Fast.request_value().to_string())
    );
    answer_paused_turn(&test.codex, &second_request.turn_id).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.submit_text_turn("start the next turn").await?;

    let requests = response_mock.requests();
    assert_eq!(
        requests.iter().map(request_settings).collect::<Vec<_>>(),
        vec![
            json!({
                "model": MODEL_A,
                "reasoning": { "effort": "low", "summary": "concise" },
                "service_tier": null,
            }),
            json!({
                "model": MODEL_C,
                "reasoning": { "effort": "high", "summary": "concise" },
                "service_tier": null,
            }),
            json!({
                "model": MODEL_C,
                "reasoning": { "effort": "high", "summary": "concise" },
                "service_tier": "priority",
            }),
            json!({
                "model": MODEL_B,
                "reasoning": { "effort": "low", "summary": "concise" },
                "service_tier": "priority",
            }),
        ]
    );
    assert_eq!(
        requests[..3]
            .iter()
            .map(request_turn_id)
            .collect::<Vec<_>>(),
        vec![request.turn_id.clone(); 3]
    );
    assert_ne!(request_turn_id(&requests[3]), request.turn_id);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn turn_settings_do_not_target_idle_or_finished_turns() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![sse_completed("resp-1"), sse_completed("resp-2")],
    )
    .await;
    let test = step_settings_test().build_with_auto_env(&server).await?;

    for (discarded_model, model) in [(MODEL_C, MODEL_B), (MODEL_A, MODEL_C)] {
        let turn_id = response_mock
            .requests()
            .last()
            .map(request_turn_id)
            .unwrap_or_else(|| "never-started".to_string());
        let before = test.codex.thread_settings_snapshot().await;
        assert_eq!(
            submit_turn_settings(
                &test.codex,
                &turn_id,
                TurnSettingsUpdate {
                    model: Some(discarded_model.to_string()),
                    ..Default::default()
                }
            )
            .await?,
            TurnSettingsUpdateOutcome::TargetUnavailable
        );
        assert_eq!(test.codex.thread_settings_snapshot().await, before);
        core_test_support::submit_thread_settings(
            &test.codex,
            ThreadSettingsOverrides {
                model: Some(model.to_string()),
                ..Default::default()
            },
        )
        .await?;
        test.submit_text_turn("start the next turn").await?;
    }

    assert_eq!(
        response_mock
            .requests()
            .iter()
            .map(request_settings)
            .collect::<Vec<_>>(),
        vec![
            json!({
                "model": MODEL_B,
                "reasoning": { "effort": "low", "summary": "concise" },
                "service_tier": null,
            }),
            json!({
                "model": MODEL_C,
                "reasoning": { "effort": "low", "summary": "concise" },
                "service_tier": null,
            }),
        ]
    );

    Ok(())
}

#[test_case(None, "detailed"; "unset summary follows the destination model")]
#[test_case(Some(ReasoningSummary::Concise), "concise"; "explicit summary is preserved")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_activation_uses_destination_metadata_defaults(
    configured_summary: Option<ReasoningSummary>,
    expected_summary: &str,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            paused_response("resp-1", "pause-before-model-change"),
            paused_response("resp-2", "pause-before-restoring-model"),
            sse_completed("resp-3"),
        ],
    )
    .await;
    let test = step_settings_test()
        .with_config(move |config| {
            config.model_context_window = None;
            config.model_auto_compact_token_limit = None;
            config.model_reasoning_summary = configured_summary;
            config.service_tier = Some(ServiceTier::Fast.request_value().to_string());
            config.base_instructions = None;
            for model in &mut config
                .model_catalog
                .as_mut()
                .expect("controlled model catalog")
                .models
            {
                model.context_window = Some(256_000);
                model.auto_compact_token_limit = Some(200_000);
                model.default_reasoning_summary = ReasoningSummary::Concise;
                if model.slug == MODEL_B {
                    model.context_window = Some(128_000);
                    model.auto_compact_token_limit = Some(100_000);
                    model.default_reasoning_summary = ReasoningSummary::Detailed;
                    model
                        .service_tiers
                        .retain(|tier| tier.id != ServiceTier::Fast.request_value());
                    model
                        .model_messages
                        .as_mut()
                        .expect("model instruction metadata")
                        .instructions_template =
                        Some("Destination model instructions.".to_string());
                }
            }
        })
        .build_with_auto_env(&server)
        .await?;
    let request = start_paused_turn(&test.codex).await?;

    apply_turn_settings(
        &test.codex,
        &request.turn_id,
        TurnSettingsUpdate {
            model: Some(MODEL_B.to_string()),
            ..Default::default()
        },
    )
    .await?;
    answer_paused_turn(&test.codex, &request.turn_id).await?;
    let paused = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RequestUserInput(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    // B cannot use the requested tier. Switching back must recover that
    // selection and preserve an unset summary, not reuse B's effective values.
    apply_turn_settings(
        &test.codex,
        &request.turn_id,
        TurnSettingsUpdate {
            model: Some(MODEL_A.to_string()),
            ..Default::default()
        },
    )
    .await?;
    answer_paused_turn(&test.codex, &paused.turn_id).await?;
    wait_for_event(&test.codex, |event| match event {
        EventMsg::Error(error) => panic!("model activation failed: {}", error.message),
        EventMsg::TurnComplete(_) => true,
        _ => false,
    })
    .await;

    let requests = response_mock.requests();
    assert_eq!(
        requests.iter().map(request_settings).collect::<Vec<_>>(),
        vec![
            json!({
                "model": MODEL_A,
                "reasoning": { "effort": "low", "summary": "concise" },
                "service_tier": "priority",
            }),
            json!({
                "model": MODEL_B,
                "reasoning": { "effort": "low", "summary": expected_summary },
                "service_tier": null,
            }),
            json!({
                "model": MODEL_A,
                "reasoning": { "effort": "low", "summary": "concise" },
                "service_tier": "priority",
            }),
        ]
    );
    assert_eq!(
        requests.iter().map(request_turn_id).collect::<Vec<_>>(),
        vec![request.turn_id; 3],
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn request_user_input_async_description_follows_mid_turn_model_changes() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            paused_response("resp-1", "pause-before-async-message-model-change"),
            sse_completed("resp-2"),
        ],
    )
    .await;
    let test = step_settings_test()
        .with_config(|config| {
            for model in &mut config
                .model_catalog
                .as_mut()
                .expect("controlled model catalog")
                .models
            {
                model
                    .experimental_supported_tools
                    .push("send_user_message_async".to_string());
                model
                    .model_messages
                    .as_mut()
                    .expect("model instruction metadata")
                    .tools = Some(ToolMessages {
                    send_user_message_async: Some(ToolMessage {
                        description: Some(format!("Async message description for {}.", model.slug)),
                    }),
                });
            }
        })
        .build_with_auto_env(&server)
        .await?;
    let paused = start_paused_turn(&test.codex).await?;
    apply_turn_settings(
        &test.codex,
        &paused.turn_id,
        TurnSettingsUpdate {
            model: Some(MODEL_B.to_string()),
            ..Default::default()
        },
    )
    .await?;
    answer_paused_turn(&test.codex, &paused.turn_id).await?;
    wait_for_event(&test.codex, |event| match event {
        EventMsg::Error(error) => panic!("model activation failed: {}", error.message),
        EventMsg::TurnComplete(_) => true,
        _ => false,
    })
    .await;

    assert_eq!(
        response_mock
            .requests()
            .iter()
            .map(|request| {
                let body = request.body_json();
                let tool = body["tools"]
                    .as_array()
                    .expect("request tools")
                    .iter()
                    .find(|tool| tool["name"] == "request_user_input_async")
                    .expect("async message tool");
                json!({"model": body["model"], "description": tool["description"]})
            })
            .collect::<Vec<_>>(),
        [MODEL_A, MODEL_B]
            .map(|model| json!({
                "model": model,
                "description": format!("Async message description for {model}."),
            }))
            .to_vec(),
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn persistent_instructions_follow_mid_turn_model_changes() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            paused_response("resp-1", "pause-before-persistent-model-change"),
            sse_completed("resp-2"),
        ],
    )
    .await;
    let test = step_settings_test()
        .with_config(|config| {
            config.model_reasoning_effort = Some(ReasoningEffort::Persistent);
            for model in &mut config
                .model_catalog
                .as_mut()
                .expect("controlled model catalog")
                .models
            {
                model
                    .supported_reasoning_levels
                    .push(ReasoningEffortPreset {
                        effort: ReasoningEffort::Persistent,
                        description: ReasoningEffort::Persistent.to_string(),
                    });
                model
                    .model_messages
                    .as_mut()
                    .expect("model instruction metadata")
                    .persistent_instructions =
                    Some(format!("Persistent instructions for {}.", model.slug));
            }
        })
        .build_with_auto_env(&server)
        .await?;
    let paused = start_paused_turn(&test.codex).await?;
    apply_turn_settings(
        &test.codex,
        &paused.turn_id,
        TurnSettingsUpdate {
            model: Some(MODEL_B.to_string()),
            ..Default::default()
        },
    )
    .await?;
    answer_paused_turn(&test.codex, &paused.turn_id).await?;
    wait_for_event(&test.codex, |event| match event {
        EventMsg::Error(error) => panic!("model activation failed: {}", error.message),
        EventMsg::TurnComplete(_) => true,
        _ => false,
    })
    .await;

    let initial =
        format!("<persistent_mode>\nPersistent instructions for {MODEL_A}.\n</persistent_mode>");
    let update = format!(
        "<persistent_mode>\nThese persistent-mode instructions replace all previously provided persistent-mode instructions.\n\nPersistent instructions for {MODEL_B}.\n</persistent_mode>"
    );
    let requests = response_mock.requests();
    assert_eq!(
        requests
            .iter()
            .map(|request| {
                let instructions = request
                    .message_input_texts("developer")
                    .into_iter()
                    .filter(|text| text.starts_with("<persistent_mode>"))
                    .collect::<Vec<_>>();
                json!({"model": request.body_json()["model"], "instructions": instructions})
            })
            .collect::<Vec<_>>(),
        vec![
            json!({"model": MODEL_A, "instructions": [initial]}),
            json!({"model": MODEL_B, "instructions": [initial, update]}),
        ]
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn turn_settings_rejection_preserves_independent_future_settings() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            paused_response("resp-1", "pause-before-node-repl-restriction"),
            sse_completed("resp-2"),
            sse_completed("resp-3"),
        ],
    )
    .await;
    let test = step_settings_test()
        .with_config(|config| {
            for model in &mut config
                .model_catalog
                .as_mut()
                .expect("controlled model catalog")
                .models
            {
                model.node_repl_disabled = model.slug == MODEL_B;
            }
        })
        .build_with_auto_env(&server)
        .await?;
    let request = start_paused_turn(&test.codex).await?;
    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            model: Some(MODEL_B.to_string()),
            ..Default::default()
        },
    )
    .await?;
    let future = test.codex.thread_settings_snapshot().await;
    assert_eq!(
        submit_turn_settings(
            &test.codex,
            &request.turn_id,
            TurnSettingsUpdate {
                model: Some(MODEL_B.to_string()),
                ..Default::default()
            }
        )
        .await?,
        TurnSettingsUpdateOutcome::Rejected {
            reason: "the destination changes the admitted node REPL availability restriction"
                .to_string(),
        }
    );
    assert_eq!(test.codex.thread_settings_snapshot().await, future);
    answer_paused_turn(&test.codex, &request.turn_id).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.submit_text_turn("admit the next turn").await?;

    let requests = response_mock.requests();
    assert_eq!(
        requests.iter().map(request_settings).collect::<Vec<_>>(),
        [MODEL_A, MODEL_A, MODEL_B]
            .into_iter()
            .map(|model| {
                json!({
                    "model": model,
                    "reasoning": { "effort": "low", "summary": "concise" },
                    "service_tier": null,
                })
            })
            .collect::<Vec<_>>()
    );
    assert_eq!(request_turn_id(&requests[1]), request.turn_id);
    assert_ne!(request_turn_id(&requests[2]), request.turn_id);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn request_preference_activation_keeps_admitted_model_metadata() -> Result<()> {
    skip_if_no_network!(Ok(()));

    // This test owns both catalog responses. The generic mock-server helper
    // installs an extra one-shot /models response that would shift the sequence.
    let server = wiremock::MockServer::start().await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            paused_response("resp-1", "pause-before-models-refresh"),
            sse_completed("resp-2"),
            sse_completed("resp-3"),
        ],
    )
    .await;
    let mut models = step_settings_models();
    for model in &mut models {
        model.default_reasoning_summary = ReasoningSummary::Concise;
    }
    let initial_catalog = mount_models_once(
        &server,
        ModelsResponse {
            models: models.clone(),
        },
    )
    .await;
    let test = step_settings_test()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(|config| {
            // Use the real refreshable models manager, and retain an unset
            // preference so the selected model's default is observable.
            config.model_catalog = None;
            config.model_reasoning_summary = None;
        })
        .build_with_auto_env(&server)
        .await?;
    let request = start_paused_turn(&test.codex).await?;
    assert_eq!(initial_catalog.requests().len(), 1);

    for model in &mut models {
        model.default_reasoning_summary = ReasoningSummary::Detailed;
    }
    let refresh = mount_models_once(&server, ModelsResponse { models }).await;
    test.thread_manager
        .get_models_manager()
        .list_models(
            RefreshStrategy::Online,
            codex_core::test_support::default_http_client_factory(),
        )
        .await;
    assert_eq!(refresh.requests().len(), 1);

    apply_turn_settings(
        &test.codex,
        &request.turn_id,
        TurnSettingsUpdate {
            effort: Some(Some(ReasoningEffort::High)),
            ..Default::default()
        },
    )
    .await?;
    answer_paused_turn(&test.codex, &request.turn_id).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.submit_text_turn("admit the next turn").await?;

    assert_eq!(
        response_mock
            .requests()
            .iter()
            .map(request_settings)
            .collect::<Vec<_>>(),
        vec![
            json!({
                "model": MODEL_A,
                "reasoning": { "effort": "low", "summary": "concise" },
                "service_tier": null,
            }),
            json!({
                "model": MODEL_A,
                "reasoning": { "effort": "high", "summary": "concise" },
                "service_tier": null,
            }),
            json!({
                "model": MODEL_A,
                "reasoning": { "effort": "low", "summary": "detailed" },
                "service_tier": null,
            }),
        ]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn captured_step_controls_exec_completion_and_write_stdin_output() -> Result<()> {
    core_test_support::skip_if_target_windows!(Ok(()), "uses POSIX read and printf");
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let output = "abcdefghij".repeat(100);
    let command = format!("stty -echo; printf '{output}'; read line; printf '{output}'; exit 7");
    let responses = mount_sse_sequence(
        &server,
        vec![
            paused_response("resp-a", "pause-a"),
            sse(vec![
                ev_response_created("resp-b"),
                ev_function_call(
                    "exec-b",
                    "exec_command",
                    &json!({
                        "cmd": command, "tty": true, "yield_time_ms": 10,
                    })
                    .to_string(),
                ),
                ev_completed("resp-b"),
            ]),
            paused_response("resp-b-pause", "pause-b"),
            sse(vec![
                ev_response_created("resp-c"),
                ev_function_call(
                    "stdin-c",
                    "write_stdin",
                    &json!({
                        "session_id": 1000, "chars": "done\n", "yield_time_ms": 1000,
                    })
                    .to_string(),
                ),
                ev_completed("resp-c"),
            ]),
            sse_completed("resp-result"),
        ],
    )
    .await;
    let test = direct_tool_settings_test()
        .with_config(|config| {
            config
                .features
                .enable(Feature::ShellTool)
                .expect("shell tools");
            config
                .features
                .enable(Feature::UnifiedExec)
                .expect("unified exec");
            config
                .permissions
                .set_permission_profile(PermissionProfile::Disabled)
                .expect("test permissions");
            for model in &mut config.model_catalog.as_mut().expect("models").models {
                model.shell_type = ConfigShellToolType::UnifiedExec;
                model.truncation_policy = codex_protocol::openai_models::TruncationPolicyConfig {
                    mode: codex_protocol::openai_models::TruncationMode::Bytes,
                    limit: match model.slug.as_str() {
                        MODEL_B => 400,
                        MODEL_C => 800,
                        _ => 8_000,
                    },
                };
            }
        })
        .build_with_auto_env(&server)
        .await?;
    let paused = start_paused_turn(&test.codex).await?;
    apply_turn_settings(
        &test.codex,
        &paused.turn_id,
        TurnSettingsUpdate {
            model: Some(MODEL_B.to_string()),
            ..Default::default()
        },
    )
    .await?;
    answer_paused_turn(&test.codex, &paused.turn_id).await?;
    let paused = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RequestUserInput(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    assert_eq!(paused.call_id, "pause-b");
    apply_turn_settings(
        &test.codex,
        &paused.turn_id,
        TurnSettingsUpdate {
            model: Some(MODEL_C.to_string()),
            ..Default::default()
        },
    )
    .await?;
    answer_paused_turn(&test.codex, &paused.turn_id).await?;
    let mut end = None;
    let mut turn_complete = false;
    wait_for_event(&test.codex, |event| {
        match event {
            EventMsg::ExecCommandEnd(event) if event.call_id == "exec-b" => {
                end = Some(event.clone())
            }
            EventMsg::TurnComplete(_) => turn_complete = true,
            _ => {}
        }
        end.is_some() && turn_complete
    })
    .await;
    let end = end.expect("exec completion");

    use codex_utils_output_truncation::TruncationPolicy;
    use codex_utils_output_truncation::formatted_truncate_text;
    let requests = responses.requests();
    let exec = requests[2]
        .function_call_output_text("exec-b")
        .expect("exec output");
    // The response budget also reserves space for command metadata and truncation notices.
    assert_eq!(exec.matches("chars truncated").count(), 1, "{exec}");
    assert!(
        (15..30).contains(&exec.matches("abcdefghij").count()),
        "{exec}"
    );
    let stdin = requests[4]
        .function_call_output_text("stdin-c")
        .expect("stdin output");
    assert_eq!(stdin.matches("chars truncated").count(), 1, "{stdin}");
    assert!(
        (60..85).contains(&stdin.matches("abcdefghij").count()),
        "{stdin}"
    );
    assert_eq!(end.exit_code, 7);
    // The formatting check needs an untruncated chunk, independent of how the executor
    // aggregates output across the initial command and later stdin interactions.
    assert!(end.aggregated_output.contains(&output));
    assert_eq!(
        end.formatted_output,
        formatted_truncate_text(&end.aggregated_output, TruncationPolicy::Bytes(400))
    );
    Ok(())
}

#[test_case(true; "model fallback and image detail")]
#[test_case(false; "text-only step omits images")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn captured_step_controls_mcp_output_limit(supports_images: bool) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    AppsTestServer::mount(&server).await?;
    let title = "abcdefghij".repeat(100);
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_partial_json(
            json!({"method": "tools/call"}),
        ))
        .respond_with(|request: &wiremock::Request| {
            let body: Value = serde_json::from_slice(&request.body).expect("MCP request");
            wiremock::ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0", "id": body["id"], "result": {
                    "content": [
                        {"type": "text", "text": body["params"]["arguments"]["title"]},
                        {"type": "image", "mimeType": "image/png",
                         "data": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
                         "_meta": {"codex/imageDetail": "original"}},
                    ], "isError": false,
                },
            }))
        })
        .with_priority(1)
        .mount(&server)
        .await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            paused_response("resp-a", "pause-a"),
            sse(vec![
                ev_response_created("resp-b"),
                ev_function_call_with_namespace(
                    "mcp-b",
                    "mcp__calendar",
                    "calendar_create_event",
                    &json!({"title": title}).to_string(),
                ),
                ev_completed("resp-b"),
            ]),
            sse_completed("resp-result"),
        ],
    )
    .await;
    let mcp_url = format!("{}/api/codex/ps/mcp", server.uri());
    let test = direct_tool_settings_test()
        .with_config(move |config| {
            for model in &mut config.model_catalog.as_mut().expect("models").models {
                model.supports_image_detail_original = model.slug != MODEL_B;
                model.input_modalities = vec![codex_protocol::openai_models::InputModality::Text];
                if model.slug != MODEL_B || supports_images {
                    model
                        .input_modalities
                        .push(codex_protocol::openai_models::InputModality::Image);
                }
                model.truncation_policy = codex_protocol::openai_models::TruncationPolicyConfig {
                    mode: codex_protocol::openai_models::TruncationMode::Bytes,
                    limit: if model.slug == MODEL_B { 80 } else { 8_000 },
                };
            }
            config
                .mcp_servers
                .set(HashMap::from([(
                    "calendar".to_string(),
                    serde_json::from_value(json!({
                        "url": mcp_url,
                        "tools": {
                            "calendar_create_event": {
                                "approval_mode": "approve"
                            }
                        },
                    }))
                    .expect("test MCP config"),
                )]))
                .expect("configure MCP");
        })
        .build_with_auto_env(&server)
        .await?;
    wait_for_mcp_server(&test.codex, "calendar").await?;
    let paused = start_paused_turn(&test.codex).await?;
    apply_turn_settings(
        &test.codex,
        &paused.turn_id,
        TurnSettingsUpdate {
            model: Some(MODEL_B.to_string()),
            ..Default::default()
        },
    )
    .await?;
    answer_paused_turn(&test.codex, &paused.turn_id).await?;
    let recorded_output = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RawResponseItem(item) => {
            let item = serde_json::to_value(&item.item).expect("raw response JSON");
            (item["call_id"] == "mcp-b" && item["type"] == "function_call_output").then_some(item)
        }
        _ => None,
    })
    .await;
    let images = recorded_output["output"]
        .as_array()
        .expect("MCP content")
        .iter()
        .filter(|item| item["type"] == "input_image")
        .map(|item| item["detail"].clone())
        .collect::<Vec<_>>();
    assert_eq!(
        images,
        if supports_images {
            vec![json!("high")]
        } else {
            vec![]
        }
    );
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let requests = responses.requests();
    let output_item = requests[2].function_call_output("mcp-b");
    let output = output_item["output"]
        .as_array()
        .expect("MCP content")
        .iter()
        .filter_map(|item| item["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(output.contains("truncated"), "{output}");
    assert!(output.matches("abcdefghij").count() < 10, "{output}");
    Ok(())
}

struct SettingsEcho;

impl ToolContributor for SettingsEcho {
    fn tools(
        &self,
        _session_store: &ExtensionData,
        _thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>> {
        vec![Arc::new(Self)]
    }
}

impl<'call> ToolExecutor<ToolCall<'call>> for SettingsEcho {
    fn tool_name(&self) -> ToolName {
        ToolName::plain("extension_settings")
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec::Function(codex_tools::ResponsesApiTool {
            name: "extension_settings".to_string(),
            description: "Report the settings received by this extension.".to_string(),
            strict: false,
            parameters: codex_tools::JsonSchema::default(),
            output_schema: None,
            defer_loading: None,
        })
    }

    fn handle<'a>(&'a self, call: ToolCall<'call>) -> ToolExecutorFuture<'a>
    where
        'call: 'a,
    {
        Box::pin(async move {
            let metadata: Value = serde_json::from_str(
                call.codex_turn_metadata
                    .as_deref()
                    .expect("extension metadata"),
            )
            .expect("metadata JSON");
            Ok(Box::new(JsonToolOutput::new(json!({
                "model": call.model,
                "metadata_model": metadata["model"],
                "effort": metadata["reasoning_effort"],
                "output_bytes": call.truncation_policy.byte_budget(),
            }))) as Box<dyn ToolOutput>)
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn captured_step_settings_reach_extension_executor() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            paused_response("resp-a", "pause-a"),
            sse(vec![
                ev_response_created("resp-b"),
                ev_function_call("extension-b", "extension_settings", "{}"),
                ev_completed("resp-b"),
            ]),
            sse_completed("resp-result"),
        ],
    )
    .await;
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.tool_contributor(Arc::new(SettingsEcho));
    let test = direct_tool_settings_test()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            for model in &mut config.model_catalog.as_mut().expect("models").models {
                model.truncation_policy =
                    TruncationPolicyConfig::bytes(if model.slug == MODEL_B { 512 } else { 8_000 });
            }
        })
        .build_with_auto_env(&server)
        .await?;
    let paused = start_paused_turn(&test.codex).await?;
    apply_turn_settings(
        &test.codex,
        &paused.turn_id,
        TurnSettingsUpdate {
            model: Some(MODEL_B.to_string()),
            effort: Some(Some(ReasoningEffort::High)),
            ..Default::default()
        },
    )
    .await?;
    answer_paused_turn(&test.codex, &paused.turn_id).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let output = responses.requests()[2]
        .function_call_output_text("extension-b")
        .expect("extension output");
    assert_eq!(
        serde_json::from_str::<Value>(&output)?,
        json!({
            "model": MODEL_B,
            "metadata_model": MODEL_B,
            "effort": "high",
            "output_bytes": 512,
        })
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn captured_step_controls_mcp_resource_output() -> Result<()> {
    skip_if_no_network!(Ok(()));
    core_test_support::skip_if_wine_exec!(Ok(()), "requires a Windows test_stdio_server binary");
    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            paused_response("resp-a", "pause-a"),
            sse(vec![
                ev_response_created("resp-b"),
                ev_function_call(
                    "resource-b",
                    "read_mcp_resource",
                    &json!({
                        "server": "resources", "uri": "memo://codex/example-note",
                    })
                    .to_string(),
                ),
                ev_completed("resp-b"),
            ]),
            sse_completed("resp-result"),
        ],
    )
    .await;
    let command = remote_aware_stdio_server_bin()?;
    let test = direct_tool_settings_test()
        .with_config(move |config| {
            for model in &mut config.model_catalog.as_mut().expect("models").models {
                model.truncation_policy =
                    TruncationPolicyConfig::bytes(if model.slug == MODEL_B { 80 } else { 8_000 });
            }
            config
                .mcp_servers
                .set(HashMap::from([(
                    "resources".to_string(),
                    serde_json::from_value(json!({
                        "command": command,
                        "environment_id": remote_aware_environment_id(),
                        "cwd": config.cwd,
                    }))
                    .expect("test MCP config"),
                )]))
                .expect("configure MCP");
        })
        .build_with_auto_env(&server)
        .await?;
    wait_for_mcp_server(&test.codex, "resources").await?;
    let paused = start_paused_turn(&test.codex).await?;
    apply_turn_settings(
        &test.codex,
        &paused.turn_id,
        TurnSettingsUpdate {
            model: Some(MODEL_B.to_string()),
            ..Default::default()
        },
    )
    .await?;
    answer_paused_turn(&test.codex, &paused.turn_id).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let output = responses.requests()[2]
        .function_call_output_text("resource-b")
        .expect("resource output");
    assert!(output.starts_with("{\"server\":\"resources\""), "{output}");
    assert_eq!(output.matches("truncated").count(), 1, "{output}");
    // B allows 96 serialized bytes plus the truncation notice; A preserves the full resource.
    assert!(output.len() < 160, "{output}");
    Ok(())
}

#[path = "step_settings_compaction.rs"]
mod compaction;
