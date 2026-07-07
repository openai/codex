#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used)]

use anyhow::Result;
use codex_config::types::AppToolApproval;
use codex_core::config::Config;
use codex_features::Feature;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ElicitationAction;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_protocol::user_input::UserInput;
use core_test_support::PathExt;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::SEARCH_CALENDAR_CREATE_TOOL;
use core_test_support::apps_test_server::SEARCH_CALENDAR_LIST_TOOL;
use core_test_support::apps_test_server::SEARCH_CALENDAR_NAMESPACE;
use core_test_support::apps_test_server::recorded_apps_tool_call_by_call_id;
use core_test_support::apps_test_server::search_capable_apps_builder;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashMap;

fn set_calendar_approval_mode(config: &mut Config, approval_mode: AppToolApproval) {
    let approval_mode = match approval_mode {
        AppToolApproval::Auto => "auto",
        AppToolApproval::Prompt => "prompt",
        AppToolApproval::Writes => "writes",
        AppToolApproval::Approve => "approve",
    };
    let user_config_path = config.codex_home.join("config.toml").abs();
    let user_config = toml::from_str(&format!(
        r#"
[apps.calendar]
default_tools_approval_mode = "{approval_mode}"
"#
    ))
    .expect("apps config should parse");
    config.config_layer_stack = config
        .config_layer_stack
        .with_user_config(&user_config_path, user_config);
}

fn set_default_app_approval_mode_and_reviewer(
    config: &mut Config,
    approval_mode: AppToolApproval,
    default_approvals_reviewer: ApprovalsReviewer,
) {
    let approval_mode = match approval_mode {
        AppToolApproval::Auto => "auto",
        AppToolApproval::Prompt => "prompt",
        AppToolApproval::Writes => "writes",
        AppToolApproval::Approve => "approve",
    };
    let user_config_path = config.codex_home.join("config.toml").abs();
    let user_config = toml::from_str(&format!(
        r#"
[apps._default]
approvals_reviewer = "{default_approvals_reviewer}"
default_tools_approval_mode = "{approval_mode}"
"#
    ))
    .expect("apps config should parse");
    config.config_layer_stack = config
        .config_layer_stack
        .with_user_config(&user_config_path, user_config);
}

async fn submit_user_turn(
    test: &TestCodex,
    text: &str,
    approval_policy: AskForApproval,
    collaboration_mode: Option<CollaborationMode>,
) -> Result<()> {
    let session_model = test.session_configured.model.clone();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, test.cwd.path());
    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: text.to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                environments: Some(local_selections(test.config.cwd.clone())),
                approval_policy: Some(approval_policy),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: collaboration_mode.or({
                    Some(codex_protocol::config_types::CollaborationMode {
                        mode: codex_protocol::config_types::ModeKind::Default,
                        settings: codex_protocol::config_types::Settings {
                            model: session_model,
                            reasoning_effort: None,
                            developer_instructions: None,
                        },
                    })
                }),
                ..Default::default()
            },
        })
        .await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn approved_mcp_tool_call_metadata_records_prior_user_input_request() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    let call_id = "calendar-call-approval";
    let calendar_args = serde_json::to_string(&json!({
        "title": "Lunch",
        "starts_at": "2026-03-10T12:00:00Z"
    }))?;
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call_with_namespace(
                    call_id,
                    SEARCH_CALENDAR_NAMESPACE,
                    SEARCH_CALENDAR_CREATE_TOOL,
                    &calendar_args,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = search_capable_apps_builder(apps_server.chatgpt_base_url.clone())
        .with_config(|config| {
            // Use the opposite global reviewer so this route must come from apps._default.
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config
                .features
                .enable(Feature::ToolCallMcpElicitation)
                .expect("test config should allow feature update");
            set_default_app_approval_mode_and_reviewer(
                config,
                AppToolApproval::Prompt,
                ApprovalsReviewer::User,
            );
        });
    let test = builder.build(&server).await?;

    submit_user_turn(
        &test,
        "Use [$calendar](app://calendar) to create a calendar event.",
        AskForApproval::OnRequest,
        /*collaboration_mode*/ None,
    )
    .await?;

    let EventMsg::McpToolCallBegin(begin) = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::McpToolCallBegin(_))
    })
    .await
    else {
        unreachable!("event guard guarantees McpToolCallBegin");
    };
    assert_eq!(begin.call_id, call_id);

    let EventMsg::ElicitationRequest(request) = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ElicitationRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await
    else {
        panic!("expected apps._default user to route the app approval to the user");
    };

    test.codex
        .submit(Op::ResolveElicitation {
            server_name: request.server_name,
            request_id: request.id,
            decision: ElicitationAction::Accept,
            content: None,
            meta: None,
        })
        .await?;

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    assert_eq!(mock.requests().len(), 2);
    let apps_tool_call = recorded_apps_tool_call_by_call_id(&server, call_id).await;

    assert_eq!(
        apps_tool_call
            .pointer("/params/_meta/x-codex-turn-metadata/user_input_requested_during_turn"),
        Some(&json!(true))
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apps_default_prompt_with_auto_review_routes_actual_mcp_approval_to_guardian() -> Result<()>
{
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    let call_id = "calendar-default-auto-review";
    let calendar_args = serde_json::to_string(&json!({
        "title": "Lunch",
        "starts_at": "2026-03-10T12:00:00Z"
    }))?;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-parent-tool"),
                ev_function_call_with_namespace(
                    call_id,
                    SEARCH_CALENDAR_NAMESPACE,
                    SEARCH_CALENDAR_CREATE_TOOL,
                    &calendar_args,
                ),
                ev_completed("resp-parent-tool"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian-review"),
                ev_assistant_message(
                    "msg-guardian-review",
                    &json!({
                        "risk_level": "low",
                        "user_authorization": "high",
                        "outcome": "allow",
                        "rationale": "Creating this calendar event is low risk.",
                    })
                    .to_string(),
                ),
                ev_completed("resp-guardian-review"),
            ]),
            sse(vec![
                ev_response_created("resp-parent-done"),
                ev_assistant_message("msg-parent-done", "done"),
                ev_completed("resp-parent-done"),
            ]),
        ],
    )
    .await;

    let mut builder = search_capable_apps_builder(apps_server.chatgpt_base_url.clone())
        .with_config(|config| {
            // Use the opposite global reviewer so this route must come from apps._default.
            config.approvals_reviewer = ApprovalsReviewer::User;
            config
                .features
                .enable(Feature::ToolCallMcpElicitation)
                .expect("test config should allow feature update");
            set_default_app_approval_mode_and_reviewer(
                config,
                AppToolApproval::Prompt,
                ApprovalsReviewer::AutoReview,
            );
        });
    let test = builder.build(&server).await?;

    submit_user_turn(
        &test,
        "Use [$calendar](app://calendar) to create a calendar event.",
        AskForApproval::OnRequest,
        /*collaboration_mode*/ None,
    )
    .await?;

    let route_event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ElicitationRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;
    assert!(
        matches!(route_event, EventMsg::TurnComplete(_)),
        "expected apps._default auto_review to route the app approval to Guardian"
    );

    let guardian_request = responses
        .requests()
        .into_iter()
        .find(|request| {
            request
                .instructions_text()
                .starts_with("You are judging one planned coding-agent action.")
        })
        .expect("expected a Guardian request for the app MCP approval");
    assert!(guardian_request.body_contains_text("calendar_create_event"));
    assert!(guardian_request.body_contains_text("Lunch"));

    let apps_tool_call = recorded_apps_tool_call_by_call_id(&server, call_id).await;
    assert_eq!(
        apps_tool_call.pointer("/params/arguments/title"),
        Some(&json!("Lunch"))
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apps_default_writes_prompts_for_writes_but_not_reads() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    let read_call_id = "calendar-read";
    let write_call_id = "calendar-write";
    let list_args = serde_json::to_string(&json!({}))?;
    let create_args = serde_json::to_string(&json!({
        "title": "Lunch",
        "starts_at": "2026-03-10T12:00:00Z"
    }))?;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-read"),
                ev_function_call_with_namespace(
                    read_call_id,
                    SEARCH_CALENDAR_NAMESPACE,
                    SEARCH_CALENDAR_LIST_TOOL,
                    &list_args,
                ),
                ev_completed("resp-read"),
            ]),
            sse(vec![
                ev_response_created("resp-write"),
                ev_function_call_with_namespace(
                    write_call_id,
                    SEARCH_CALENDAR_NAMESPACE,
                    SEARCH_CALENDAR_CREATE_TOOL,
                    &create_args,
                ),
                ev_completed("resp-write"),
            ]),
            sse(vec![
                ev_response_created("resp-done"),
                ev_assistant_message("msg-done", "done"),
                ev_completed("resp-done"),
            ]),
        ],
    )
    .await;

    let mut builder = search_capable_apps_builder(apps_server.chatgpt_base_url.clone())
        .with_config(|config| {
            config
                .features
                .enable(Feature::ToolCallMcpElicitation)
                .expect("test config should allow feature update");
            set_default_app_approval_mode_and_reviewer(
                config,
                AppToolApproval::Writes,
                ApprovalsReviewer::User,
            );
        });
    let test = builder.build(&server).await?;

    submit_user_turn(
        &test,
        "Use [$calendar](app://calendar) to list events, then create one.",
        AskForApproval::OnRequest,
        /*collaboration_mode*/ None,
    )
    .await?;

    let EventMsg::McpToolCallBegin(read_begin) = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::McpToolCallBegin(_))
    })
    .await
    else {
        unreachable!("event guard guarantees McpToolCallBegin");
    };
    assert_eq!(read_begin.call_id, read_call_id);

    let next_route = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::McpToolCallBegin(_) | EventMsg::ElicitationRequest(_)
        )
    })
    .await;
    let EventMsg::McpToolCallBegin(write_begin) = next_route else {
        panic!("read-only app action should not prompt in writes mode");
    };
    assert_eq!(write_begin.call_id, write_call_id);

    let EventMsg::ElicitationRequest(request) = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ElicitationRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await
    else {
        panic!("write app action should prompt in writes mode");
    };

    test.codex
        .submit(Op::ResolveElicitation {
            server_name: request.server_name,
            request_id: request.id,
            decision: ElicitationAction::Accept,
            content: None,
            meta: None,
        })
        .await?;

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    assert_eq!(responses.requests().len(), 3);
    recorded_apps_tool_call_by_call_id(&server, read_call_id).await;
    recorded_apps_tool_call_by_call_id(&server, write_call_id).await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_tool_call_metadata_records_prior_request_user_input_tool() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    let request_user_input_call_id = "user-input-call";
    let calendar_call_id = "calendar-call-after-user-input";
    let request_user_input_args = json!({
        "questions": [{
            "id": "confirm_path",
            "header": "Confirm",
            "question": "Proceed with the plan?",
            "options": [{
                "label": "Yes (Recommended)",
                "description": "Continue the current plan."
            }, {
                "label": "No",
                "description": "Stop and revisit the approach."
            }]
        }]
    })
    .to_string();
    let calendar_args = serde_json::to_string(&json!({
        "title": "Lunch",
        "starts_at": "2026-03-10T12:00:00Z"
    }))?;
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    request_user_input_call_id,
                    "request_user_input",
                    &request_user_input_args,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_function_call_with_namespace(
                    calendar_call_id,
                    SEARCH_CALENDAR_NAMESPACE,
                    SEARCH_CALENDAR_CREATE_TOOL,
                    &calendar_args,
                ),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-3"),
            ]),
        ],
    )
    .await;

    let mut builder = search_capable_apps_builder(apps_server.chatgpt_base_url.clone())
        .with_config(|config| {
            set_calendar_approval_mode(config, AppToolApproval::Approve);
        });
    let test = builder.build(&server).await?;

    submit_user_turn(
        &test,
        "Ask for confirmation, then create a calendar event.",
        AskForApproval::Never,
        Some(CollaborationMode {
            mode: ModeKind::Plan,
            settings: Settings {
                model: test.session_configured.model.clone(),
                reasoning_effort: None,
                developer_instructions: None,
            },
        }),
    )
    .await?;

    let request = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RequestUserInput(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    assert_eq!(request.call_id, request_user_input_call_id);

    test.codex
        .submit(Op::UserInputAnswer {
            id: request.turn_id,
            response: RequestUserInputResponse {
                answers: HashMap::from([(
                    "confirm_path".to_string(),
                    RequestUserInputAnswer {
                        answers: vec!["Yes (Recommended)".to_string()],
                    },
                )]),
            },
        })
        .await?;

    let EventMsg::McpToolCallBegin(begin) = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::McpToolCallBegin(_))
    })
    .await
    else {
        unreachable!("event guard guarantees McpToolCallBegin");
    };
    assert_eq!(begin.call_id, calendar_call_id);

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    assert_eq!(mock.requests().len(), 3);
    let apps_tool_call = recorded_apps_tool_call_by_call_id(&server, calendar_call_id).await;

    assert_eq!(
        apps_tool_call
            .pointer("/params/_meta/x-codex-turn-metadata/user_input_requested_during_turn"),
        Some(&json!(true))
    );

    Ok(())
}
