//! Parent-backed history retrieval and live authorization on a reused reviewer.

use std::sync::Arc;
use std::sync::Mutex;

use codex_config::test_support::CloudConfigBundleFixture;
use codex_core::config::Constrained;
use codex_features::Feature;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::apps_enabled_builder;
use core_test_support::apps_test_server::recorded_apps_tool_calls;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use test_case::test_case;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::method;

const CUSTOM_HISTORY_PROMPT: &str =
    "Eval history policy: retrieve the owner's earlier instructions before approving.";

#[derive(Clone, Copy)]
enum HistoryScenario {
    BoundedHistory,
    SmallerReviewerBudget,
    PermissionChanges,
}

#[test_case(HistoryScenario::BoundedHistory; "parent_connection_identity_and_output_limit")]
#[test_case(HistoryScenario::SmallerReviewerBudget; "preserves_smaller_reviewer_output_budget")]
#[test_case(HistoryScenario::PermissionChanges; "reused_reviewer_honors_permission_changes")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_conversation_history(scenario: HistoryScenario) -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let history_server = MockServer::start().await;
    let large = !matches!(scenario, HistoryScenario::PermissionChanges);
    let smaller_reviewer_budget = matches!(scenario, HistoryScenario::SmallerReviewerBudget);
    let actions = [
        ("search_messages", json!({"query": "workspace"})),
        ("read_messages", json!({"message_id": "scope"})),
    ];
    let tools = actions.iter().map(|(name, _)| json!({
        "name": format!("user_message.{name}"), "description": format!("History {name}"),
        "inputSchema": {"type": "object", "properties": {"query": {"type": "string"}, "message_id": {"type": "string"}}},
        "annotations": {"readOnlyHint": true},
        "_meta": {"connector_id": "history", "connector_name": "user_message", "_codex_apps": {"connector_id": "history"}}
    })).collect();
    let apps =
        AppsTestServer::mount_with_tools(&history_server, Arc::new(Mutex::new(tools))).await?;
    let read_text = if large {
        "Earlier user instructions. ".repeat(10_000)
    } else {
        "Earlier user instructions.".to_owned()
    };
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method": "tools/call"})))
        .respond_with(move |request: &Request| {
            let body: Value = serde_json::from_slice(&request.body).expect("history call JSON");
            let text = if body["params"]["name"] == "user_message.search_messages" {
                "history-search-evidence"
            } else {
                &read_text
            };
            ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0", "id": body["id"],
                "result": {"content": [{"type": "text", "text": text}], "isError": false}
            }))
        })
        .with_priority(/*priority*/ 1)
        .mount(&history_server)
        .await;
    let mock = responses::mount_sse_sequence(&server, review_responses("initial", &actions)).await;
    let test = apps_enabled_builder(apps.chatgpt_base_url)
        .with_cloud_config_bundle(
            CloudConfigBundleFixture::loader_with_enterprise_requirement(
                "[features]\napps = true\nnon_prefixed_mcp_tool_names = true\n",
            ),
        )
        .with_model("gpt-5.5")
        .with_pre_build_hook(move |home| {
            if large {
                std::fs::write(
                    home.join("config.toml"),
                    format!("[auto_review]\nexperimental_conversation_history_prompt = {CUSTOM_HISTORY_PROMPT:?}\nconversation_history_max_output_tokens = 800\n"),
                ).expect("write history prompt config");
            }
        })
        .with_config(move |config| {
            super::configure_scenario_catalog(config);
            if smaller_reviewer_budget {
                config.tool_output_token_limit = Some(500);
            }
            config
                .features
                .enable(Feature::GuardianConversationHistoryTools)
                .expect("history tools");
            config.workspace_roots = vec![config.cwd.clone()];
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config
                .permissions
                .set_permission_profile(PermissionProfile::read_only())
                .expect("read-only profile");
        })
        .build_with_auto_env(&server)
        .await?;
    test.submit_text_turn("Check the workspace.").await?;

    let requests = mock.requests();
    let reviewers = requests
        .iter()
        .filter(|request| request.body_json()["client_metadata"]["x-openai-subagent"] == "guardian")
        .collect::<Vec<_>>();
    let body = reviewers[0].body_json();
    let tool_body = body["input"]
        .as_array()
        .and_then(|input| input.iter().find(|item| item["type"] == "additional_tools"))
        .unwrap_or(&body);
    for (name, _) in &actions {
        assert!(
            responses::namespace_child_tool(tool_body, "user_message", name).is_some(),
            "Guardian must advertise user_message.{name}"
        );
    }
    let history_prompt = if large {
        CUSTOM_HISTORY_PROMPT
    } else {
        "## Conversation history retrieval"
    };
    assert!(body.to_string().contains(history_prompt));
    let calls = recorded_apps_tool_calls(&history_server).await;
    let root = test.codex.startup_metadata().thread_id.to_string();
    assert_eq!(
        calls.iter().map(|call| json!({
            "name": call["params"]["name"], "arguments": call["params"]["arguments"],
            "threadId": call["params"]["_meta"]["threadId"], "sessionId": call["params"]["_meta"]["sessionId"]
        })).collect::<Vec<_>>(),
        actions.iter().map(|(name, arguments)| json!({
            "name": format!("user_message.{name}"), "arguments": arguments, "threadId": root, "sessionId": root
        })).collect::<Vec<_>>()
    );
    assert_eq!(
        history_server
            .received_requests()
            .await
            .expect("MCP requests")
            .iter()
            .filter_map(|request| serde_json::from_slice::<Value>(&request.body).ok())
            .filter(|body| body["method"] == "initialize")
            .count(),
        1,
        "one parent connection"
    );
    let evidence = reviewers.last().expect("reviewer evidence");
    assert!(
        evidence
            .function_call_output("initial-search_messages")
            .to_string()
            .contains("history-search-evidence")
    );
    if large {
        let output = evidence
            .function_call_output("initial-read_messages")
            .to_string();
        let max_output_bytes = if smaller_reviewer_budget {
            3_000
        } else {
            5_000
        };
        assert!(
            output.contains("truncated") && output.len() < max_output_bytes,
            "{output}"
        );
    } else {
        for (label, policy, expected_error) in [
            (
                "disabled",
                "[apps.history]\nenabled = false\n",
                "blocked by app configuration",
            ),
            (
                "prompt",
                "[apps.history.tools.\"user_message.read_messages\"]\napproval_mode = \"prompt\"\n",
                "requires approval on the parent",
            ),
        ] {
            let current_config = test.codex.config().await;
            let mut config = test.config.clone();
            config.config_layer_stack = config.config_layer_stack.with_user_config(
                &config.codex_home.join("config.toml"),
                toml::from_str(policy)?,
            )?;
            assert_eq!(
                test.codex
                    .refresh_runtime_config(current_config, config)
                    .await,
                codex_core::ConfigRefreshOutcome::Published
            );
            let followup =
                responses::mount_sse_sequence(&server, review_responses(label, &actions[1..]))
                    .await;
            test.submit_text_turn("Check again.").await?;
            let requests = followup.requests();
            assert_eq!(
                requests[1].body_json()["client_metadata"]["thread_id"],
                body["client_metadata"]["thread_id"],
                "reuse the reviewer"
            );
            assert!(
                requests[2]
                    .function_call_output(&format!("{label}-read_messages"))
                    .to_string()
                    .contains(expected_error)
            );
            assert_eq!(
                recorded_apps_tool_calls(&history_server).await,
                calls,
                "blocked calls must not reach the service"
            );
        }
    }
    Ok(())
}

fn review_responses(label: &str, actions: &[(&str, Value)]) -> Vec<String> {
    let mut replies = vec![responses::sse(vec![
        responses::ev_function_call(
            &format!("{label}-command"),
            "exec_command",
            r#"{"cmd":"exit 0","sandbox_permissions":"require_escalated"}"#,
        ),
        responses::ev_completed(&format!("{label}-action")),
    ])];
    replies.extend(actions.iter().map(|(name, arguments)| {
        responses::sse(vec![
            responses::ev_function_call_with_namespace(
                &format!("{label}-{name}"),
                "user_message",
                name,
                &arguments.to_string(),
            ),
            responses::ev_completed(&format!("{label}-{name}")),
        ])
    }));
    replies.extend([
        responses::sse(vec![responses::ev_assistant_message(&format!("{label}-decision"), r#"{"risk_level":"low","user_authorization":"low","outcome":"deny","rationale":"Fixture review complete."}"#), responses::ev_completed(&format!("{label}-review"))]),
        responses::sse(vec![responses::ev_completed(&format!("{label}-done"))]),
    ]);
    replies
}
