#![allow(clippy::expect_used, clippy::unwrap_used)]

use anyhow::Result;
use codex_connectors::metadata::connector_install_url;
use codex_features::Feature;
use codex_protocol::approvals::ElicitationRequest;
use codex_protocol::approvals::ElicitationRequestEvent;
use codex_protocol::mcp::RequestId;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ElicitationAction;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::apps_enabled_builder;
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
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::method;
use wiremock::matchers::path_regex;

const SITES_CONNECTOR_ID: &str = "connector_20205bf7d4e99a89d7154bb849718324";
const SITES_CREATE_TOOL: &str = "_create_site";
const SITES_CREATE_TOOL_NAME: &str = "sites_create_site";
const SITES_NAMESPACE: &str = "mcp__codex_apps__sites";
const SITES_TERMS_AUTH_REASON: &str = "sites_publication_terms_required:2026-06-12";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sites_terms_elicitation_is_emitted_on_first_tool_call_under_never_policy() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;

    Mock::given(method("POST"))
        .and(path_regex("^/api/codex/apps/?$"))
        .and(body_partial_json(json!({"method": "tools/list"})))
        .respond_with(|request: &Request| {
            let body: Value = serde_json::from_slice(&request.body).expect("valid JSON-RPC body");
            ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0",
                "id": body.get("id").cloned().unwrap_or(Value::Null),
                "result": {
                    "tools": [{
                        "name": SITES_CREATE_TOOL_NAME,
                        "description": "Create a website.",
                        "annotations": {
                            "readOnlyHint": false,
                            "destructiveHint": false,
                            "openWorldHint": false
                        },
                        "inputSchema": {
                            "type": "object",
                            "properties": {},
                            "additionalProperties": false
                        },
                        "_meta": {
                            "connector_id": SITES_CONNECTOR_ID,
                            "connector_name": "Sites",
                            "connector_description": "Create and publish websites.",
                            "_codex_apps": {
                                "connector_id": SITES_CONNECTOR_ID
                            }
                        }
                    }],
                    "nextCursor": null
                }
            }))
        })
        .with_priority(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path_regex("^/api/codex/apps/?$"))
        .and(body_partial_json(json!({
            "method": "tools/call",
            "params": {"name": SITES_CREATE_TOOL_NAME}
        })))
        .respond_with(|request: &Request| {
            let body: Value = serde_json::from_slice(&request.body).expect("valid JSON-RPC body");
            ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0",
                "id": body.get("id").cloned().unwrap_or(Value::Null),
                "result": {
                    "content": [{
                        "type": "text",
                        "text": "Sites terms acceptance required"
                    }],
                    "isError": true,
                    "_meta": {
                        "_codex_apps": {
                            "connector_auth_failure": {
                                "is_auth_failure": true,
                                "auth_reason": SITES_TERMS_AUTH_REASON,
                                "connector_id": SITES_CONNECTOR_ID,
                                "connector_name": "Untrusted Sites",
                                "link_id": "link_sites",
                                "error_code": "TERMS_REQUIRED",
                                "error_http_status_code": 428,
                                "error_action": "TRIGGER_TERMS_ACCEPTANCE"
                            }
                        }
                    }
                }
            }))
        })
        .with_priority(1)
        .expect(1)
        .mount(&server)
        .await;

    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call_with_namespace(
                    "sites-call-1",
                    SITES_NAMESPACE,
                    SITES_CREATE_TOOL,
                    "{}",
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

    let mut builder = apps_enabled_builder(apps_server.chatgpt_base_url).with_config(|config| {
        config
            .features
            .enable(Feature::AuthElicitation)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "Create a site.".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                approval_policy: Some(AskForApproval::Never),
                ..Default::default()
            },
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
        panic!("expected Sites terms elicitation before the turn completed");
    };

    assert!(request.turn_id.is_some());
    let install_url = connector_install_url("Sites", SITES_CONNECTOR_ID);
    assert_eq!(
        request,
        ElicitationRequestEvent {
            turn_id: request.turn_id.clone(),
            server_name: "codex_apps".to_string(),
            id: RequestId::String("codex_apps_auth_sites-call-1".to_string()),
            request: ElicitationRequest::Url {
                meta: Some(json!({
                    "_codex_apps": {
                        "connector_auth_failure": {
                            "is_auth_failure": true,
                            "connector_id": SITES_CONNECTOR_ID,
                            "connector_name": "Sites",
                            "install_url": install_url,
                            "auth_reason": SITES_TERMS_AUTH_REASON,
                            "link_id": "link_sites",
                            "error_code": "TERMS_REQUIRED",
                            "error_http_status_code": 428,
                            "error_action": "TRIGGER_TERMS_ACCEPTANCE"
                        }
                    }
                })),
                message: "Review the ChatGPT Sites Terms to continue.".to_string(),
                url: install_url.clone(),
                elicitation_id: "codex_apps_auth_sites-call-1".to_string(),
            },
        }
    );

    test.codex
        .submit(Op::ResolveElicitation {
            server_name: request.server_name,
            request_id: request.id,
            decision: ElicitationAction::Decline,
            content: None,
            meta: None,
        })
        .await?;

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(responses.requests().len(), 2);

    Ok(())
}
