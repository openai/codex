#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used)]

use anyhow::Context;
use anyhow::Result;
use codex_config::types::AppToolApproval;
use codex_core::config::Config;
use codex_features::Feature;
use codex_protocol::approvals::ElicitationRequest;
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
use core_test_support::apps_test_server::CALENDAR_MCP_SERVER_NAME;
use core_test_support::apps_test_server::CALENDAR_UPSTREAM_ERROR_TITLE;
use core_test_support::apps_test_server::SEARCH_CALENDAR_CREATE_TOOL;
use core_test_support::apps_test_server::SEARCH_CALENDAR_EXTRACT_TEXT_TOOL;
use core_test_support::apps_test_server::SEARCH_CALENDAR_NAMESPACE;
use core_test_support::apps_test_server::recorded_apps_tool_call_by_call_id;
use core_test_support::apps_test_server::recorded_apps_tool_calls;
use core_test_support::apps_test_server::search_capable_apps_builder;
use core_test_support::apps_test_server::search_capable_apps_builder_with_analytics;
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
use core_test_support::wait_for_mcp_server_registration;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::time::Duration;
use tokio::sync::Notify;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::Request;
use wiremock::Respond;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

fn set_calendar_approval_mode(config: &mut Config, approval_mode: AppToolApproval) {
    let approval_mode = match approval_mode {
        AppToolApproval::Auto => "auto",
        AppToolApproval::Prompt => "prompt",
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

#[derive(Clone, Copy, Debug)]
enum AppsAnalyticsCase {
    Success,
    UpstreamError,
    Decline,
    Cancel,
}

impl AppsAnalyticsCase {
    fn slug(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::UpstreamError => "upstream-error",
            Self::Decline => "decline",
            Self::Cancel => "cancel",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::UpstreamError => CALENDAR_UPSTREAM_ERROR_TITLE,
            Self::Success | Self::Decline | Self::Cancel => "Lunch",
        }
    }

    fn approval_decision(self) -> Option<ElicitationAction> {
        match self {
            Self::Decline => Some(ElicitationAction::Decline),
            Self::Cancel => Some(ElicitationAction::Cancel),
            Self::Success | Self::UpstreamError => None,
        }
    }

    fn attempted(self) -> bool {
        matches!(self, Self::Success | Self::UpstreamError)
    }
}

#[derive(Clone, Default)]
struct AnalyticsCapture {
    inner: Arc<AnalyticsCaptureInner>,
}

#[derive(Default)]
struct AnalyticsCaptureInner {
    events: Mutex<Vec<serde_json::Value>>,
    changed: Notify,
}

impl AnalyticsCapture {
    async fn wait_for_app_mention_after(
        &self,
        thread_id: &str,
        completed_turn_id: &str,
    ) -> Vec<serde_json::Value> {
        timeout(Duration::from_secs(10), async {
            loop {
                let events = self
                    .inner
                    .events
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .clone();
                if events.iter().any(|event| {
                    event["event_type"] == "codex_app_mentioned"
                        && event["event_params"]["thread_id"] == thread_id
                        && event["event_params"]["turn_id"] != completed_turn_id
                }) {
                    return events;
                }
                self.inner.changed.notified().await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for analytics barrier: {thread_id}"))
    }
}

impl Respond for AnalyticsCapture {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let events = serde_json::from_slice::<serde_json::Value>(&request.body)
            .ok()
            .and_then(|payload| payload["events"].as_array().cloned())
            .unwrap_or_default();
        if !events.is_empty() {
            self.inner
                .events
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .extend(events);
            self.inner.changed.notify_one();
        }
        ResponseTemplate::new(200)
    }
}

async fn run_apps_analytics_case(case: AppsAnalyticsCase) -> Result<()> {
    let server = start_mock_server().await;
    let analytics = AnalyticsCapture::default();
    Mock::given(method("POST"))
        .and(path("/codex/analytics-events/events"))
        .respond_with(analytics.clone())
        .mount(&server)
        .await;
    let apps_server = AppsTestServer::mount(&server).await?;
    let call_id = format!("apps-analytics-{}", case.slug());
    let arguments = serde_json::to_string(&json!({
        "title": case.title(),
        "starts_at": "2026-03-10T12:00:00Z",
    }))?;
    mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-tool"),
                ev_function_call_with_namespace(
                    &call_id,
                    SEARCH_CALENDAR_NAMESPACE,
                    SEARCH_CALENDAR_CREATE_TOOL,
                    &arguments,
                ),
                ev_completed("resp-tool"),
            ]),
            sse(vec![
                ev_response_created("resp-done"),
                ev_assistant_message("msg-done", "done"),
                ev_completed("resp-done"),
            ]),
            sse(vec![
                ev_response_created("resp-barrier"),
                ev_assistant_message("msg-barrier", "barrier"),
                ev_completed("resp-barrier"),
            ]),
        ],
    )
    .await;

    let mut builder =
        search_capable_apps_builder_with_analytics(apps_server.chatgpt_base_url.clone())
            .with_config(move |config| {
                config
                    .features
                    .enable(Feature::ToolCallMcpElicitation)
                    .expect("test config should allow MCP approval elicitation");
                if case.approval_decision().is_some() {
                    set_default_app_approval_mode_and_reviewer(
                        config,
                        AppToolApproval::Prompt,
                        ApprovalsReviewer::User,
                    );
                } else {
                    set_calendar_approval_mode(config, AppToolApproval::Approve);
                }
            });
    let test = builder.build(&server).await?;
    wait_for_mcp_server_registration(&test.codex, CALENDAR_MCP_SERVER_NAME).await?;
    submit_user_turn(
        &test,
        "Use [$calendar](app://calendar) for this request.",
        if case.approval_decision().is_some() {
            AskForApproval::OnRequest
        } else {
            AskForApproval::Never
        },
        /*collaboration_mode*/ None,
    )
    .await?;

    if let Some(decision) = case.approval_decision() {
        let request = wait_for_event_match(&test.codex, |event| match event {
            EventMsg::ElicitationRequest(request) => Some(request.clone()),
            _ => None,
        })
        .await;
        test.codex
            .submit(Op::ResolveElicitation {
                server_name: request.server_name,
                request_id: request.id,
                decision,
                content: None,
                meta: None,
            })
            .await?;
    }

    let tool_end = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::McpToolCallEnd(end) if end.call_id == call_id => Some(end.clone()),
        _ => None,
    })
    .await;
    match case {
        AppsAnalyticsCase::Success => assert!(tool_end.is_success()),
        AppsAnalyticsCase::UpstreamError => assert_eq!(
            tool_end
                .result
                .as_ref()
                .expect("upstream MCP error should return a tool result")
                .is_error,
            Some(true),
        ),
        AppsAnalyticsCase::Decline | AppsAnalyticsCase::Cancel => {
            assert!(tool_end.result.is_err())
        }
    }

    let completed_turn_id = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TurnComplete(event) => Some(event.turn_id.clone()),
        _ => None,
    })
    .await;
    let thread_id = test.session_configured.thread_id.to_string();
    submit_user_turn(
        &test,
        "Use [$calendar](app://calendar) as an analytics barrier.",
        AskForApproval::Never,
        /*collaboration_mode*/ None,
    )
    .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let events = analytics
        .wait_for_app_mention_after(&thread_id, &completed_turn_id)
        .await;
    let used = events
        .iter()
        .filter(|event| {
            event["event_type"] == "codex_app_used"
                && event["event_params"]["turn_id"] == completed_turn_id
        })
        .collect::<Vec<_>>();
    assert_eq!(
        used.len(),
        usize::from(case.attempted()),
        "unexpected Apps usage analytics for {case:?}: {events:?}"
    );
    assert_eq!(
        recorded_apps_tool_calls(&server).await.len(),
        usize::from(case.attempted()),
        "unexpected upstream Apps attempts for {case:?}"
    );
    if let [event] = used.as_slice() {
        assert_eq!(event["event_params"]["connector_id"], "calendar");
        assert_eq!(event["event_params"]["app_name"], "Calendar");
        assert_eq!(event["event_params"]["invoke_type"], "explicit");
        assert_eq!(event["event_params"]["thread_id"], thread_id);
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apps_analytics_tracks_only_attempted_calls_through_the_host_lifecycle() -> Result<()> {
    skip_if_no_network!(Ok(()));

    for case in [
        AppsAnalyticsCase::Success,
        AppsAnalyticsCase::UpstreamError,
        AppsAnalyticsCase::Decline,
        AppsAnalyticsCase::Cancel,
    ] {
        run_apps_analytics_case(case)
            .await
            .with_context(|| format!("Apps analytics case {case:?}"))?;
    }
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
    wait_for_mcp_server_registration(&test.codex, CALENDAR_MCP_SERVER_NAME).await?;

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
async fn apps_always_approval_persists_raw_policy_and_survives_host_restart() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    let first_call_id = "calendar-always-approval-first";
    let second_call_id = "calendar-always-approval-after-restart";
    let extract_args = serde_json::to_string(&json!({
        "file": {
            "file_id": "file-already-uploaded"
        }
    }))?;
    mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-first-tool"),
                ev_function_call_with_namespace(
                    first_call_id,
                    SEARCH_CALENDAR_NAMESPACE,
                    SEARCH_CALENDAR_EXTRACT_TEXT_TOOL,
                    &extract_args,
                ),
                ev_completed("resp-first-tool"),
            ]),
            sse(vec![
                ev_response_created("resp-first-done"),
                ev_assistant_message("msg-first-done", "done"),
                ev_completed("resp-first-done"),
            ]),
            sse(vec![
                ev_response_created("resp-second-tool"),
                ev_function_call_with_namespace(
                    second_call_id,
                    SEARCH_CALENDAR_NAMESPACE,
                    SEARCH_CALENDAR_EXTRACT_TEXT_TOOL,
                    &extract_args,
                ),
                ev_completed("resp-second-tool"),
            ]),
            sse(vec![
                ev_response_created("resp-second-done"),
                ev_assistant_message("msg-second-done", "done"),
                ev_completed("resp-second-done"),
            ]),
        ],
    )
    .await;

    let home = Arc::new(tempfile::tempdir()?);
    let mut first_builder = search_capable_apps_builder(apps_server.chatgpt_base_url.clone())
        .with_home(Arc::clone(&home))
        .with_config(|config| {
            config
                .features
                .enable(Feature::ToolCallMcpElicitation)
                .expect("test config should allow feature update");
            set_default_app_approval_mode_and_reviewer(
                config,
                AppToolApproval::Auto,
                ApprovalsReviewer::User,
            );
        });
    let first = first_builder.build(&server).await?;
    wait_for_mcp_server_registration(&first.codex, CALENDAR_MCP_SERVER_NAME).await?;

    submit_user_turn(
        &first,
        "Use [$calendar](app://calendar) to extract text from the document.",
        AskForApproval::OnRequest,
        /*collaboration_mode*/ None,
    )
    .await?;

    let request = wait_for_event_match(&first.codex, |event| match event {
        EventMsg::ElicitationRequest(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    let ElicitationRequest::Form {
        meta: Some(request_meta),
        ..
    } = &request.request
    else {
        panic!("expected an MCP approval form with persistence metadata");
    };
    assert!(
        request_meta
            .get(codex_protocol::mcp_approval_meta::PERSIST_KEY)
            .and_then(serde_json::Value::as_array)
            .is_some_and(|choices| {
                choices.iter().any(|choice| {
                    choice.as_str() == Some(codex_protocol::mcp_approval_meta::PERSIST_ALWAYS)
                })
            }),
        "the Apps-owned runtime persistence callback must reach the generic MCP approval form"
    );

    first
        .codex
        .submit(Op::ResolveElicitation {
            server_name: request.server_name,
            request_id: request.id,
            decision: ElicitationAction::Accept,
            content: None,
            meta: Some(json!({
                codex_protocol::mcp_approval_meta::PERSIST_KEY:
                    codex_protocol::mcp_approval_meta::PERSIST_ALWAYS,
            })),
        })
        .await?;
    wait_for_event(&first.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let persisted = std::fs::read_to_string(home.path().join("config.toml"))?;
    let persisted = toml::from_str::<toml::Value>(&persisted)?;
    assert_eq!(
        persisted["apps"]["calendar"]["tools"]["calendar_extract_text"]["approval_mode"].as_str(),
        Some("approve"),
        "the extension must persist the raw upstream tool name"
    );
    assert!(
        persisted["apps"]["calendar"]["tools"]
            .as_table()
            .is_some_and(|tools| !tools.contains_key("extract_text")),
        "the exposed MCP alias must not leak into Apps policy"
    );
    recorded_apps_tool_call_by_call_id(&server, first_call_id).await;

    first.codex.shutdown_and_wait().await?;
    drop(first);

    let mut second_builder = search_capable_apps_builder(apps_server.chatgpt_base_url.clone())
        .with_home(Arc::clone(&home))
        .with_config(|config| {
            config
                .features
                .enable(Feature::ToolCallMcpElicitation)
                .expect("test config should allow feature update");
        });
    let second = second_builder.build(&server).await?;
    wait_for_mcp_server_registration(&second.codex, CALENDAR_MCP_SERVER_NAME).await?;
    submit_user_turn(
        &second,
        "Use [$calendar](app://calendar) to extract text from the document again.",
        AskForApproval::OnRequest,
        /*collaboration_mode*/ None,
    )
    .await?;

    let terminal_event = wait_for_event(&second.codex, |event| {
        matches!(
            event,
            EventMsg::ElicitationRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;
    assert!(
        matches!(terminal_event, EventMsg::TurnComplete(_)),
        "the persisted Apps policy should suppress approval after a full host restart"
    );
    recorded_apps_tool_call_by_call_id(&server, second_call_id).await;

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
    wait_for_mcp_server_registration(&test.codex, CALENDAR_MCP_SERVER_NAME).await?;

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
    assert!(guardian_request.body_contains_text(SEARCH_CALENDAR_CREATE_TOOL));
    assert!(guardian_request.body_contains_text(CALENDAR_MCP_SERVER_NAME));
    assert!(guardian_request.body_contains_text("Create a calendar event."));
    assert!(guardian_request.body_contains_text("Lunch"));

    let apps_tool_call = recorded_apps_tool_call_by_call_id(&server, call_id).await;
    assert_eq!(
        apps_tool_call.pointer("/params/arguments/title"),
        Some(&json!("Lunch"))
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
    wait_for_mcp_server_registration(&test.codex, CALENDAR_MCP_SERVER_NAME).await?;

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
