//! Verify MCP prompts and elicitation analytics through actual turns.

use anyhow::Result;
use codex_analytics::AnalyticsEventsClient;
use codex_analytics::AppServerRpcTransport;
use codex_app_server_protocol as app;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_core::TurnInputSubmission;
use codex_core::config::Constrained;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::McpToolResultInput;
use codex_extension_api::ToolLifecycleContributor;
use codex_extension_api::ToolLifecycleFuture;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_protocol::approvals::ElicitationRequest;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::items::McpToolCallStatus;
use codex_protocol::items::TurnItem;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ElicitationAction;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::PathExt;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::SEARCH_CALENDAR_CREATE_TOOL;
use core_test_support::apps_test_server::SEARCH_CALENDAR_NAMESPACE;
use core_test_support::apps_test_server::recorded_apps_tool_call_by_call_id;
use core_test_support::apps_test_server::recorded_apps_tool_calls;
use core_test_support::apps_test_server::search_capable_apps_builder;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::wait_for_event;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use test_case::test_case;
use wiremock::Mock;
use wiremock::Request;
use wiremock::Respond;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::path_regex;

const CALL_ID: &str = "calendar-elicitation-call";
const PRIVATE_SENTINEL: &str = "synthetic-private-value";

#[derive(Clone, Copy)]
enum AuthFailureFormat {
    Text,
    Structured,
}

#[derive(Clone, Copy)]
struct AuthFailureResponder {
    scenario: Scenario,
    format: AuthFailureFormat,
}

impl AuthFailureResponder {
    fn result(self) -> Value {
        let mut response = json!({
            "content": [{
                "type": "text",
                "text": PRIVATE_SENTINEL,
            }],
            "isError": true,
            "_meta": {
                "_codex_apps": {
                    "connector_auth_failure": {
                        "is_auth_failure": true,
                        "auth_reason": "reauthentication_required",
                        "connector_id": "calendar",
                        "link_id": "link_123",
                        "error_code": "UNAUTHORIZED",
                        "error_http_status_code": 401,
                        "error_action": "TRIGGER_REAUTHENTICATION",
                    },
                },
            },
        });
        if self.scenario == Scenario::AuthMetadataRemoved {
            response["_meta"] = json!({"_codex_apps": {"connector_auth_failure": {
                "is_auth_failure": true, "connector_id": "calendar",
            }}});
        }
        if matches!(self.format, AuthFailureFormat::Structured) {
            response["structuredContent"] = json!({
                "error": "synthetic-structured-auth-failure",
                "details": {"reason": "reauthentication_required", "status": 401},
            });
        }
        response
    }
}

impl Respond for AuthFailureResponder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: Value =
            serde_json::from_slice(&request.body).expect("tools/call request should be valid JSON");
        ResponseTemplate::new(/*status*/ 200).set_body_json(json!({
            "jsonrpc": "2.0",
            "id": body.get("id").cloned().unwrap_or(Value::Null),
            "result": self.result(),
        }))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Scenario {
    DefaultAuth,
    ModernAuth,
    AuthMetadataRemoved,
    LegacySuccess,
    LegacyCancelled,
    ModernDeclined,
}

struct RemoveAuthMetadata;

impl ToolLifecycleContributor for RemoveAuthMetadata {
    fn on_mcp_tool_result<'a>(&'a self, input: McpToolResultInput<'a>) -> ToolLifecycleFuture<'a> {
        Box::pin(async move { input.result.meta = None })
    }
}

#[test_case(Scenario::DefaultAuth; "auth requests elicitation by default")]
#[test_case(Scenario::ModernAuth; "modern accepted auth")]
#[test_case(Scenario::AuthMetadataRemoved; "auth metadata removed by callback")]
#[test_case(Scenario::LegacySuccess; "legacy accepted ordinary")]
#[test_case(Scenario::LegacyCancelled; "legacy cancelled")]
#[test_case(Scenario::ModernDeclined; "modern declined")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actual_turn_elicitation_analytics(scenario: Scenario) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let modern = scenario != Scenario::LegacySuccess && scenario != Scenario::LegacyCancelled;
    let expected_approvals = usize::from(scenario != Scenario::DefaultAuth);
    let approved = scenario != Scenario::LegacyCancelled && scenario != Scenario::ModernDeclined;
    let auth_failure = matches!(
        scenario,
        Scenario::DefaultAuth | Scenario::ModernAuth | Scenario::AuthMetadataRemoved
    );
    let server = responses::start_mock_server().await;
    AppsTestServer::mount_searchable(&server).await?;
    Mock::given(method("POST"))
        .and(path("/codex/analytics-events/events"))
        .respond_with(ResponseTemplate::new(/*status*/ 200))
        .mount(&server)
        .await;

    if auth_failure {
        Mock::given(method("POST"))
            .and(path_regex("^/api/codex/ps/mcp/?$"))
            .and(body_partial_json(json!({
                "method": "tools/call",
                "params": {"name": "calendar_create_event"},
            })))
            .respond_with(AuthFailureResponder {
                scenario,
                format: AuthFailureFormat::Text,
            })
            .with_priority(/*p*/ 1)
            .mount(&server)
            .await;
    }

    let arguments =
        json!({"title": PRIVATE_SENTINEL, "starts_at": "2026-06-18T12:00:00Z"}).to_string();
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_function_call_with_namespace(
                    CALL_ID,
                    SEARCH_CALENDAR_NAMESPACE,
                    SEARCH_CALENDAR_CREATE_TOOL,
                    &arguments,
                ),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse_completed("resp-2"),
        ],
    )
    .await;

    let client = AnalyticsEventsClient::new(
        codex_core::test_support::auth_manager_from_auth(
            CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        ),
        server.uri(),
        /*analytics_enabled*/ Some(true),
    );
    let mut builder = search_capable_apps_builder(server.uri())
        .with_analytics_events_client(client.clone())
        .with_config(move |config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::User;
            if scenario != Scenario::DefaultAuth {
                config
                    .features
                    .set_enabled(Feature::ToolCallMcpElicitation, modern)
                    .expect("approval feature should be configurable");
            }
            let user_config_path = config.codex_home.join("config.toml").abs();
            let approval_mode = if scenario == Scenario::DefaultAuth {
                "auto"
            } else {
                "prompt"
            };
            let user_config = toml::from_str(&format!(
                r#"[apps.calendar]
default_tools_approval_mode = "{approval_mode}"
approvals_reviewer = "user"
"#,
            ))
            .expect("apps config should parse");
            config.config_layer_stack = config
                .config_layer_stack
                .with_user_config(&user_config_path, user_config)
                .expect("apps user config should be valid");
        });
    if scenario == Scenario::AuthMetadataRemoved {
        let mut extensions = ExtensionRegistryBuilder::new();
        extensions.tool_lifecycle_contributor(Arc::new(RemoveAuthMetadata));
        builder = builder.with_extensions(Arc::new(extensions.build()));
    }
    let test = builder.build_with_auto_env(&server).await?;
    let session = &test.session_configured;
    let thread_id = session.thread_id.to_string();
    client.track_initialize(
        /*connection_id*/ 1,
        app::InitializeParams::default(),
        "test-client".to_string(),
        AppServerRpcTransport::Stdio,
    );
    let thread_response = serde_json::from_value(json!({
        "thread": {
            "id": thread_id, "sessionId": session.session_id.to_string(), "preview": "",
            "ephemeral": false, "modelProvider": session.model_provider_id,
            "createdAt": 1, "updatedAt": 1, "status": {"type": "idle"},
            "cwd": session.cwd, "cliVersion": "0.0.0", "source": "exec", "turns": [],
        },
        "model": session.model, "modelProvider": session.model_provider_id, "cwd": session.cwd,
        "approvalPolicy": app::AskForApproval::from(session.approval_policy),
        "approvalsReviewer": app::ApprovalsReviewer::from(session.approvals_reviewer),
        "sandbox": app::SandboxPolicy::from(test.config.legacy_sandbox_policy()),
    }))?;
    client.track_response(
        /*connection_id*/ 1,
        app::RequestId::Integer(1),
        &app::ClientResponsePayload::ThreadStart(thread_response),
    );

    let request = TurnInputRequest::user_input(vec![UserInput::Text {
        text: "Use [$calendar](app://calendar) to create a calendar event.".to_string(),
        text_elements: Vec::new(),
    }]);
    let submitted = test.codex.start_or_steer_turn(request).await?;
    let TurnInputSubmission::Started { turn_id } = submitted else {
        anyhow::bail!("expected a new turn, got {submitted:?}");
    };

    let mut approvals_seen = 0;
    let mut auth_request = None;
    let mut target_lifecycle = [0; 2];
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let event = test.codex.next_event().await?;
            let event_turn_id = event.id;
            match event.msg {
                EventMsg::TurnStarted(started) => {
                    assert_eq!(started.turn_id, turn_id);
                    client.track_notification(&app::ServerNotification::TurnStarted(
                        serde_json::from_value(json!({
                            "threadId": thread_id,
                            "turn": {"id": started.turn_id, "items": [],
                                "status": "inProgress", "startedAt": started.started_at},
                        }))?,
                    ));
                }
                message @ (EventMsg::ItemStarted(_) | EventMsg::ItemCompleted(_)) => {
                    let (item, item_thread_id, item_turn_id, completed) = match &message {
                        EventMsg::ItemStarted(event) => {
                            (&event.item, &event.thread_id, &event.turn_id, false)
                        }
                        EventMsg::ItemCompleted(event) => {
                            (&event.item, &event.thread_id, &event.turn_id, true)
                        }
                        _ => unreachable!("item guard guarantees item lifecycle event"),
                    };
                    if let TurnItem::McpToolCall(item) = item
                        && item.id == CALL_ID
                    {
                        assert_eq!(event_turn_id, turn_id);
                        assert_eq!(item_thread_id, &session.thread_id);
                        assert_eq!(item_turn_id, &turn_id);
                        assert_eq!(item.server, CODEX_APPS_MCP_SERVER_NAME);
                        assert_eq!(item.connector_id.as_deref(), Some("calendar"));
                        target_lifecycle[usize::from(completed)] += 1;
                    }
                    let notification =
                        app::item_event_to_server_notification(message, &thread_id, &event_turn_id);
                    client.track_notification(&notification);
                }
                EventMsg::RequestUserInput(request) => {
                    assert!(!modern, "legacy approval requested for {scenario:?}");
                    assert!(recorded_apps_tool_calls(&server).await.is_empty());
                    assert_eq!(request.turn_id, turn_id);
                    approvals_seen += 1;
                    let answer = if approved { "Allow" } else { "Cancel" };
                    let response = serde_json::from_value(json!({"answers": {
                        (request.questions[0].id.clone()): {
                            "answers": [answer, format!("user_note: {PRIVATE_SENTINEL}")]
                        }
                    }}))?;
                    test.codex
                        .submit(Op::UserInputAnswer {
                            id: request.turn_id,
                            response,
                        })
                        .await?;
                }
                EventMsg::ElicitationRequest(request) => {
                    assert_eq!(request.server_name, CODEX_APPS_MCP_SERVER_NAME);
                    let action = match &request.request {
                        ElicitationRequest::UserVerification { .. } => {
                            unreachable!("unexpected verification")
                        }
                        ElicitationRequest::Url { .. } => {
                            assert_eq!(approvals_seen, expected_approvals);
                            assert!(auth_request.is_none());
                            assert_eq!(recorded_apps_tool_calls(&server).await.len(), 1);
                            assert_eq!(
                                request.id,
                                codex_protocol::mcp::RequestId::String(format!(
                                    "codex_apps_auth_{CALL_ID}"
                                ))
                            );
                            auth_request = Some(request.request.clone());
                            ElicitationAction::Accept
                        }
                        ElicitationRequest::Form { .. }
                        | ElicitationRequest::OpenAiForm { .. }
                        | ElicitationRequest::OpenAiElicitationForm { .. } => {
                            assert!(modern, "modern approval requested for {scenario:?}");
                            assert!(recorded_apps_tool_calls(&server).await.is_empty());
                            approvals_seen += 1;
                            if approved {
                                ElicitationAction::Accept
                            } else {
                                ElicitationAction::Decline
                            }
                        }
                    };
                    test.codex
                        .submit(Op::ResolveElicitation {
                            server_name: request.server_name,
                            request_id: request.id,
                            decision: action,
                            content: None,
                            meta: None,
                        })
                        .await?;
                }
                EventMsg::TurnComplete(completed) => {
                    assert_eq!(completed.turn_id, turn_id);
                    client.track_notification(&app::ServerNotification::TurnCompleted(
                        serde_json::from_value(json!({
                            "threadId": thread_id,
                            "turn": {"id": completed.turn_id, "items": [], "status": "completed",
                                "startedAt": completed.started_at,
                                "completedAt": completed.completed_at,
                                "durationMs": completed.duration_ms},
                        }))?,
                    ));
                    break;
                }
                EventMsg::Error(error) => {
                    anyhow::bail!("unexpected turn error for {scenario:?}: {error:?}");
                }
                _ => {}
            }
        }
        Ok::<(), anyhow::Error>(())
    })
    .await??;

    assert_eq!(
        (approvals_seen, target_lifecycle),
        (expected_approvals, [1, 1])
    );
    let expected_auth_request = if matches!(scenario, Scenario::DefaultAuth | Scenario::ModernAuth)
    {
        Some(ElicitationRequest::Url {
            meta: Some(json!({
                "_codex_apps": {
                    "connector_auth_failure": {
                        "is_auth_failure": true,
                        "connector_id": "calendar",
                        "connector_name": "Calendar",
                        "install_url": "https://chatgpt.com/apps/calendar/calendar",
                        "auth_reason": "reauthentication_required",
                        "link_id": "link_123",
                        "error_code": "UNAUTHORIZED",
                        "error_http_status_code": 401,
                        "error_action": "TRIGGER_REAUTHENTICATION",
                    },
                },
            })),
            message: "Reconnect Calendar on ChatGPT to restore access for this request."
                .to_string(),
            url: "https://chatgpt.com/apps/calendar/calendar".to_string(),
            elicitation_id: format!("codex_apps_auth_{CALL_ID}"),
        })
    } else {
        None
    };
    assert_eq!(auth_request, expected_auth_request);
    let tool_calls = recorded_apps_tool_calls(&server).await;
    assert_eq!(tool_calls.len(), usize::from(approved));
    if approved {
        let request = recorded_apps_tool_call_by_call_id(&server, CALL_ID).await;
        assert_eq!(
            request.pointer("/params/name").and_then(Value::as_str),
            Some("calendar_create_event")
        );
    }
    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    if matches!(scenario, Scenario::DefaultAuth | Scenario::ModernAuth) {
        let output = requests[1].function_call_output(CALL_ID);
        let items = output["output"]
            .as_array()
            .expect("auth elicitation result should contain content items");
        assert_eq!(
            &items[1..],
            &[json!({
                "type": "input_text",
                "text": "Authentication for Calendar was requested and accepted. Retry this tool call now.",
            })]
        );
        assert!(!output.to_string().contains(PRIVATE_SENTINEL));
    }

    tokio::time::timeout(Duration::from_secs(10), client.flush()).await?;
    let mut events = Vec::new();
    for request in server.received_requests().await.unwrap_or_default() {
        if request.method != "POST" || request.url.path() != "/codex/analytics-events/events" {
            continue;
        }
        let payload: Value = serde_json::from_slice(&request.body)?;
        let batch = payload["events"].as_array().expect("analytics events");
        events.extend(batch.iter().cloned());
    }
    let mcp_events = events
        .iter()
        .filter(|event| {
            event["event_type"] == "codex_mcp_tool_call_event"
                && event["event_params"]["item_id"] == CALL_ID
        })
        .collect::<Vec<_>>();
    let app_used_events = events
        .iter()
        .filter(|event| {
            event["event_type"] == "codex_app_used"
                && event["event_params"]["connector_id"] == "calendar"
                && event["event_params"]["turn_id"] == turn_id
        })
        .collect::<Vec<_>>();
    assert_eq!(mcp_events.len(), 1);
    assert_eq!(app_used_events.len(), usize::from(approved));
    assert_eq!(mcp_events[0]["event_params"]["connector_id"], "calendar");
    assert_eq!(mcp_events[0]["event_params"]["thread_id"], thread_id);
    assert_eq!(mcp_events[0]["event_params"]["turn_id"], turn_id);
    assert_eq!(
        mcp_events[0]["event_params"]["terminal_status"],
        if scenario == Scenario::LegacySuccess {
            "completed"
        } else {
            "failed"
        }
    );
    let expected = match scenario {
        Scenario::DefaultAuth | Scenario::ModernAuth | Scenario::AuthMetadataRemoved => {
            json!("auth_or_link")
        }
        Scenario::LegacySuccess => Value::Null,
        Scenario::LegacyCancelled | Scenario::ModernDeclined => json!("approval"),
    };
    for event in mcp_events.iter().chain(app_used_events.iter()) {
        assert_eq!(
            event["event_params"].get("elicitation_type"),
            Some(&expected)
        );
    }
    let target_payload = serde_json::to_string(&(mcp_events, app_used_events))?;
    assert!(!target_payload.contains(PRIVATE_SENTINEL));
    assert!(!target_payload.contains("https://chatgpt.com/apps/"));

    Ok(())
}

#[derive(Clone, Copy)]
enum SubagentRequestKind {
    ToolApproval,
    LegacyToolApproval,
    ConnectorAuth(AuthFailureFormat),
}

#[test_case(SubagentRequestKind::ToolApproval, ElicitationAction::Accept; "child_tool_approval_waits_for_user_before_execution")]
#[test_case(SubagentRequestKind::LegacyToolApproval, ElicitationAction::Accept; "child_legacy_tool_approval_is_rejected_before_execution")]
#[test_case(SubagentRequestKind::ConnectorAuth(AuthFailureFormat::Text), ElicitationAction::Accept; "child_connector_auth_accepts_text_failure")]
#[test_case(SubagentRequestKind::ConnectorAuth(AuthFailureFormat::Structured), ElicitationAction::Accept; "child_connector_auth_accepts_structured_failure")]
#[test_case(SubagentRequestKind::ConnectorAuth(AuthFailureFormat::Structured), ElicitationAction::Decline; "child_declined_connector_auth_preserves_original_failure")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn core_generated_mcp_elicitations_support_subagents(
    kind: SubagentRequestKind,
    decision: ElicitationAction,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let apps = AppsTestServer::mount_searchable(&server).await?;
    if let SubagentRequestKind::ConnectorAuth(format) = kind {
        Mock::given(method("POST"))
            .and(path_regex("^/api/codex/ps/mcp/?$"))
            .and(body_partial_json(json!({
                "method": "tools/call",
                "params": {"name": "calendar_create_event"}
            })))
            .respond_with(AuthFailureResponder {
                scenario: Scenario::DefaultAuth,
                format,
            })
            .with_priority(/*p*/ 1)
            .mount(&server)
            .await;
    }
    let test = search_capable_apps_builder(apps.chatgpt_base_url)
        .with_config(move |config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::User;
            config
                .features
                .enable(Feature::AuthElicitation)
                .expect("enable auth elicitation");
            config
                .features
                .set_enabled(
                    Feature::ToolCallMcpElicitation,
                    !matches!(kind, SubagentRequestKind::LegacyToolApproval),
                )
                .expect("configure MCP approval transport");
            let approval_mode = match kind {
                SubagentRequestKind::ToolApproval | SubagentRequestKind::LegacyToolApproval => {
                    "prompt"
                }
                SubagentRequestKind::ConnectorAuth(_) => "approve",
            };
            let user_config = toml::from_str(&format!(
                r#"[apps.calendar]
default_tools_approval_mode = "{approval_mode}"
approvals_reviewer = "user"
"#
            ))
            .expect("apps config");
            config.config_layer_stack = config
                .config_layer_stack
                .with_user_config(&config.codex_home.join("config.toml").abs(), user_config)
                .expect("apply apps config");
        })
        .build_with_auto_env(&server)
        .await?;
    let child = test
        .thread_manager
        .start_thread(StartThreadOptions {
            session_source: Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id: test.session_configured.thread_id,
                depth: 1,
                agent_path: None,
                agent_nickname: None,
                agent_role: None,
            })),
            environments: Some(vec![test.executor_environment().selection().clone()]),
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?
        .thread;
    wait_for_mcp_server(&child, CODEX_APPS_MCP_SERVER_NAME).await?;
    responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_function_call_with_namespace(
                "calendar-call",
                SEARCH_CALENDAR_NAMESPACE,
                SEARCH_CALENDAR_CREATE_TOOL,
                &json!({"title": "Lunch", "starts_at": "2026-06-18T12:00:00Z"}).to_string(),
            ),
            responses::ev_completed("tool-response"),
        ]),
    )
    .await;
    let follow_up = responses::mount_sse_once(&server, responses::sse_completed("done")).await;
    child
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Use [$calendar](app://calendar) to create a calendar event.".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                approval_policy: Some(AskForApproval::OnRequest),
                permission_profile: Some(PermissionProfile::Disabled),
                ..Default::default()
            }),
        )
        .await?;
    let mut completed_status = None;
    let mut completed_result = None;
    let mut prompts = 0;
    loop {
        let event = wait_for_event(&child, |event| {
            assert!(
                !matches!(event, EventMsg::RequestUserInput(_)),
                "subagents must not fall back to request_user_input"
            );
            if let EventMsg::ItemCompleted(event) = event
                && let TurnItem::McpToolCall(item) = &event.item
                && item.id == "calendar-call"
            {
                completed_status = Some(item.status);
                completed_result = item.result.clone();
            }
            matches!(
                event,
                EventMsg::ElicitationRequest(_) | EventMsg::TurnComplete(_)
            )
        })
        .await;
        let EventMsg::ElicitationRequest(request) = event else {
            break;
        };
        assert_eq!(prompts, 0, "one tool call must not prompt twice");
        prompts += 1;
        assert_eq!(request.server_name, CODEX_APPS_MCP_SERVER_NAME);
        assert!(
            follow_up.requests().is_empty(),
            "the tool must wait for input"
        );
        match kind {
            SubagentRequestKind::ToolApproval => {
                assert!(matches!(request.request, ElicitationRequest::Form { .. }));
                assert!(recorded_apps_tool_calls(&server).await.is_empty());
            }
            SubagentRequestKind::ConnectorAuth(_) => {
                assert!(matches!(request.request, ElicitationRequest::Url { .. }));
                assert_eq!(recorded_apps_tool_calls(&server).await.len(), 1);
            }
            SubagentRequestKind::LegacyToolApproval => {
                panic!("legacy subagent approval must be rejected before prompting");
            }
        }
        child
            .submit(Op::ResolveElicitation {
                server_name: request.server_name,
                request_id: request.id,
                decision,
                content: None,
                meta: None,
            })
            .await?;
    }
    let expects_prompt = !matches!(kind, SubagentRequestKind::LegacyToolApproval);
    assert_eq!(prompts, usize::from(expects_prompt));
    assert_eq!(
        completed_status,
        Some(if matches!(kind, SubagentRequestKind::ToolApproval) {
            McpToolCallStatus::Completed
        } else {
            McpToolCallStatus::Failed
        })
    );
    assert_eq!(
        recorded_apps_tool_calls(&server).await.len(),
        usize::from(expects_prompt)
    );
    let output = follow_up
        .single_request()
        .function_call_output("calendar-call");
    let output_text = match &output["output"] {
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .filter_map(|item| item["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        other => panic!("unexpected MCP output: {other}"),
    };
    if matches!(kind, SubagentRequestKind::LegacyToolApproval) {
        assert!(output_text.contains("parent agent"));
    }
    if let SubagentRequestKind::ConnectorAuth(format) = kind {
        let original: CallToolResult = serde_json::from_value(
            AuthFailureResponder {
                scenario: Scenario::DefaultAuth,
                format,
            }
            .result(),
        )?;
        let expected_result = if decision == ElicitationAction::Accept {
            let text =
                "Authentication for Calendar was requested and accepted. Retry this tool call now.";
            assert!(output_text.contains(text));
            assert!(!output_text.contains(PRIVATE_SENTINEL));
            CallToolResult {
                content: vec![json!({"type": "text", "text": text})],
                structured_content: None,
                is_error: Some(true),
                meta: original.meta,
            }
        } else {
            assert!(output_text.contains("synthetic-structured-auth-failure"));
            original
        };
        assert_eq!(completed_result, Some(expected_result));
        assert!(
            !output_text.contains("link_123"),
            "private auth metadata must not reach the model"
        );
    }
    child.shutdown_and_wait().await?;
    test.codex.shutdown_and_wait().await?;
    Ok(())
}
