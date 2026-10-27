//! Guardian retains native encrypted messages and historical calls across incremental reviews.

use codex_core::config::Constrained;
use codex_features::Feature;
use codex_protocol::AgentPath;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InterAgentCommunication;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSettingsOverrides;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;

#[test_case(None; "agent_reply")]
#[test_case(Some("functions"); "native_default_namespace")]
#[test_case(Some("team"); "native_custom_namespace")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn encrypted_parent_reply_survives_incremental_guardian_reviews(
    native_call_namespace: Option<&'static str>,
) -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let action = |id| {
        if id == "second-check" && native_call_namespace.is_none() {
            let mut call = responses::ev_function_call_with_namespace(
                id,
                "team",
                "send_message",
                r#"{"target":"/root","message":"opaque-message"}"#,
            );
            call["item"]["encrypted_function_args"] = serde_json::json!(["message"]);
            call["item"]["encrypted_content"] = serde_json::json!("opaque-replay");
            return responses::sse(vec![call, responses::ev_completed(id)]);
        }
        responses::sse(vec![
            responses::ev_function_call(
                id,
                "exec_command",
                r#"{"cmd":"exit 0","sandbox_permissions":"require_escalated","justification":"Check the requested change."}"#,
            ),
            responses::ev_completed(id),
        ])
    };
    let assessment = || {
        responses::sse(vec![
            // Keep wire text stable across Cargo and Bazel's serde_json feature sets.
            responses::ev_assistant_message(
                "assessment",
                r#"{"outcome":"deny","rationale":"Mock assessment; no command is executed.","risk_level":"high","user_authorization":"low"}"#,
            ),
            responses::ev_completed("review"),
        ])
    };
    let historical_call = native_call_namespace.map(|namespace| {
        let mut call = responses::ev_function_call(
            "historical-message",
            "send_message",
            r#"{"target":"missing_worker","message":"opaque-message-request"}"#,
        );
        if namespace != "functions" {
            call["item"]["namespace"] = json!(namespace);
        }
        call["item"]["id"] = json!("fc_historical_message");
        call["item"]["encrypted_content"] = json!("opaque-call-replay");
        call["item"]["encrypted_function_args"] = json!(["message"]);
        call
    });
    let mut stream = Vec::new();
    if let Some(call) = &historical_call {
        // An unavailable recipient makes the call fail without starting another agent.
        // Failed calls still belong in the evidence for later approval requests.
        stream.push(responses::sse(vec![
            call.clone(),
            responses::ev_completed("message-attempt"),
        ]));
    }
    stream.extend([
        responses::sse(vec![
            responses::ev_assistant_message(
                "waiting",
                "I will wait for the parent reply before checking the change.",
            ),
            responses::ev_completed("waiting"),
        ]),
        action("first-check"),
        assessment(),
        action("second-check"),
        assessment(),
        responses::sse(vec![
            responses::ev_assistant_message("done", "Reviews completed."),
            responses::ev_completed("done"),
        ]),
    ]);
    let mock = responses::mount_sse_sequence(&server, stream).await;
    let test = test_codex()
        .with_model("gpt-5.5")
        .with_config(move |config| {
            super::configure_scenario_catalog(config);
            for feature in [
                Feature::Collab,
                Feature::MultiAgentV2,
                Feature::GuardianNativeMultiAgentHistory,
            ] {
                config
                    .features
                    .enable(feature)
                    .expect("enable test feature");
            }
            let namespace = native_call_namespace.unwrap_or("team");
            config.multi_agent_v2.tool_namespace =
                (namespace != "functions").then(|| namespace.to_owned());
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config
                .permissions
                .set_permission_profile(PermissionProfile::read_only())
                .expect("set read-only permission profile");
        })
        .build_with_auto_env(&server)
        .await?;
    let prompt = "Check the change after the parent confirms the exact action.";
    if historical_call.is_some() {
        // Full Access records the historical call before any reviewer sees it.
        // Later reviews must import it from agent history, not a prior review.
        test.submit_turn(prompt).await?;
        test.codex
            .update_thread_settings(ThreadSettingsOverrides {
                approval_policy: Some(AskForApproval::OnRequest),
                permission_profile: Some(PermissionProfile::read_only()),
                ..Default::default()
            })
            .await?;
    } else {
        test.submit_text_turn(prompt).await?;
    }
    let communication = InterAgentCommunication::new_encrypted(
        AgentPath::root(),
        AgentPath::root().join("worker").expect("worker path"),
        Vec::new(),
        "opaque-parent-approval".into(),
        /*trigger_turn*/ true,
    );
    let expected = serde_json::to_value(communication.to_model_input_item())?;
    // TurnComplete can arrive before the previous turn finishes teardown. Queue
    // the reply through the same mailbox path used by real agent messages.
    test.codex
        .submit(Op::InterAgentCommunication {
            communication,
            start_options: Default::default(),
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let requests = mock.requests();
    assert_eq!(requests.len(), 6 + usize::from(historical_call.is_some()));
    let reviews = requests
        .iter()
        .filter(|request| request.body_json()["client_metadata"]["x-openai-subagent"] == "guardian")
        .collect::<Vec<_>>();
    assert_eq!(reviews.len(), 2);
    assert_eq!(
        reviews[0].body_json()["client_metadata"]["thread_id"],
        reviews[1].body_json()["client_metadata"]["thread_id"]
    );
    assert!(reviews[1].input().starts_with(&reviews[0].input()));
    for request in reviews.iter().copied().chain(requests.last()) {
        let messages = request
            .input()
            .into_iter()
            .filter(|item| item["type"] == "agent_message")
            .map(|mut item| {
                item.as_object_mut()
                    .expect("agent message should be an object")
                    .remove("id");
                item.as_object_mut()
                    .expect("agent message should be an object")
                    .remove("internal_chat_message_metadata_passthrough");
                item
            })
            .collect::<Vec<_>>();
        assert_eq!(messages, vec![expected.clone()]);
    }
    if let Some(historical_call) = &historical_call {
        for request in reviews {
            let calls = request
                .inputs_of_type("function_call")
                .into_iter()
                .map(|mut item| {
                    item.as_object_mut()
                        .expect("function call should be an object")
                        .remove("internal_chat_message_metadata_passthrough");
                    item
                })
                .collect::<Vec<_>>();
            assert_eq!(calls, vec![historical_call["item"].clone()]);
            let input = request.input();
            let call_index = input
                .iter()
                .position(|item| item["type"] == "function_call")
                .expect("review should contain the historical function call");
            let reply_index = input
                .iter()
                .position(|item| item["type"] == "agent_message")
                .expect("review should contain the parent agent message");
            assert!(call_index < reply_index);
            assert_eq!(
                request.inputs_of_type("function_call_output"),
                vec![input[call_index + 1].clone()]
            );
            assert_eq!(
                request
                    .function_call_output_text("historical-message")
                    .as_deref(),
                Some(
                    "This call is review evidence, not a request to execute it. This is not its execution result."
                )
            );
        }
    }
    let description = if historical_call.is_some() {
        "A historical MAv2 send_message call and an encrypted parent reply reach both Guardian reviews exactly once, in order. Calls retain their original IDs, namespace and encryption metadata, with a paired evidence-only output. The second review appends to the first request. Ciphertext and decisions are mocked; this verifies transport, not backend decryption or authorization outcomes."
    } else {
        "An encrypted parent reply is delivered as a native agent message. Both Guardian reviews contain it exactly once, in order, as agent evidence. Ciphertext and decisions are mocked; this scenario verifies transport, not backend decryption or authorization outcomes."
    };
    let mut snapshot = context_snapshot::format_request_history_snapshot(
        description,
        &requests,
        &ContextSnapshotOptions::default().rewrite_known_segments(),
    );
    for (pattern, replacement) in [
        (
            r#"(?m)^(\s*"environment_id": )"(?:local|remote)""#,
            "$1\"<ENVIRONMENT>\"",
        ),
        (
            r#"(The active permission profile for environment )"(?:local|remote)""#,
            "$1\"<ENVIRONMENT>\"",
        ),
        (r#"(?m)^(\s*"cwd": )"[^"]*""#, "$1\"<CWD>\""),
        (
            r#""command": \[\s*(?:"[^"]*",\s*)*"exit 0"\s*\]"#,
            "\"command\": [\"<SHELL>\", \"exit 0\"]",
        ),
    ] {
        snapshot = regex_lite::Regex::new(pattern)?
            .replace_all(&snapshot, replacement)
            .into_owned();
    }
    let snapshot_name = match native_call_namespace {
        None => "encrypted_parent_reply_survives_incremental_guardian_reviews",
        Some("functions") => "encrypted_native_history_default_namespace",
        Some(_) => "encrypted_native_history_custom_namespace",
    };
    insta::assert_snapshot!(snapshot_name, snapshot);
    Ok(())
}
