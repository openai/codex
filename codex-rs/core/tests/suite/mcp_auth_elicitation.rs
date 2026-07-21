#![allow(clippy::unwrap_used)]

use anyhow::Result;
use codex_core::config::Constrained;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_protocol::approvals::ElicitationRequest;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ElicitationAction;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::PathExt;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::SEARCH_CALENDAR_CREATE_TOOL;
use core_test_support::apps_test_server::SEARCH_CALENDAR_NAMESPACE;
use core_test_support::apps_test_server::search_capable_apps_builder;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use wiremock::Mock;
use wiremock::Request;
use wiremock::Respond;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::method;
use wiremock::matchers::path_regex;

#[derive(Clone, Copy)]
struct AuthFailureResponder;

impl Respond for AuthFailureResponder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: Value =
            serde_json::from_slice(&request.body).expect("tools/call request should be valid JSON");
        let id = body.get("id").cloned().unwrap_or(Value::Null);

        ResponseTemplate::new(/*status*/ 200).set_body_json(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{
                    "type": "text",
                    "text": "Connector reauthentication required",
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
            },
        }))
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn codex_apps_auth_failure_requests_elicitation_by_default() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount_searchable(&server).await?;
    Mock::given(method("POST"))
        .and(path_regex("^/api/codex/ps/mcp/?$"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": {
                "name": "calendar_create_event",
            },
        })))
        .respond_with(AuthFailureResponder)
        .with_priority(/*p*/ 1)
        .mount(&server)
        .await;

    let call_id = "calendar-auth-call";
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call_with_namespace(
                    call_id,
                    SEARCH_CALENDAR_NAMESPACE,
                    SEARCH_CALENDAR_CREATE_TOOL,
                    &json!({
                        "title": "Lunch",
                        "starts_at": "2026-06-18T12:00:00Z",
                    })
                    .to_string(),
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

    let mut builder =
        search_capable_apps_builder(apps_server.chatgpt_base_url).with_config(|config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            let user_config_path = config.codex_home.join("config.toml").abs();
            let user_config = toml::from_str(
                r#"
[apps.calendar]
default_tools_approval_mode = "auto"
"#,
            )
            .expect("apps config should parse");
            config.config_layer_stack = config
                .config_layer_stack
                .with_user_config(&user_config_path, user_config)
                .expect("apps user config should be valid");
        });
    let test = builder.build(&server).await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "Use [$calendar](app://calendar) to create a calendar event.".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;

    let EventMsg::ElicitationRequest(request) = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ElicitationRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await
    else {
        panic!("default auth elicitation should prompt before completing the turn");
    };

    assert_eq!(request.server_name, CODEX_APPS_MCP_SERVER_NAME);
    assert_eq!(
        request.id,
        codex_protocol::mcp::RequestId::String(format!("codex_apps_auth_{call_id}"))
    );
    assert_eq!(
        request.request,
        ElicitationRequest::Url {
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
            elicitation_id: format!("codex_apps_auth_{call_id}"),
        }
    );

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

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let output = requests[1]
        .function_call_output_text(call_id)
        .expect("follow-up request should contain the auth elicitation result");
    assert!(output.contains(
        "Authentication for Calendar was requested and accepted. Retry this tool call now."
    ));
    assert!(!output.contains("Connector reauthentication required"));

    Ok(())
}
