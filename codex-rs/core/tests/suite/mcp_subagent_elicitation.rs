//! MCP requests keep human input on the root and allow automatic approval in subagents.

use anyhow::Result;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_protocol::approvals::ElicitationAction;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ResponsesRequest;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use test_case::test_case;
use wiremock::matchers::body_partial_json;

const ROOT_ONLY_MESSAGE: &str = codex_mcp::MCP_ELICITATION_HANDOFF_MESSAGE;

const SERVER: &str = r#"
import json
import sys

def send(message):
    print(json.dumps({"jsonrpc": "2.0", **message}), flush=True)

elicitation = json.loads(sys.argv[1])
pending = None
for line in sys.stdin:
    request = json.loads(line)
    method = request.get("method")
    if method == "initialize":
        result = {"protocolVersion": request["params"]["protocolVersion"],
                  "capabilities": {"tools": {}},
                  "serverInfo": {"name": "elicitation-test", "version": "1"}}
    elif method == "tools/list":
        result = {"tools": [{"name": "request_input", "inputSchema": {"type": "object", "properties": {}},
                             "annotations": {"readOnlyHint": True}}]}
    elif method == "tools/call":
        pending = request["id"]
        send({"id": "input-request", "method": "elicitation/create", "params": elicitation})
        continue
    elif method is None and request.get("id") == "input-request":
        send({"id": pending, "result": {"content": [{"type": "text", "text": json.dumps(
            request.get("result", request.get("error")))}]}})
        continue
    elif method == "resources/list":
        result = {"resources": []}
    elif method == "resources/templates/list":
        result = {"resourceTemplates": []}
    elif "id" not in request:
        continue
    else:
        result = {}
    send({"id": request["id"], "result": result})
"#;

#[derive(Clone, Copy)]
pub(super) enum Caller {
    Root,
    Subagent,
    FullAccessSubagent,
}

#[derive(Clone, Copy)]
pub(super) enum RequestKind {
    BrowserAuth,
    Permission,
    StrictReview,
}

#[test_case(Caller::FullAccessSubagent, RequestKind::BrowserAuth; "full_access_cannot_auto_accept_subagent_browser_auth")]
#[test_case(Caller::Root, RequestKind::BrowserAuth; "root_browser_auth_remains_interactive")]
#[test_case(Caller::FullAccessSubagent, RequestKind::Permission; "full_access_subagent_automatically_approves_permission")]
#[test_case(Caller::Subagent, RequestKind::StrictReview; "subagent_keeps_strict_automatic_safety_review")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_server_elicitations_keep_user_interaction_on_root(
    caller: Caller,
    request_kind: RequestKind,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(Ok(()), "the MCP fixture requires a host Python interpreter");
    mcp_server_elicitation_scenario(caller, request_kind).await?;
    Ok(())
}

pub(super) async fn mcp_server_elicitation_scenario(
    caller: Caller,
    request_kind: RequestKind,
) -> Result<Vec<ResponsesRequest>> {
    let server = responses::start_mock_server().await;
    let test = test_codex().build_with_auto_env(&server).await?;
    let mut elicitation = json!({
        "mode": "form",
        "message": "Provide the requested input.",
        "requestedSchema": {"type": "object", "properties": {}}
    });
    match request_kind {
        RequestKind::BrowserAuth => {
            // Browser credentials travel through the browser broker, so the MCP schema is empty.
            elicitation["_meta"] = json!({
                "codex_approval_kind": "browser_auth",
                "browser_auth_challenge_id": "test_browser_auth_challenge_0123456789",
                "origin": "https://example.com",
                "reason": "Sign in to continue browsing.",
                "fields": [{"id": "password", "label": "Password", "type": "password", "required": true}]
            });
        }
        RequestKind::Permission | RequestKind::StrictReview => {
            elicitation["_meta"] = json!({
                "codex_request_type": "approval_request",
                "codex_approval_kind": "mcp_tool_call",
                "codex_strict_auto_review": matches!(request_kind, RequestKind::StrictReview),
                "codex_sensitive_action": true,
                "tool_name": "write_record",
                "tool_params": {"value": 42}
            });
        }
    }
    let mut config = test.config.clone();
    config.approvals_reviewer = ApprovalsReviewer::User;
    config.mcp_servers.set(serde_json::from_value(json!({
        "elicitation": {
            "command": if cfg!(windows) { "python" } else { "python3" },
            "args": ["-u", "-c", SERVER, elicitation.to_string()],
            "default_tools_approval_mode": "approve"
        }
    }))?)?;
    let expects_prompt = matches!(caller, Caller::Root);
    let session_source = if expects_prompt {
        SessionSource::Exec
    } else {
        SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id: test.session_configured.thread_id,
            depth: 1,
            agent_path: None,
            agent_nickname: None,
            agent_role: None,
        })
    };
    let thread = test
        .thread_manager
        .start_thread(StartThreadOptions {
            session_source: Some(session_source),
            environments: Some(vec![test.executor_environment().selection().clone()]),
            ..StartThreadOptions::new(config)
        })
        .await?
        .thread;
    wait_for_mcp_server(&thread, "elicitation").await?;
    let tool_call = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_function_call_with_namespace(
                "input-call",
                "mcp__elicitation",
                "request_input",
                "{}",
            ),
            responses::ev_completed("tool-response"),
        ]),
    )
    .await;
    let guardian = if matches!(request_kind, RequestKind::StrictReview) {
        Some(
            responses::mount_sse_once_match(
                &server,
                body_partial_json(json!({"client_metadata": {"x-openai-subagent": "guardian"}})),
                responses::sse(vec![
                    responses::ev_assistant_message(
                        "review-result",
                        &json!({
                            "risk_level": "low", "user_authorization": "high", "outcome": "allow",
                            "rationale": "The user requested this action."
                        })
                        .to_string(),
                    ),
                    responses::ev_completed("guardian-review"),
                ]),
            )
            .await,
        )
    } else {
        None
    };
    let follow_up = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message("done-message", "done"),
            responses::ev_completed("done-response"),
        ]),
    )
    .await;
    let full_access = matches!(caller, Caller::FullAccessSubagent);
    thread
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Run the input tool.".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                approval_policy: Some(if full_access {
                    AskForApproval::Never
                } else {
                    AskForApproval::OnRequest
                }),
                permission_profile: Some(if full_access {
                    PermissionProfile::Disabled
                } else {
                    PermissionProfile::read_only()
                }),
                ..Default::default()
            }),
        )
        .await?;
    let mut prompts = 0;
    loop {
        let event = wait_for_event(&thread, |event| {
            matches!(
                event,
                EventMsg::ElicitationRequest(_) | EventMsg::TurnComplete(_)
            )
        })
        .await;
        let EventMsg::ElicitationRequest(request) = event else {
            break;
        };
        assert!(
            expects_prompt,
            "subagent MCP elicitations must not reach the user"
        );
        assert_eq!(prompts, 0, "one tool call must not prompt twice");
        prompts += 1;
        assert!(
            follow_up.requests().is_empty(),
            "the tool must wait for input"
        );
        thread
            .submit(Op::ResolveElicitation {
                server_name: request.server_name,
                request_id: request.id,
                decision: ElicitationAction::Accept,
                content: Some(json!({})),
                meta: None,
            })
            .await?;
    }
    assert_eq!(prompts, usize::from(expects_prompt));
    let output = follow_up
        .single_request()
        .function_call_output("input-call");
    let response: Value = serde_json::from_str(
        output["output"][1]["text"]
            .as_str()
            .expect("MCP response must reach the model"),
    )?;
    let expected = if matches!(request_kind, RequestKind::StrictReview) {
        json!({
            "action": "accept", "content": {},
            "_meta": {"approvals_reviewer": "auto_review"}
        })
    } else if expects_prompt || full_access && matches!(request_kind, RequestKind::Permission) {
        json!({"action": "accept", "content": {}})
    } else {
        json!({"code": -32603, "message": ROOT_ONLY_MESSAGE})
    };
    assert_eq!(response, expected);
    let mut requests = tool_call.requests();
    if let Some(guardian) = guardian {
        assert!(guardian.single_request().body_contains_text("write_record"));
        requests.extend(guardian.requests());
    }
    requests.extend(follow_up.requests());
    thread.shutdown_and_wait().await?;
    test.codex.shutdown_and_wait().await?;
    Ok(requests)
}
