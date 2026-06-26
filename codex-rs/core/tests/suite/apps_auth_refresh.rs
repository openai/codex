use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use anyhow::Result;
use codex_features::Feature;
use codex_protocol::approvals::ElicitationAction;
use codex_protocol::approvals::ElicitationRequest;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::CALENDAR_MCP_SERVER_NAME;
use core_test_support::apps_test_server::recorded_apps_tool_call_by_call_id;
use core_test_support::apps_test_server::search_capable_apps_builder;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::ev_tool_search_call;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::namespace_child_tool;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use core_test_support::wait_for_mcp_server_registration;
use serde_json::Value;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::Respond;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path_regex;

const CALENDAR_NAMESPACE: &str = "mcp__codex_apps__calendar";
const CALENDAR_TOOL: &str = "_requires_auth";
const CALENDAR_UPSTREAM_TOOL: &str = "calendar_requires_auth";
const GMAIL_NAMESPACE: &str = "mcp__codex_apps__gmail";
const GMAIL_TOOL: &str = "_search";
const GMAIL_UPSTREAM_TOOL: &str = "gmail_search";

#[derive(Default)]
struct AuthRefreshState {
    tools_list_calls: AtomicUsize,
}

#[derive(Clone)]
struct AuthRefreshResponder {
    state: Arc<AuthRefreshState>,
}

impl Respond for AuthRefreshResponder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: Value = serde_json::from_slice(&request.body).expect("valid JSON-RPC request");
        let method = body
            .get("method")
            .and_then(Value::as_str)
            .expect("JSON-RPC method");
        let id = body.get("id").cloned().unwrap_or(Value::Null);

        match method {
            "initialize" => ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": body
                        .pointer("/params/protocolVersion")
                        .and_then(Value::as_str)
                        .unwrap_or("2025-11-25"),
                    "capabilities": { "tools": { "listChanged": true } },
                    "serverInfo": { "name": "apps-auth-refresh-test", "version": "1.0.0" }
                }
            })),
            "tools/list" => {
                let first_list = self.state.tools_list_calls.fetch_add(1, Ordering::AcqRel) == 0;
                let (connector_id, connector_name, upstream_tool) = if first_list {
                    ("calendar", "Calendar", CALENDAR_UPSTREAM_TOOL)
                } else {
                    ("gmail", "Gmail", GMAIL_UPSTREAM_TOOL)
                };
                ResponseTemplate::new(200).set_body_json(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "tools": [{
                            "name": upstream_tool,
                            "description": format!("Call {connector_name}."),
                            "annotations": { "readOnlyHint": true },
                            "inputSchema": {
                                "type": "object",
                                "properties": {},
                                "additionalProperties": false
                            },
                            "_meta": {
                                "connector_id": connector_id,
                                "connector_name": connector_name,
                                "connector_description": format!("{connector_name} connector")
                            }
                        }],
                        "nextCursor": null
                    }
                }))
            }
            "tools/call" => {
                let upstream_tool = body
                    .pointer("/params/name")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if upstream_tool == CALENDAR_UPSTREAM_TOOL {
                    ResponseTemplate::new(200).set_body_json(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": "sign in required" }],
                            "isError": true,
                            "_meta": {
                                "_codex_apps": {
                                    "connector_auth_failure": {
                                        "is_auth_failure": true,
                                        "connector_id": "calendar",
                                        "auth_reason": "missing_link",
                                        "error_code": "AUTH_REQUIRED"
                                    }
                                }
                            }
                        }
                    }))
                } else {
                    ResponseTemplate::new(200).set_body_json(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": "gmail search complete" }],
                            "isError": false
                        }
                    }))
                }
            }
            method if method.starts_with("notifications/") => ResponseTemplate::new(202),
            _ => ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("method not found: {method}") }
            })),
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn accepted_apps_auth_refresh_replaces_namespaces_at_next_sample() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let responses_server = MockServer::start().await;
    let apps_host = MockServer::start().await;
    let apps_server = AppsTestServer::mount(&apps_host).await?;
    let state = Arc::new(AuthRefreshState::default());
    Mock::given(method("POST"))
        .and(path_regex("^/api/codex/ps/mcp/?$"))
        .respond_with(AuthRefreshResponder {
            state: Arc::clone(&state),
        })
        .with_priority(1)
        .mount(&apps_host)
        .await;

    let calendar_call_id = "calendar-auth-call";
    let gmail_call_id = "gmail-search-call";
    let calendar_search_id = "calendar-auth-search";
    let gmail_search_id = "gmail-search";
    let responses = mount_sse_sequence(
        &responses_server,
        vec![
            sse(vec![
                ev_response_created("auth-refresh-1"),
                ev_tool_search_call(
                    calendar_search_id,
                    &json!({ "query": CALENDAR_UPSTREAM_TOOL, "limit": 1 }),
                ),
                ev_completed("auth-refresh-1"),
            ]),
            sse(vec![
                ev_response_created("auth-refresh-2"),
                ev_function_call_with_namespace(
                    calendar_call_id,
                    CALENDAR_NAMESPACE,
                    CALENDAR_TOOL,
                    "{}",
                ),
                ev_completed("auth-refresh-2"),
            ]),
            sse(vec![
                ev_response_created("auth-refresh-3"),
                ev_tool_search_call(
                    gmail_search_id,
                    &json!({ "query": GMAIL_UPSTREAM_TOOL, "limit": 1 }),
                ),
                ev_completed("auth-refresh-3"),
            ]),
            sse(vec![
                ev_response_created("auth-refresh-4"),
                ev_function_call_with_namespace(gmail_call_id, GMAIL_NAMESPACE, GMAIL_TOOL, "{}"),
                ev_completed("auth-refresh-4"),
            ]),
            sse(vec![
                ev_response_created("auth-refresh-5"),
                ev_assistant_message("auth-refresh-message", "done"),
                ev_completed("auth-refresh-5"),
            ]),
        ],
    )
    .await;

    let mut builder =
        search_capable_apps_builder(apps_server.chatgpt_base_url).with_config(|config| {
            config
                .features
                .enable(Feature::AuthElicitation)
                .expect("test config should allow auth elicitation");
        });
    let test = builder.build(&responses_server).await?;
    wait_for_mcp_server_registration(&test.codex, CALENDAR_MCP_SERVER_NAME).await?;
    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "Call the connected app, authenticate if needed, then use the refreshed app."
                    .to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;

    let elicitation = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::ElicitationRequest(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    assert_eq!(elicitation.server_name, CALENDAR_MCP_SERVER_NAME);
    let ElicitationRequest::Url {
        url,
        elicitation_id,
        ..
    } = &elicitation.request
    else {
        panic!("Apps auth failure should request URL elicitation");
    };
    assert_eq!(url, "https://chatgpt.com/apps/calendar/calendar");
    assert!(elicitation_id.starts_with("codex_apps_auth_"));
    assert_eq!(state.tools_list_calls.load(Ordering::Acquire), 1);
    assert_eq!(responses.requests().len(), 2);

    test.codex
        .submit(Op::ResolveElicitation {
            server_name: elicitation.server_name,
            request_id: elicitation.id,
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
    assert_eq!(requests.len(), 5);
    let calendar_search = requests[1].tool_search_output(calendar_search_id);
    assert!(
        namespace_child_tool(&calendar_search, CALENDAR_NAMESPACE, CALENDAR_TOOL).is_some(),
        "Calendar auth tool missing from first search: {calendar_search:?}"
    );
    assert!(namespace_child_tool(&calendar_search, GMAIL_NAMESPACE, GMAIL_TOOL).is_none());
    let gmail_search = requests[3].tool_search_output(gmail_search_id);
    assert!(namespace_child_tool(&gmail_search, CALENDAR_NAMESPACE, CALENDAR_TOOL).is_none());
    assert!(
        namespace_child_tool(&gmail_search, GMAIL_NAMESPACE, GMAIL_TOOL).is_some(),
        "Gmail tool missing from refreshed search: {gmail_search:?}"
    );

    let calendar_call = recorded_apps_tool_call_by_call_id(&apps_host, calendar_call_id).await;
    assert_eq!(
        calendar_call
            .pointer("/params/name")
            .and_then(Value::as_str),
        Some(CALENDAR_UPSTREAM_TOOL)
    );
    let gmail_call = recorded_apps_tool_call_by_call_id(&apps_host, gmail_call_id).await;
    assert_eq!(
        gmail_call.pointer("/params/name").and_then(Value::as_str),
        Some(GMAIL_UPSTREAM_TOOL)
    );
    assert_eq!(state.tools_list_calls.load(Ordering::Acquire), 2);

    Ok(())
}
