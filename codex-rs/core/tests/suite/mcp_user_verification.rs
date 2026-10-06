//! Configured MCP servers cannot activate verification or hold a turn waiting for proof.

use anyhow::Result;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_protocol::mcp::ClientMcpExtensions;
use codex_protocol::mcp::OPENAI_ELICITATION_EXTENSION_ID;
use codex_protocol::protocol::EventMsg;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use test_case::test_case;

const SERVER: &str = r#"
import json
import sys

def send(message):
    print(json.dumps({"jsonrpc": "2.0", **message}), flush=True)

extensions = {}
pending = None
for line in sys.stdin:
    request = json.loads(line)
    method = request.get("method")
    if method == "initialize":
        extensions = request["params"]["capabilities"].get("extensions", {})
        result = {"protocolVersion": request["params"]["protocolVersion"],
                  "capabilities": {"tools": {}},
                  "serverInfo": {"name": "verification-test", "version": "1"}}
    elif method == "tools/list":
        result = {"tools": [{"name": "verify", "inputSchema": {"type": "object", "properties": {}},
                             "annotations": {"readOnlyHint": True}}]}
    elif method == "tools/call":
        pending = request["id"]
        send({"id": "verification", "method": "openai/elicitation/create", "params": {
            "mode": "openai/userVerification", "title": "Approve", "description": "", "challenge": "AQID"}})
        continue
    elif method is None and request.get("id") == "verification":
        send({"id": pending, "result": {"content": [{"type": "text", "text": json.dumps({
            "extensions": extensions, "response": request.get("result", request.get("error"))})}]}})
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
enum CapabilitySource {
    HostProjection,
    ExplicitSession,
}

#[test_case(CapabilitySource::HostProjection; "host_projection_does_not_advertise_verification")]
#[test_case(CapabilitySource::ExplicitSession; "explicit_session_cannot_enable_configured_server")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_user_verification_rejects_configured_servers(source: CapabilitySource) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(Ok(()), "the MCP fixture requires a host Python interpreter");
    let server = responses::start_mock_server().await;
    let test = test_codex().build_with_auto_env(&server).await?;
    let declarations = HashMap::from([(
        OPENAI_ELICITATION_EXTENSION_ID.to_string(),
        json!({"userVerification": {}}),
    )]);
    let client_mcp_extensions = match source {
        CapabilitySource::HostProjection => codex_mcp::client_mcp_extensions(
            Some(&declarations),
            /*legacy_openai_form_elicitation*/ false,
        ),
        // Even an explicitly enabled session cannot grant this capability to a configured server.
        CapabilitySource::ExplicitSession => ClientMcpExtensions::new(declarations),
    };
    let mut config = test.config.clone();
    config.mcp_servers.set(serde_json::from_value(json!({
        "verification": {
            "command": if cfg!(windows) { "python" } else { "python3" },
            "args": ["-u", "-c", SERVER], "default_tools_approval_mode": "approve"
        }
    }))?)?;
    let thread = test
        .thread_manager
        .start_thread(StartThreadOptions {
            client_mcp_extensions,
            environments: Some(vec![test.executor_environment().request()]),
            ..StartThreadOptions::new(config)
        })
        .await?
        .thread;
    wait_for_mcp_server(&thread, "verification").await?;
    responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_function_call_with_namespace(
                "verify-call",
                "mcp__verification",
                "verify",
                "{}",
            ),
            responses::ev_completed("tool-response"),
        ]),
    )
    .await;
    let follow_up = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message("done-message", "done"),
            responses::ev_completed("done-response"),
        ]),
    )
    .await;
    thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Run verification".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&thread, |event| {
        assert!(
            !matches!(event, EventMsg::ElicitationRequest(_)),
            "configured servers must not prompt for verification"
        );
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let output = follow_up
        .single_request()
        .function_call_output("verify-call");
    let echoed: Value = serde_json::from_str(
        output["output"][1]["text"]
            .as_str()
            .expect("MCP wire response"),
    )?;
    assert!(
        echoed["extensions"][OPENAI_ELICITATION_EXTENSION_ID]
            .get("userVerification")
            .is_none()
    );
    assert_eq!(
        echoed["response"],
        json!({"code": -32601, "message": "openai/elicitation/create"})
    );
    thread.shutdown_and_wait().await?;
    test.codex.shutdown_and_wait().await?;
    Ok(())
}
