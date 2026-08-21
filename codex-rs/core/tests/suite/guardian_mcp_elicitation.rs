use anyhow::Context;
use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_core::config::Constrained;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::GuardianAssessmentStatus;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_mcp_server;
use serde_json::Value;
use serde_json::json;
use std::time::Duration;

// The tool itself needs no approval. Its server requests a separate Guardian
// review after tools/call, exercising the ordinary MCP elicitation path.
const ELICITATION_SERVER: &str = r#"
import json
import sys

def send(message):
    print(json.dumps({"jsonrpc": "2.0", **message}), flush=True)

pending_call = None
for line in sys.stdin:
    request = json.loads(line)
    method = request.get("method")
    if method == "initialize":
        result = {"protocolVersion": request["params"]["protocolVersion"],
                  "capabilities": {"tools": {}},
                  "serverInfo": {"name": "guardian-elicitation-test", "version": "1"}}
    elif method == "tools/list":
        result = {"tools": [{"name": "request_approval",
                  "inputSchema": {"type": "object", "properties": {}},
                  "annotations": {"readOnlyHint": True}}]}
    elif method == "tools/call":
        pending_call = request["id"]
        send({"id": "server-approval", "method": "elicitation/create", "params": {
            "message": "Approve the server-side action?",
            "requestedSchema": {"type": "object", "properties": {}},
            "_meta": {"codex_request_type": "approval_request",
                      "codex_approval_kind": "mcp_tool_call", "tool_name": "write_record"}}})
        continue
    elif method is None and request.get("id") == "server-approval":
        send({"id": pending_call, "result": {"content": [
            {"type": "text", "text": json.dumps(request.get("result"))}]}})
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupt_aborts_server_initiated_mcp_guardian_review() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(Ok(()), "the MCP fixture requires a host Python interpreter");

    let server = responses::start_mock_server().await;
    let mcp_servers = serde_json::from_value(json!({
        "elicitation": {
            "command": if cfg!(windows) { "python" } else { "python3" },
            "args": ["-u", "-c", ELICITATION_SERVER],
            "default_tools_approval_mode": "approve",
        }
    }))?;
    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
        config.approvals_reviewer = ApprovalsReviewer::AutoReview;
        config
            .mcp_servers
            .set(mcp_servers)
            .expect("set MCP fixture");
    });
    let test = builder.build_with_auto_env(&server).await?;
    wait_for_mcp_server(&test.codex, "elicitation").await?;
    responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_function_call_with_namespace(
                "eliciting-tool",
                "mcp__elicitation",
                "request_approval",
                "{}",
            ),
            responses::ev_completed("parent-tool"),
        ]),
    )
    .await;
    let pending_guardian = responses::mount_response_once_match(
        &server,
        |request: &wiremock::Request| {
            serde_json::from_slice::<Value>(&request.body)
                .expect("Responses request body should be valid JSON")
                .pointer("/client_metadata/x-openai-subagent")
                .and_then(Value::as_str)
                == Some("guardian")
        },
        responses::sse_response(responses::sse(vec![
            responses::ev_assistant_message(
                "review-result",
                &json!({
                    "risk_level": "low",
                    "user_authorization": "high",
                    "outcome": "allow",
                    "rationale": "This response must be interrupted.",
                })
                .to_string(),
            ),
            responses::ev_completed("pending-review"),
        ]))
        .set_delay(Duration::from_secs(60)),
    )
    .await;

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Run the tool that requests server-side approval.".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(local_selections(test.config.cwd.clone())),
                ..Default::default()
            }),
        )
        .await?;
    tokio::time::timeout(Duration::from_secs(10), async {
        while pending_guardian.requests().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .context("server-initiated Guardian review did not start")?;
    assert!(
        pending_guardian
            .single_request()
            .body_contains_text("write_record")
    );

    test.codex.submit(Op::Interrupt).await?;
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut guardian_aborted = false;
        let mut parent_aborted = false;
        while !guardian_aborted || !parent_aborted {
            match test.codex.next_event().await?.msg {
                EventMsg::GuardianAssessment(assessment)
                    if assessment.status == GuardianAssessmentStatus::Aborted =>
                {
                    guardian_aborted = true;
                }
                EventMsg::TurnAborted(_) => parent_aborted = true,
                _ => {}
            }
        }
        anyhow::Ok(())
    })
    .await
    .context("turn interruption did not abort the MCP elicitation review")??;
    test.codex.shutdown_and_wait().await?;
    Ok(())
}
