use anyhow::Context;
use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_core::config::Constrained;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::items::McpToolCallStatus;
use codex_protocol::items::TurnItem;
use codex_protocol::openai_models::AutoReviewMessages;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::GuardianAssessmentAction;
use codex_protocol::protocol::GuardianAssessmentStatus;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
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
multiple = isinstance(approval_meta, list)
approval_metas = approval_meta if multiple else [approval_meta]
pending_elicitations = []
approval_results = {}
for line in sys.stdin:
    request = json.loads(line)
    method = request.get("method")
    if method == "initialize":
        result = {"protocolVersion": request["params"]["protocolVersion"],
                  "capabilities": {"tools": {}},
                  "serverInfo": {"name": "guardian-elicitation-test", "version": "1"}}
    elif method == "tools/list":
        result = {"tools": [{"name": "js" if multiple else "request_approval",
                  "inputSchema": {"type": "object", "properties": {}},
                  "annotations": {"readOnlyHint": True}}]}
    elif method == "tools/call":
        if request["params"].get("arguments", {}).get("seed"):
            send({"id": request["id"], "result": {"content": [{"type": "text", "text": "seeded"}]}})
            continue
        pending_call = request["id"]
        approval_results = {}
        for index, meta in enumerate(approval_metas):
            invocation_meta = {}
            if len(sys.argv) > 2 and sys.argv[2] == "forward":
                invocation_meta["callId"] = request["params"]["_meta"]["callId"]
            pending_elicitations.append({
                "id": f"server-approval-{index}" if multiple else "server-approval",
                "method": "elicitation/create", "params": {
                    "message": "Approve the server-side action?",
                    "requestedSchema": {"type": "object", "properties": {}},
                    "_meta": {"codex_request_type": "approval_request",
                              "codex_approval_kind": "mcp_tool_call", "tool_name": "write_record",
                              **meta, **invocation_meta}}})
        if len(sys.argv) > 3:
            for elicitation in pending_elicitations:
                params = elicitation["params"]
                params.update(json.loads(sys.argv[3]))
                if params.get("mode") == "url":
                    params.pop("requestedSchema")
        send(pending_elicitations.pop(0))
        continue
    elif method is None and str(request.get("id", "")).startswith("server-approval"):
        approval_results[request["id"]] = request.get("result")
        if pending_elicitations:
            send(pending_elicitations.pop(0))
        else:
            result = approval_results if multiple else request.get("result")
            send({"id": pending_call, "result": {"content": [
                {"type": "text", "text": json.dumps(result)}]}})
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
        fn decide<'a>(
            &'a self,
            _input: &'a codex_extension_api::ApprovalDecisionInput<'_>,
        ) -> codex_extension_api::ExtensionFuture<'a, Option<codex_extension_api::ApprovalDecision>>
        {
            Box::pin(async { Some(codex_extension_api::ApprovalDecision::Allow) })
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum CallIdSource {
    Host,
    Missing,
    NonString,
    Unknown,
    WrongServer,
    PreviousTurn,
}

#[test_case("node_repl", false, CallIdSource::Host; "ordinary_node_repl")]
#[test_case("node_repl", true, CallIdSource::Host; "strict_node_repl")]
#[test_case("cua_repl", false, CallIdSource::Host; "ordinary_cua_repl")]
#[test_case("cua_repl", true, CallIdSource::Host; "strict_cua_repl")]
#[test_case("node_repl", false, CallIdSource::Missing; "missing_call_id")]
#[test_case("node_repl", true, CallIdSource::Missing; "strict_missing_call_id")]
#[test_case("node_repl", false, CallIdSource::NonString; "non_string_call_id")]
#[test_case("node_repl", false, CallIdSource::Unknown; "unknown_call_id")]
#[test_case("node_repl", false, CallIdSource::WrongServer; "wrong_server_call_id")]
#[test_case("node_repl", false, CallIdSource::PreviousTurn; "previous_turn_call_id")]
#[test_case("elicitation", false, CallIdSource::Host; "unrelated_server")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn node_elicitations_attribute_independent_reviews_without_changing_actions(
    server_name: &str,
    strict_auto_review: bool,
    call_id_source: CallIdSource,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(Ok(()), "the MCP fixture requires a host Python interpreter");

    let actions = [
        (
            "access_browser_origin",
            json!({"origin": "https://example.com"}),
        ),
        ("webmcp:write_record", json!({"record": {"value": 42}})),
    ];
    let meta = actions.map(|(tool_name, arguments)| {
        let mut meta = json!({
            "codex_strict_auto_review": strict_auto_review,
            "codex_sensitive_action": true,
            "tool_name": tool_name,
            "tool_params": arguments,
            "connector_id": "inner-connector",
            "connector_name": "Inner Connector",
            "connector_description": "Connector for the reviewed inner action",
            "tool_title": "Inner action",
            "tool_description": "Review this action independently from JavaScript".repeat(220),
        });
        match call_id_source {
            CallIdSource::Host | CallIdSource::Missing => {}
            CallIdSource::NonString => meta["callId"] = json!(42),
            CallIdSource::Unknown => meta["callId"] = json!("unknown-invocation"),
            CallIdSource::WrongServer | CallIdSource::PreviousTurn => {
                meta["callId"] = json!("seed-invocation");
            }
        }
        meta
    });
    let server = responses::start_mock_server().await;
    let fixture_config = json!({
        "command": if cfg!(windows) { "python" } else { "python3" },
        "args": ["-u", "-c", ELICITATION_SERVER, serde_json::to_string(&meta)?,
                 if call_id_source == CallIdSource::Host { "forward" } else { "omit" }],
        "default_tools_approval_mode": "approve",
    });
    let mut mcp_servers = json!({(server_name): fixture_config.clone()});
    if call_id_source == CallIdSource::WrongServer {
        mcp_servers["cua_repl"] = fixture_config;
    }
    let mcp_servers = serde_json::from_value(mcp_servers)?;
    let mut builder = test_codex().with_config(move |config| {
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
    wait_for_mcp_server(&test.codex, server_name).await?;
    if matches!(
        call_id_source,
        CallIdSource::WrongServer | CallIdSource::PreviousTurn
    ) {
        let seed_server = if call_id_source == CallIdSource::WrongServer {
            "cua_repl"
        } else {
            server_name
        };
        responses::mount_sse_once(
            &server,
            responses::sse(vec![
                responses::ev_function_call_with_namespace(
                    "seed-invocation",
                    &format!("mcp__{seed_server}"),
                    "js",
                    r#"{"seed":true}"#,
                ),
                responses::ev_completed("seed-call"),
            ]),
        )
        .await;
        if call_id_source == CallIdSource::PreviousTurn {
            responses::mount_sse_once(
                &server,
                responses::sse(vec![responses::ev_completed("seed-complete")]),
            )
            .await;
            test.submit_text_turn("Prepare the earlier invocation.")
                .await?;
        }
    }
    let parent_arguments = json!({"code": "await browser.doTwoActions()"});
    responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_function_call_with_namespace(
                "eliciting-tool",
                &format!("mcp__{server_name}"),
                "js",
                &parent_arguments.to_string(),
            ),
            responses::ev_completed("parent-tool"),
        ]),
    )
    .await;
    let mut guardian = Vec::new();
    for outcome in ["allow", "deny"] {
        guardian.push(
            responses::mount_sse_once_match(
                &server,
                body_partial_json(json!({"client_metadata": {"x-openai-subagent": "guardian"}})),
                responses::sse(vec![
                    responses::ev_assistant_message(
                        "review-result",
                        &json!({
                            "risk_level": "low", "user_authorization": "high",
                            "outcome": outcome, "rationale": "Independent inner action decision.",
                        })
                        .to_string(),
                    ),
                    responses::ev_completed(outcome),
                ]),
            )
            .await,
        );
    }
    let parent = responses::mount_sse_once(
        &server,
        responses::sse(vec![responses::ev_completed("parent-complete")]),
    )
    .await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Run the JavaScript invocation with two reviewed inner actions.".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let mut assessments = Vec::new();
    let mut tool_items = Vec::new();
    loop {
        let event = tokio::time::timeout(Duration::from_secs(30), test.codex.next_event())
            .await
            .context("timed out waiting for independent MCP Guardian reviews")??;
        match event.msg {
            EventMsg::GuardianAssessment(assessment)
                if assessment.status != GuardianAssessmentStatus::InProgress =>
            {
                assessments.push(assessment)
            }
            EventMsg::ItemCompleted(event) => {
                if let TurnItem::McpToolCall(item) = event.item
                    && item.id == "eliciting-tool"
                {
                    tool_items.push(item);
                }
            }
            EventMsg::ElicitationRequest(_) => panic!("Guardian approval must not prompt the user"),
            EventMsg::TurnComplete(_) => break,
            _ => {}
        }
    }
    assert_eq!(assessments.len(), 2);
    assert_ne!(assessments[0].id, assessments[1].id);
    let attributed = call_id_source == CallIdSource::Host && server_name != "elicitation";
    for (index, (assessment, status)) in assessments
        .iter()
        .zip([
            GuardianAssessmentStatus::Approved,
            GuardianAssessmentStatus::Denied,
        ])
        .enumerate()
    {
        let target = if attributed {
            "eliciting-tool".to_string()
        } else {
            format!("mcp_elicitation:{server_name}:server-approval-{index}")
        };
        assert_eq!(
            (&assessment.target_item_id, &assessment.status),
            (&Some(target), &status)
        );
        assert_eq!(
            assessment.action,
            GuardianAssessmentAction::McpToolCall {
                server: server_name.to_string(),
                tool_name: meta[index]["tool_name"]
                    .as_str()
                    .expect("inner tool name")
                    .to_string(),
                connector_id: Some("inner-connector".to_string()),
                connector_name: Some("Inner Connector".to_string()),
                tool_title: Some("Inner action".to_string()),
            }
        );
        let request = guardian[index].single_request();
        let prompt = request.message_input_texts("user").join("\n");
        let action_text = prompt
            .rsplit_once("Planned action JSON:\n")
            .context("Guardian planned action")?
            .1
            .split_once("\n>>> APPROVAL REQUEST END")
            .context("Guardian action end")?
            .0;
        assert_eq!(
            serde_json::from_str::<Value>(action_text)?,
            json!({
                "tool": "mcp_tool_call", "server": server_name,
                "tool_name": meta[index]["tool_name"], "arguments": meta[index]["tool_params"],
                "connector_id": "inner-connector", "connector_name": "Inner Connector",
                "connector_description": "Connector for the reviewed inner action",
                "tool_title": "Inner action",
                "tool_description": "Review this action independently from JavaScript".repeat(220),
            })
        );
    }
    let [tool_item] = tool_items.as_slice() else {
        panic!("expected one completed enclosing tool item: {tool_items:?}");
    };
    assert_eq!(
        (
            tool_item.server.as_str(),
            tool_item.tool.as_str(),
            &tool_item.arguments,
            tool_item.status
        ),
        (
            server_name,
            "js",
            &parent_arguments,
            McpToolCallStatus::Completed
        )
    );
    let output = parent
        .single_request()
        .function_call_output("eliciting-tool");
    let replies: Value = serde_json::from_str(
        output["output"][1]["text"]
            .as_str()
            .context("MCP fixture responses")?,
    )?;
    assert_eq!(replies.as_object().map(serde_json::Map::len), Some(2));
    assert_eq!(
        replies["server-approval-0"],
        json!({
            "action": "accept", "content": {}, "_meta": {"approvals_reviewer": "auto_review"},
        })
    );
    assert_eq!(replies["server-approval-1"]["action"], "decline");
    assert_eq!(
        replies["server-approval-1"]["_meta"]["approvals_reviewer"],
        "auto_review"
    );
    assert!(
        replies["server-approval-1"]["_meta"]["message"]
            .as_str()
            .expect("Guardian rejection reason")
            .contains("Independent inner action decision.")
    );
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

#[test_case("form"; "nonempty_form")]
#[test_case("url"; "url")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_review_preserves_unsupported_guardian_elicitations(mode: &str) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(Ok(()), "the MCP fixture requires a host Python interpreter");
    let server = responses::start_mock_server().await;
    let form_schema = json!({
        "type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]
    });
    let params = if mode == "url" {
        json!({"mode": "url", "url": "https://example.com/approve", "elicitationId": "approval"})
    } else {
        json!({"requestedSchema": form_schema})
    };
    let mcp_servers = serde_json::from_value(json!({
        "elicitation": {
            "command": if cfg!(windows) { "python" } else { "python3" },
            "args": ["-u", "-c", ELICITATION_SERVER, "{}", "", params.to_string()],
            "default_tools_approval_mode": "approve",
        }
    }))?;
    let test = test_codex()
        .with_config(move |config| {
            config.approvals_reviewer = ApprovalsReviewer::User;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::Granular(
                codex_protocol::protocol::GranularApprovalConfig {
                    sandbox_approval: false,
                    rules: false,
                    skill_approval: false,
                    request_permissions: false,
                    mcp_elicitations: true,
                },
            ));
            config
                .mcp_servers
                .set(mcp_servers)
                .expect("set MCP fixture");
        })
        .build_with_auto_env(&server)
        .await?;
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
    let parent = responses::mount_sse_once(
        &server,
        responses::sse(vec![responses::ev_completed("parent-complete")]),
    )
    .await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Run the tool and ask me for its approval.".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ElicitationRequest(_)
                | EventMsg::GuardianAssessment(_)
                | EventMsg::TurnComplete(_)
        )
    })
    .await;
    let EventMsg::ElicitationRequest(request) = event else {
        panic!("expected a user elicitation, got {event:?}");
    };
    match &request.request {
        codex_protocol::approvals::ElicitationRequest::Form {
            requested_schema, ..
        } => {
            assert_eq!(requested_schema, &form_schema);
        }
        codex_protocol::approvals::ElicitationRequest::Url { url, .. } => {
            assert_eq!(url, "https://example.com/approve");
        }
        other => panic!("unexpected elicitation: {other:?}"),
    }
    test.codex
        .submit(Op::ResolveElicitation {
            server_name: request.server_name,
            request_id: request.id,
            decision: codex_protocol::approvals::ElicitationAction::Decline,
            content: None,
            meta: None,
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert!(
        parent
            .single_request()
            .function_call_output("eliciting-tool")
            .to_string()
            .contains("decline")
    );
    test.codex.shutdown_and_wait().await?;
    Ok(())
}
