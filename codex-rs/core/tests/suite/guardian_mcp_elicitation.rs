use anyhow::Context;
use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_core::config::Constrained;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::openai_models::AutoReviewMessages;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::GuardianAssessmentStatus;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use test_case::test_case;
use wiremock::matchers::body_partial_json;

// The tool itself needs no approval. Its server requests a separate Guardian
// review after tools/call, exercising the ordinary MCP elicitation path.
const ELICITATION_SERVER: &str = r#"
import json
import sys

def send(message):
    print(json.dumps({"jsonrpc": "2.0", **message}), flush=True)

pending_call = None
approval_meta = json.loads(sys.argv[1]) if len(sys.argv) > 1 else {}
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
                      "codex_approval_kind": "mcp_tool_call", "tool_name": "write_record",
                      **approval_meta}}})
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

#[test_case(None, false; "ordinary_absent")]
#[test_case(Some(false), false; "ordinary_false")]
#[test_case(Some(true), false; "ordinary_true")]
#[test_case(None, true; "strict_absent")]
#[test_case(Some(false), true; "strict_false")]
#[test_case(Some(true), true; "strict_true")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn server_initiated_mcp_elicitation_can_require_synchronous_auto_review(
    sensitive_action: Option<bool>,
    strict_auto_review: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(Ok(()), "the MCP fixture requires a host Python interpreter");

    const RATIONALE: &str = "The server-side action is not authorized.";
    const REJECTION_INSTRUCTIONS: &str = "Do not retry the rejected action.";

    struct AutoApprovingReviewContributor;

    impl codex_extension_api::ApprovalReviewContributor for AutoApprovingReviewContributor {
        fn fast_decision<'a>(
            &'a self,
            _session_store: &'a codex_extension_api::ExtensionData,
            _thread_store: &'a codex_extension_api::ExtensionData,
            _prompt: &'a str,
            _extension_metrics: Option<Arc<dyn codex_extension_api::ExtensionMetrics>>,
        ) -> codex_extension_api::ExtensionFuture<'a, Option<ReviewDecision>> {
            Box::pin(async { Some(ReviewDecision::Approved) })
        }
    }

    let mut extensions = codex_extension_api::ExtensionRegistryBuilder::new();
    extensions.approval_review_contributor(Arc::new(AutoApprovingReviewContributor));
    let mut meta = serde_json::Map::new();
    if strict_auto_review {
        meta.insert("codex_strict_auto_review".to_string(), json!(true));
    }
    if let Some(sensitive_action) = sensitive_action {
        meta.insert(
            "codex_sensitive_action".to_string(),
            json!(sensitive_action),
        );
    }

    let server = responses::start_mock_server().await;
    let mcp_servers = serde_json::from_value(json!({
        "elicitation": {
            "command": if cfg!(windows) { "python" } else { "python3" },
            "args": ["-u", "-c", ELICITATION_SERVER, serde_json::to_string(&meta)?],
            "default_tools_approval_mode": "approve",
        }
    }))?;
    let mut builder = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_model_info_override("gpt-5.5", |model| {
            model
                .model_messages
                .as_mut()
                .expect("acting model messages")
                .auto_review = Some(AutoReviewMessages {
                policy: None,
                policy_template: None,
                node_repl_policy: None,
                rejection_instructions: Some(REJECTION_INSTRUCTIONS.to_string()),
                timeout_instructions: None,
            });
        })
        .with_config(move |config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = if strict_auto_review {
                ApprovalsReviewer::User
            } else {
                ApprovalsReviewer::AutoReview
            };
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
    let guardian = responses::mount_sse_once_match(
        &server,
        body_partial_json(json!({"client_metadata": {"x-openai-subagent": "guardian"}})),
        responses::sse(vec![
            responses::ev_assistant_message(
                "review-result",
                &json!({
                    "risk_level": "high",
                    "user_authorization": "low",
                    "outcome": "deny",
                    "rationale": RATIONALE,
                })
                .to_string(),
            ),
            responses::ev_completed("guardian-review"),
        ]),
    )
    .await;
    let parent = responses::mount_sse_once(
        &server,
        responses::sse(vec![responses::ev_completed("parent-complete")]),
    )
    .await;

    test.submit_text_turn("Run the tool that requests server-side approval.")
        .await?;

    let output = parent
        .single_request()
        .function_call_output("eliciting-tool");
    let response: Value = serde_json::from_str(
        output["output"][1]["text"]
            .as_str()
            .expect("MCP fixture should echo the wire response"),
    )?;
    let requires_sync = sensitive_action == Some(true);
    let expected_response = if requires_sync {
        json!({
            "action": "decline",
            "_meta": {
                "approvals_reviewer": "auto_review",
                "message": format!(
                    "This action was rejected due to unacceptable risk.\nReason: {RATIONALE}\n{REJECTION_INSTRUCTIONS}"
                ),
            },
        })
    } else {
        json!({
            "action": "accept",
            "content": {},
            "_meta": {"approvals_reviewer": "auto_review"},
        })
    };
    assert_eq!(response, expected_response);

    let guardian_requests = guardian
        .requests()
        .into_iter()
        .filter(|request| request.body_json()["client_metadata"]["x-openai-subagent"] == "guardian")
        .collect::<Vec<_>>();
    assert_eq!(guardian_requests.len(), usize::from(requires_sync));
    if requires_sync {
        assert!(guardian_requests[0].body_contains_text("write_record"));
    }
    test.codex.shutdown_and_wait().await?;
    Ok(())
}
