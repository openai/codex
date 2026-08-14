#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used)]

use anyhow::Result;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_config::types::AppToolApproval;
use codex_core::TurnInputRequest;
use codex_core::config::Config;
use codex_features::Feature;
use codex_history::RolloutItem;
use codex_history::RolloutLine;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::items::McpToolCallStatus;
use codex_protocol::items::TurnItem;
use codex_protocol::models::NetworkPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ElicitationAction;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::request_permissions::PermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionProfile;
use codex_protocol::request_permissions::RequestPermissionsResponse;
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
use core_test_support::responses::assert_root_turn;
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
use test_case::test_case;

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
        .with_user_config(&user_config_path, user_config)
        .expect("apps user config should be valid");
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
        .with_user_config(&user_config_path, user_config)
        .expect("apps user config should be valid");
}

async fn submit_user_turn(
    test: &TestCodex,
    text: &str,
    approval_policy: AskForApproval,
    permission_profile: PermissionProfile,
    collaboration_mode: Option<CollaborationMode>,
) -> Result<()> {
    let session_model = test.session_configured.model.clone();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(permission_profile, test.cwd.path());
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: text.to_string(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(local_selections(test.config.cwd.clone())),
                approval_policy: Some(approval_policy),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: collaboration_mode.or({
                    Some(CollaborationMode {
                        mode: ModeKind::Default,
                        settings: Settings {
                            model: session_model,
                            reasoning_effort: None,
                            developer_instructions: None,
                        },
                    })
                }),
                ..Default::default()
            }),
        )
        .await?;
    Ok(())
}

async fn wait_for_mcp_tool_call_item(
    test: &TestCodex,
    call_id: &str,
    status: McpToolCallStatus,
) -> Option<bool> {
    wait_for_event_match(&test.codex, |event| {
        let item = match event {
            EventMsg::ItemStarted(event) => &event.item,
            EventMsg::ItemCompleted(event) => &event.item,
            _ => return None,
        };
        let TurnItem::McpToolCall(item) = item else {
            return None;
        };
        (item.id == call_id && item.status == status).then_some(item.read_only_hint)
    })
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(false; "without strict auto review")]
#[test_case(true; "with strict auto review")]
async fn approved_mcp_tool_call_metadata_records_prior_user_input_request(
    strict_auto_review: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    let call_id = "calendar-call-approval";
    let calendar_args = serde_json::to_string(&json!({
        "title": "Lunch",
        "starts_at": "2026-03-10T12:00:00Z"
    }))?;
    let mut response_sequence = Vec::new();
    if strict_auto_review {
        let requested_permissions = RequestPermissionProfile {
            network: Some(NetworkPermissions {
                enabled: Some(true),
            }),
            ..Default::default()
        };
        let request_permissions_args = json!({
            "reason": "Enable strict auto review before the MCP approval",
            "permissions": requested_permissions,
        });
        response_sequence.push(sse(vec![
            ev_response_created("resp-permissions"),
            ev_function_call(
                "calendar-strict-permissions",
                "request_permissions",
                &serde_json::to_string(&request_permissions_args)?,
            ),
            ev_completed("resp-permissions"),
        ]));
    }
    response_sequence.push(sse(vec![
        ev_response_created("resp-1"),
        ev_function_call_with_namespace(
            call_id,
            SEARCH_CALENDAR_NAMESPACE,
            SEARCH_CALENDAR_CREATE_TOOL,
            &calendar_args,
        ),
        ev_completed("resp-1"),
    ]));
    if strict_auto_review {
        response_sequence.push(sse(vec![
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
        ]));
    }
    response_sequence.push(sse(vec![
        ev_response_created("resp-2"),
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]));
    let mock = mount_sse_sequence(&server, response_sequence).await;

    let mut builder = search_capable_apps_builder(apps_server.chatgpt_base_url.clone())
        .with_config(move |config| {
            // The permission grant needs user review before strict review applies to the turn.
            config.approvals_reviewer = if strict_auto_review {
                ApprovalsReviewer::User
            } else {
                ApprovalsReviewer::AutoReview
            };
            config
                .features
                .enable(Feature::ToolCallMcpElicitation)
                .expect("test config should allow feature update");
            if strict_auto_review {
                config
                    .features
                    .enable(Feature::RequestPermissionsTool)
                    .expect("test config should allow feature update");
            }
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
        PermissionProfile::Disabled,
        /*collaboration_mode*/ None,
    )
    .await?;

    if strict_auto_review {
        let event = wait_for_event(&test.codex, |event| {
            matches!(
                event,
                EventMsg::RequestPermissions(_) | EventMsg::TurnComplete(_)
            )
        })
        .await;
        let EventMsg::RequestPermissions(request) = event else {
            panic!("expected permission request before MCP approval, received {event:?}");
        };
        assert_eq!(request.call_id, "calendar-strict-permissions");
        test.codex
            .submit(Op::RequestPermissionsResponse {
                id: request.call_id,
                response: RequestPermissionsResponse {
                    permissions: request.permissions,
                    scope: PermissionGrantScope::Turn,
                    strict_auto_review: true,
                },
            })
            .await?;
    }

    let EventMsg::McpToolCallBegin(begin) = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::McpToolCallBegin(_))
    })
    .await
    else {
        unreachable!("event guard guarantees McpToolCallBegin");
    };
    assert_eq!(begin.call_id, call_id);

    if !strict_auto_review {
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
    }

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let response_requests = mock.requests();
    assert_eq!(
        response_requests.len(),
        2 + 2 * usize::from(strict_auto_review)
    );
    let response_body = response_requests[0].body_json();
    let turn_id = response_body["client_metadata"]["turn_id"]
        .as_str()
        .expect("Responses request turn id");
    assert_root_turn(&response_body, Some(turn_id))?;
    let apps_tool_call = recorded_apps_tool_call_by_call_id(&server, call_id).await;
    let mcp_turn_metadata = apps_tool_call
        .pointer("/params/_meta/x-codex-turn-metadata")
        .expect("MCP tools/call turn metadata");
    assert_eq!(
        (
            mcp_turn_metadata.get("root_turn_id"),
            mcp_turn_metadata.get("parent_turn_id"),
        ),
        (None, None)
    );

    assert_eq!(
        apps_tool_call.pointer("/params/_meta/callId"),
        Some(&json!(call_id))
    );
    assert_eq!(
        apps_tool_call
            .pointer("/params/_meta/x-codex-turn-metadata/user_input_requested_during_turn"),
        (!strict_auto_review).then_some(&json!(true))
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(false; "unmanaged model")]
#[test_case(true; "protected model")]
async fn apps_default_prompt_with_auto_review_routes_actual_mcp_approval_to_guardian(
    protected_model: bool,
) -> Result<()> {
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
        .with_config(move |config| {
            let (reviewer, app_reviewer) = if protected_model {
                (ApprovalsReviewer::AutoReview, ApprovalsReviewer::User)
            } else {
                (ApprovalsReviewer::User, ApprovalsReviewer::AutoReview)
            };
            config.approvals_reviewer = reviewer;
            config
                .features
                .enable(Feature::ToolCallMcpElicitation)
                .expect("test config should allow feature update");
            set_default_app_approval_mode_and_reviewer(
                config,
                AppToolApproval::Prompt,
                app_reviewer,
            );
        });
    if protected_model {
        builder = builder.with_model("gpt-5.4").with_cloud_config_bundle(
            CloudConfigBundleFixture::loader_with_enterprise_requirement(
                "[auto_review]\nrequired_on_models = [\"gpt-5.4\"]\n",
            ),
        );
    }
    let test = builder.build(&server).await?;

    submit_user_turn(
        &test,
        "Use [$calendar](app://calendar) to create a calendar event.",
        AskForApproval::OnRequest,
        if protected_model {
            PermissionProfile::workspace_write()
        } else {
            PermissionProfile::Disabled
        },
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
        PermissionProfile::Disabled,
        /*collaboration_mode*/ None,
    )
    .await?;

    assert_eq!(
        wait_for_mcp_tool_call_item(&test, read_call_id, McpToolCallStatus::InProgress).await,
        Some(true)
    );
    let EventMsg::McpToolCallBegin(read_begin) = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::McpToolCallBegin(_))
    })
    .await
    else {
        unreachable!("event guard guarantees McpToolCallBegin");
    };
    assert_eq!(read_begin.call_id, read_call_id);
    assert_eq!(read_begin.read_only_hint, Some(true));

    assert_eq!(
        wait_for_mcp_tool_call_item(&test, read_call_id, McpToolCallStatus::Completed).await,
        Some(true)
    );
    assert_eq!(
        wait_for_mcp_tool_call_item(&test, write_call_id, McpToolCallStatus::InProgress).await,
        Some(false)
    );
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
    assert_eq!(write_begin.read_only_hint, Some(false));

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

    assert_eq!(
        wait_for_mcp_tool_call_item(&test, write_call_id, McpToolCallStatus::Completed).await,
        Some(false)
    );
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    assert_eq!(responses.requests().len(), 3);
    recorded_apps_tool_call_by_call_id(&server, read_call_id).await;
    recorded_apps_tool_call_by_call_id(&server, write_call_id).await;

    test.codex.ensure_rollout_materialized().await;
    test.codex.flush_rollout().await?;
    let rollout_path = test.codex.rollout_path().expect("rollout path");
    let persisted_hints = tokio::fs::read_to_string(rollout_path)
        .await?
        .lines()
        .map(serde_json::from_str::<RolloutLine>)
        .collect::<serde_json::Result<Vec<_>>>()?
        .into_iter()
        .filter_map(|line| match line.item {
            RolloutItem::EventMsg(EventMsg::McpToolCallEnd(event))
                if event.call_id == read_call_id || event.call_id == write_call_id =>
            {
                Some((event.call_id, event.read_only_hint))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        persisted_hints,
        vec![
            (read_call_id.to_string(), Some(true)),
            (write_call_id.to_string(), Some(false)),
        ]
    );

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
        PermissionProfile::Disabled,
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
