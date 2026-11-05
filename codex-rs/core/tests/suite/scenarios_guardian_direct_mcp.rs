//! Direct MCP review uses the current catalog and policy without dispatching the recorded call.

use super::guardian_direct_review::model_input;
use anyhow::Result;
use codex_config::types::AppToolApproval;
use codex_core::config::Constrained;
use codex_core::guardian_review::GuardianReviewResult;
use codex_features::Feature;
use codex_history::RolloutItem;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::input_text;
use codex_protocol::protocol::AskForApproval;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::SEARCH_CALENDAR_CREATE_TOOL;
use core_test_support::apps_test_server::SEARCH_CALENDAR_NAMESPACE;
use core_test_support::apps_test_server::recorded_apps_tool_calls;
use core_test_support::apps_test_server::search_capable_apps_builder;
use core_test_support::apps_test_server::set_default_app_approval_mode_and_reviewer;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tokio_util::sync::CancellationToken;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn direct_mcp_review_without_dispatch() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let apps = AppsTestServer::mount(&server).await?;
    let mut builder = search_capable_apps_builder(apps.chatgpt_base_url)
        .with_model("gpt-5.5")
        .with_config(|config| {
            super::configure_scenario_catalog(config);
            // App policy routes the live call to Guardian despite the thread reviewer setting.
            config.approvals_reviewer = ApprovalsReviewer::User;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config
                .features
                .enable(Feature::ToolCallMcpElicitation)
                .expect("enable MCP approvals");
            set_default_app_approval_mode_and_reviewer(
                config,
                AppToolApproval::Prompt,
                ApprovalsReviewer::AutoReview,
            );
        });
    let test = builder.build_with_auto_env(&server).await?;
    wait_for_mcp_server(&test.codex, "codex_apps").await?;
    let assessment = json!({
        "risk_level": "low",
        "user_authorization": "high",
        "outcome": "allow",
        "rationale": "The calendar event is authorized."
    });
    let responses = vec![
        sse(vec![
            ev_function_call_with_namespace(
                "mcp-review",
                SEARCH_CALENDAR_NAMESPACE,
                SEARCH_CALENDAR_CREATE_TOOL,
                r#"{"title":"Lunch","starts_at":"2026-03-10T12:00:00Z"}"#,
            ),
            ev_completed("parent-call"),
        ]),
        sse(vec![
            ev_assistant_message("live-denial", r#"{"outcome":"deny"}"#),
            ev_completed("live-review"),
        ]),
        sse(vec![
            ev_assistant_message("parent-done", "Denied."),
            ev_completed("parent-done"),
        ]),
        sse(vec![
            ev_assistant_message("direct-allow", &assessment.to_string()),
            ev_completed("direct-review"),
        ]),
    ];
    let mock = responses::mount_sse_sequence(&server, responses).await;

    // The live denial captures real Guardian input without calling the MCP tool.
    let prompt = "Use [$calendar](app://calendar) to create a lunch event.";
    test.submit_text_turn(prompt).await?;
    assert_eq!(recorded_apps_tool_calls(&server).await, Vec::<Value>::new());
    test.codex.flush_rollout().await?;
    let history = test.codex.load_history(/*include_archived*/ false).await?;
    let items = history
        .items
        .into_iter()
        .filter_map(|entry| match entry {
            RolloutItem::ResponseItem(item) => match &item.item {
                ResponseItem::Message { role, content, .. }
                    if role == "user" && content == &[input_text(prompt)] =>
                {
                    Some(item.into_item())
                }
                ResponseItem::FunctionCall { .. } => Some(item.into_item()),
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(items.len(), 2);

    // Review the same recorded call under each current policy, using one live baseline.
    for (current_approval_mode, expected_assessment) in [
        (
            AppToolApproval::Prompt,
            Some(serde_json::from_value(assessment)?),
        ),
        (AppToolApproval::Approve, None),
    ] {
        let mut options = test.start_thread_options().await;
        set_default_app_approval_mode_and_reviewer(
            &mut options.config,
            current_approval_mode,
            ApprovalsReviewer::AutoReview,
        );
        let direct = test.thread_manager.start_thread(options).await?;
        wait_for_mcp_server(&direct.thread, "codex_apps").await?;
        direct.thread.inject_response_items(items.clone()).await?;
        let result = direct
            .thread
            .guardian_review(&CancellationToken::new())
            .await?;
        assert_eq!(
            result,
            GuardianReviewResult {
                call_id: "mcp-review".to_owned(),
                assessment: expected_assessment,
            }
        );
        // The approve policy adds no Guardian request; both phases leave exactly four.
        let requests = mock.requests();
        let [_, live, _, recorded] = requests.as_slice() else {
            panic!("expected parent, live Guardian, parent, direct Guardian");
        };
        if result.assessment.is_some() {
            assert_eq!(
                model_input(recorded, direct.thread_id)?,
                model_input(live, history.thread_id)?,
            );
        }
        assert_eq!(recorded_apps_tool_calls(&server).await, Vec::<Value>::new());
        direct.thread.shutdown_and_wait().await?;
    }
    let snapshot = context_snapshot::format_request_history_snapshot(
        "A live MCP denial and a direct approval render the same action; neither dispatches the tool.",
        &mock.requests(),
        &ContextSnapshotOptions::default().rewrite_known_segments(),
    );
    let environment_id = &test.executor_environment().selection().environment_id;
    let snapshot = snapshot.replace(
        &format!("environment {environment_id:?}"),
        "environment \"<ENVIRONMENT>\"",
    );
    insta::assert_snapshot!("guardian_direct_mcp", snapshot);
    test.codex.shutdown_and_wait().await?;
    Ok(())
}
