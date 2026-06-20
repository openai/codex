use anyhow::Result;
use codex_core::config::TokenBudgetConfig;
use codex_features::Feature;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_completed_with_tokens;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

const CONFIGURED_CONTEXT_WINDOW: i64 = 128_000;

fn token_budget_texts(request: &ResponsesRequest) -> Vec<String> {
    request
        .message_input_texts("developer")
        .into_iter()
        .filter(|text| text.starts_with("<token_budget>"))
        .collect()
}

fn tool_names(request: &ResponsesRequest) -> Vec<String> {
    request
        .body_json()
        .get("tools")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(str::to_string))
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_budget_messages_emit_on_usage_and_compaction_thresholds() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_completed_with_tokens("resp-1", /*total_tokens*/ 2_500),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_completed_with_tokens("resp-2", /*total_tokens*/ 3_000),
            ]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_completed_with_tokens("resp-3", /*total_tokens*/ 5_000),
            ]),
            sse(vec![
                ev_response_created("resp-4"),
                ev_completed_with_tokens("resp-4", /*total_tokens*/ 8_000),
            ]),
            sse(vec![ev_response_created("resp-5"), ev_completed("resp-5")]),
        ],
    )
    .await;
    let test = test_codex()
        .with_config(|config| {
            config.model_context_window = Some(10_000);
            config.token_budget = Some(TokenBudgetConfig {
                reminder_threshold_tokens: Some(2_000),
                ..TokenBudgetConfig::default()
            });
            config
                .features
                .enable(Feature::TokenBudget)
                .expect("test config should allow token budget");
        })
        .build(&server)
        .await?;

    for turn in 1..=5 {
        test.submit_turn(&format!("turn {turn}")).await?;
    }

    let requests = responses.requests();
    assert_eq!(requests.len(), 5);

    let threshold_25 =
        "<token_budget>\nYou have 7000 tokens left in this context window.\n</token_budget>"
            .to_string();
    let threshold_50 =
        "<token_budget>\nYou have 4500 tokens left in this context window.\n</token_budget>"
            .to_string();
    let threshold_75 =
        "<token_budget>\nYou have 1500 tokens left in this context window.\n</token_budget>"
            .to_string();
    let wrap_up_reminder = "<token_budget>\nYour context window is nearly exhausted (only 1000 tokens remaining) and will be automatically reset for you soon. Once reset, message items in current context window will be cleared in the new window, but notes and history items will be persistent across windows.\n</token_budget>"
        .to_string();

    assert_eq!(token_budget_texts(&requests[1]), vec![threshold_25.clone()]);
    assert_eq!(token_budget_texts(&requests[2]), vec![threshold_25.clone()]);
    assert_eq!(
        token_budget_texts(&requests[3]),
        vec![threshold_25.clone(), threshold_50.clone()]
    );
    assert_eq!(
        token_budget_texts(&requests[4]),
        vec![threshold_25, threshold_50, threshold_75, wrap_up_reminder]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_budget_reminder_is_level_triggered_once_per_window() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_completed_with_tokens("resp-1", /*total_tokens*/ 1_000),
            ]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        ],
    )
    .await;
    let test = test_codex()
        .with_config(|config| {
            config.model_context_window = Some(10_000);
            config.token_budget = Some(TokenBudgetConfig {
                reminder_threshold_tokens: Some(10_000),
                reminder_message_template: "Custom reminder.".to_string(),
            });
            config
                .features
                .enable(Feature::TokenBudget)
                .expect("test config should allow token budget");
        })
        .build(&server)
        .await?;

    test.submit_turn("first turn").await?;
    test.submit_turn("second turn").await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let reminder = "<token_budget>\nCustom reminder.\n</token_budget>".to_string();
    assert_eq!(token_budget_texts(&requests[0]), vec![reminder.clone()]);
    assert_eq!(token_budget_texts(&requests[1]), vec![reminder]);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_context_remaining_returns_token_budget_remaining_fragment() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "remaining-call";
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg-1", "noted"),
                ev_completed_with_tokens("resp-1", /*total_tokens*/ 2_500),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_function_call(call_id, "get_context_remaining", "{}"),
                ev_completed_with_tokens("resp-2", /*total_tokens*/ 2_500),
            ]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_assistant_message("msg-3", "done"),
                ev_completed("resp-3"),
            ]),
        ],
    )
    .await;
    let test = test_codex()
        .with_config(|config| {
            config.model_context_window = Some(10_000);
            config
                .features
                .enable(Feature::TokenBudget)
                .expect("test config should allow token budget");
        })
        .build(&server)
        .await?;

    test.submit_turn("spend some tokens").await?;
    test.submit_turn("check remaining context").await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 3);
    assert!(
        tool_names(&requests[1])
            .iter()
            .any(|name| name == "get_context_remaining"),
        "get_context_remaining should be exposed when token budget is enabled"
    );

    let remaining_context =
        "<token_budget>\nYou have 7000 tokens left in this context window.\n</token_budget>"
            .to_string();
    let token_budgets = token_budget_texts(&requests[1]);
    assert_eq!(token_budgets, vec![remaining_context.clone()]);
    assert_eq!(
        requests[2].function_call_output_content_and_success(call_id),
        Some((Some(remaining_context), None))
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_context_remaining_returns_unknown_when_window_is_unavailable() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "remaining-call";
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, "get_context_remaining", "{}"),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    let test = test_codex()
        .with_model_info_override("gpt-5.2", |model_info| {
            model_info.context_window = None;
            model_info.max_context_window = None;
        })
        .with_config(|config| {
            config.model_context_window = None;
            config
                .features
                .enable(Feature::TokenBudget)
                .expect("test config should allow token budget");
        })
        .build(&server)
        .await?;

    test.submit_turn("check remaining context").await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    assert!(
        tool_names(&requests[0])
            .iter()
            .any(|name| name == "get_context_remaining"),
        "get_context_remaining should be exposed when token budget is enabled"
    );

    assert_eq!(
        requests[1].function_call_output_content_and_success(call_id),
        Some((
            Some(
                "<token_budget>\nYou have unknown tokens left in this context window.\n</token_budget>"
                    .to_string()
            ),
            None,
        ))
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn new_context_tool_starts_new_window_before_follow_up() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "new-window-call";
    let continue_call_id = "continue-call";
    let continue_args = json!({
        "plan": [
            {"step": "Continue in the new context window", "status": "in_progress"}
        ],
    })
    .to_string();
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, "new_context", "{}"),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_function_call(continue_call_id, "update_plan", &continue_args),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_assistant_message("msg-3", "done"),
                ev_completed("resp-3"),
            ]),
        ],
    )
    .await;
    let test = test_codex()
        .with_config(|config| {
            config.model_context_window = Some(CONFIGURED_CONTEXT_WINDOW);
            config
                .features
                .enable(Feature::TokenBudget)
                .expect("test config should allow token budget");
        })
        .build(&server)
        .await?;

    test.submit_turn("request new context window").await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 3);
    assert!(
        tool_names(&requests[0])
            .iter()
            .any(|name| name == "new_context"),
        "new_context should be exposed when token budget is enabled"
    );
    assert!(
        !requests[2].body_contains_text("request new context window"),
        "new_context should drop the prior window history before continuing the turn"
    );
    assert_eq!(
        requests[2].function_call_output_text(continue_call_id),
        Some("Plan updated".to_string())
    );
    let snapshot = context_snapshot::format_labeled_requests_snapshot(
        "New context window tool installs fresh full context before the next follow-up request.",
        &[("Final Follow-Up Request", &requests[2])],
        &ContextSnapshotOptions::default(),
    );
    insta::assert_snapshot!(
        "token_budget_new_context_window_tool_full_context",
        snapshot
    );

    Ok(())
}
