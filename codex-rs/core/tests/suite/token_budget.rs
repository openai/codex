use anyhow::Result;
use codex_config::types::McpServerConfig;
use codex_config::types::McpServerTransportConfig;
use codex_core::config::TokenBudgetConfig;
use codex_features::Feature;
use codex_model_provider_info::built_in_model_providers;
use codex_protocol::config_types::AutoCompactTokenLimitScope;
use codex_protocol::items::TurnItem;
use codex_protocol::protocol::CONTEXT_WINDOW_CLOSE_TAG;
use codex_protocol::protocol::CONTEXT_WINDOW_GUIDANCE_CLOSE_TAG;
use codex_protocol::protocol::CONTEXT_WINDOW_GUIDANCE_OPEN_TAG;
use codex_protocol::protocol::CONTEXT_WINDOW_OPEN_TAG;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::ItemStartedEvent;
use codex_protocol::protocol::Op;
use core_test_support::PathBufExt;
use core_test_support::assert_regex_match;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::hooks::trust_discovered_hooks;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_completed_with_tokens;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_compact_json_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::stdio_server_bin;
use core_test_support::test_codex::local;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

const CONFIGURED_CONTEXT_WINDOW: i64 = 128_000;

fn token_budget_contexts(request: &ResponsesRequest) -> Vec<String> {
    let context_window_prefix = format!("{CONTEXT_WINDOW_OPEN_TAG}\nThread id: ");
    request
        .message_input_texts("developer")
        .into_iter()
        .filter(|text| text.starts_with(&context_window_prefix))
        .collect()
}

fn token_budget_window_ids(
    text: &str,
    thread_id: codex_protocol::ThreadId,
) -> (String, Option<String>, String) {
    let captures = assert_regex_match(
        &format!(
            r"^{CONTEXT_WINDOW_OPEN_TAG}\nThread id: {thread_id}\nFirst context window id: ([0-9a-f-]{{36}})\nCurrent context window id: ([0-9a-f-]{{36}})(?:\nPrevious context window id: ([0-9a-f-]{{36}}))?\n{CONTEXT_WINDOW_CLOSE_TAG}$"
        ),
        text,
    );
    let first_window_id = captures
        .get(1)
        .expect("first window id capture")
        .as_str()
        .to_string();
    let window_id = captures
        .get(2)
        .expect("window id capture")
        .as_str()
        .to_string();
    let previous_window_id = captures.get(3).map(|capture| capture.as_str().to_string());
    (first_window_id, previous_window_id, window_id)
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

fn ev_completed_with_usage(id: &str, input_tokens: i64, output_tokens: i64) -> Value {
    json!({
        "type": "response.completed",
        "response": {
            "id": id,
            "usage": {
                "input_tokens": input_tokens,
                "input_tokens_details": null,
                "output_tokens": output_tokens,
                "output_tokens_details": null,
                "total_tokens": input_tokens + output_tokens
            }
        }
    })
}

fn python_hook_command(script_path: &Path) -> String {
    format!("python3 \"{}\"", script_path.display())
}

fn write_token_budget_compact_hooks(home: &Path) {
    let script_path = home.join("token_budget_compact_hook.py");
    std::fs::write(
        &script_path,
        "import json\nimport sys\njson.load(sys.stdin)\n",
    )
    .expect("write compact hook script");
    let hooks = json!({
        "hooks": {
            "PreCompact": [{
                "matcher": "manual",
                "hooks": [{
                    "type": "command",
                    "command": python_hook_command(&script_path),
                }]
            }],
            "PostCompact": [{
                "matcher": "manual",
                "hooks": [{
                    "type": "command",
                    "command": python_hook_command(&script_path),
                }]
            }]
        }
    });
    std::fs::write(home.join("hooks.json"), hooks.to_string()).expect("write hooks.json");
}

async fn assert_context_compaction_item_lifecycle(codex: &std::sync::Arc<codex_core::CodexThread>) {
    let mut saw_compaction_started = false;
    let mut saw_compaction_completed = false;

    loop {
        let event = codex.next_event().await.expect("next event");
        match event.msg {
            EventMsg::ItemStarted(ItemStartedEvent {
                item: TurnItem::ContextCompaction(_),
                ..
            }) => saw_compaction_started = true,
            EventMsg::ItemCompleted(ItemCompletedEvent {
                item: TurnItem::ContextCompaction(_),
                ..
            }) => saw_compaction_completed = true,
            EventMsg::TurnComplete(_) => break,
            _ => {}
        }
    }

    assert!(saw_compaction_started, "compaction item should start");
    assert!(saw_compaction_completed, "compaction item should complete");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_budget_context_is_only_emitted_with_full_context() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
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

    test.submit_turn("first turn").await?;

    let second_cwd = test.workspace_path("second-cwd");
    std::fs::create_dir_all(&second_cwd)?;
    test.submit_turn_with_environments("second turn", Some(vec![local(second_cwd.abs())]))
        .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);

    let thread_id = test.session_configured.thread_id;
    let initial_token_budget = token_budget_contexts(&requests[0]);
    assert_eq!(initial_token_budget.len(), 1);
    let (first_window_id, previous_window_id, window_id) =
        token_budget_window_ids(&initial_token_budget[0], thread_id);
    assert_eq!(previous_window_id, None);
    assert_eq!(first_window_id, window_id);
    assert_eq!(
        token_budget_contexts(&requests[1]),
        initial_token_budget,
        "steady-state context update should not advance the context window"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_budget_guidance_follows_context_window() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response = mount_sse_sequence(
        &server,
        vec![sse(vec![
            ev_response_created("resp-1"),
            ev_completed("resp-1"),
        ])],
    )
    .await;
    let guidance_message = "Preserve important state before compaction.";
    let test = test_codex()
        .with_config(move |config| {
            config.model_context_window = Some(CONFIGURED_CONTEXT_WINDOW);
            config.token_budget = Some(TokenBudgetConfig {
                guidance_message: Some(guidance_message.to_string()),
                ..TokenBudgetConfig::default()
            });
            config
                .features
                .enable(Feature::TokenBudget)
                .expect("test config should allow token budget");
        })
        .build_with_auto_env(&server)
        .await?;

    test.submit_turn("inspect context guidance").await?;

    let developer_texts = response.single_request().message_input_texts("developer");
    let context_window_index = developer_texts
        .iter()
        .position(|text| text.starts_with(CONTEXT_WINDOW_OPEN_TAG))
        .expect("context-window metadata should be present");
    assert_eq!(
        developer_texts.get(context_window_index + 1),
        Some(&format!(
            "{CONTEXT_WINDOW_GUIDANCE_OPEN_TAG}\n{guidance_message}\n{CONTEXT_WINDOW_GUIDANCE_CLOSE_TAG}"
        ))
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_budget_context_injects_plain_thread_hint_text() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let rmcp_test_server_bin = stdio_server_bin()?;
    let test = test_codex()
        .with_config(move |config| {
            config.model_context_window = Some(CONFIGURED_CONTEXT_WINDOW);
            config
                .features
                .enable(Feature::TokenBudget)
                .expect("test config should allow token budget");
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(
                "notes".to_string(),
                McpServerConfig {
                    auth: Default::default(),
                    transport: McpServerTransportConfig::Stdio {
                        command: rmcp_test_server_bin,
                        args: Vec::new(),
                        env: None,
                        env_vars: Vec::new(),
                        cwd: None,
                    },
                    environment_id: "local".to_string(),
                    enabled: true,
                    required: false,
                    supports_parallel_tool_calls: false,
                    disabled_reason: None,
                    startup_timeout_sec: Some(Duration::from_secs(10)),
                    tool_timeout_sec: None,
                    default_tools_approval_mode: None,
                    enabled_tools: None,
                    disabled_tools: None,
                    scopes: None,
                    oauth: None,
                    oauth_resource: None,
                    tools: HashMap::new(),
                },
            );
            config
                .mcp_servers
                .set(servers)
                .expect("test mcp servers should accept any configuration");
        })
        .build(&server)
        .await?;
    wait_for_mcp_server(&test.codex, "notes").await?;
    let responses = mount_sse_sequence(
        &server,
        vec![sse(vec![
            ev_response_created("resp-1"),
            ev_completed("resp-1"),
        ])],
    )
    .await;

    test.submit_turn("inject the history hint").await?;

    let request = responses.single_request();
    let thread_id = test.session_configured.thread_id;
    let token_budgets = token_budget_contexts(&request);
    assert_eq!(token_budgets.len(), 1);
    let captures = assert_regex_match(
        &format!(
            r"^{CONTEXT_WINDOW_OPEN_TAG}\nThread id: {thread_id}\nFirst context window id: ([0-9a-f-]{{36}})\nCurrent context window id: ([0-9a-f-]{{36}})\nmanual history hint for thread {thread_id}\nunstructured notes/thread_hint fixture result\n{CONTEXT_WINDOW_CLOSE_TAG}$"
        ),
        &token_budgets[0],
    );
    assert_eq!(
        captures.get(1).expect("first window id capture").as_str(),
        captures.get(2).expect("current window id capture").as_str()
    );
    assert!(
        !tool_names(&request)
            .iter()
            .any(|name| name == "mcp__notes__thread_hint"),
        "thread_hint should be hidden from model tool exposure"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_budget_reminder_emits_after_crossing_compaction_threshold() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_completed_with_tokens("resp-1", /*total_tokens*/ 8_000),
            ]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
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

    test.submit_turn("cross threshold").await?;
    test.submit_turn("observe reminder").await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let initial_context = token_budget_contexts(&requests[0]);
    assert_eq!(initial_context.len(), 1);
    let reminder = "Your context window is nearly exhausted (only 1000 tokens remaining) and will be automatically reset for you soon. Once reset, message items in current context window will be cleared in the new window, but notes and history items will be persistent across windows.";
    assert_eq!(
        requests[1]
            .message_input_texts("developer")
            .into_iter()
            .filter(|text| text == reminder)
            .count(),
        1
    );
    assert_eq!(token_budget_contexts(&requests[1]), initial_context);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_budget_reminder_uses_body_after_prefix_window() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_completed_with_tokens("resp-1", /*total_tokens*/ 8_000),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_completed_with_tokens("resp-2", /*total_tokens*/ 8_600),
            ]),
            sse(vec![ev_response_created("resp-3"), ev_completed("resp-3")]),
        ],
    )
    .await;
    let test = test_codex()
        .with_config(|config| {
            config.model_context_window = Some(10_000);
            config.model_auto_compact_token_limit = Some(1_000);
            config.model_auto_compact_token_limit_scope =
                AutoCompactTokenLimitScope::BodyAfterPrefix;
            config.token_budget = Some(TokenBudgetConfig {
                reminder_threshold_tokens: Some(600),
                ..TokenBudgetConfig::default()
            });
            config
                .features
                .enable(Feature::TokenBudget)
                .expect("test config should allow token budget");
        })
        .build(&server)
        .await?;

    test.submit_turn("establish prefix").await?;
    test.submit_turn("grow body").await?;
    test.submit_turn("observe reminder").await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 3);
    let reminder = "Your context window is nearly exhausted (only 400 tokens remaining) and will be automatically reset for you soon. Once reset, message items in current context window will be cleared in the new window, but notes and history items will be persistent across windows.";
    assert!(
        requests[1]
            .message_input_texts("developer")
            .into_iter()
            .all(|text| text != reminder),
        "first-window prefix should not count against the body-after-prefix reminder threshold"
    );
    assert_eq!(
        requests[2]
            .message_input_texts("developer")
            .into_iter()
            .filter(|text| text == reminder)
            .count(),
        1
    );

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

    let thread_id = test.session_configured.thread_id;
    let remaining_context = "You have 6500 tokens left in this context window.".to_string();
    let token_budgets = token_budget_contexts(&requests[1]);
    assert_eq!(token_budgets.len(), 1);
    token_budget_window_ids(&token_budgets[0], thread_id);
    assert_eq!(
        requests[2].function_call_output_content_and_success(call_id),
        Some((Some(remaining_context), None))
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_context_remaining_uses_body_after_prefix_window() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "remaining-call";
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg-1", "noted"),
                ev_completed_with_usage(
                    "resp-1", /*input_tokens*/ 2_000, /*output_tokens*/ 500,
                ),
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
            config.model_auto_compact_token_limit = Some(7_000);
            config.model_auto_compact_token_limit_scope =
                AutoCompactTokenLimitScope::BodyAfterPrefix;
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

    let remaining_context = "You have 6500 tokens left in this context window.".to_string();
    assert_eq!(
        requests[2].function_call_output_content_and_success(call_id),
        Some((Some(remaining_context), None))
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_context_remaining_returns_unknown_when_threshold_is_unbounded() -> Result<()> {
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

    assert_eq!(token_budget_contexts(&requests[0]), Vec::<String>::new());
    assert_eq!(
        requests[1].function_call_output_content_and_success(call_id),
        Some((
            Some("You have unknown tokens left in this context window.".to_string()),
            None,
        ))
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_budget_context_uses_new_window_after_compaction() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg-1", "assistant before compact"),
                ev_completed("resp-1"),
            ]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        ],
    )
    .await;
    let compact = mount_compact_json_once(&server, json!({ "output": [] })).await;

    let mut model_provider = built_in_model_providers(/*openai_base_url*/ None)["openai"].clone();
    model_provider.base_url = Some(format!("{}/v1", server.uri()));
    model_provider.supports_websockets = false;

    let test = test_codex()
        .with_config(move |config| {
            config.model_provider = model_provider;
            config.model_context_window = Some(CONFIGURED_CONTEXT_WINDOW);
            config
                .features
                .enable(Feature::TokenBudget)
                .expect("test config should allow token budget");
        })
        .build(&server)
        .await?;

    test.submit_turn("before compact").await?;
    test.codex.submit(Op::Compact).await?;
    assert_context_compaction_item_lifecycle(&test.codex).await;
    test.submit_turn("after compact").await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    assert!(
        compact.requests().is_empty(),
        "token budget compaction should not call server-side compaction"
    );

    let thread_id = test.session_configured.thread_id;
    let initial_token_budget = token_budget_contexts(&requests[0]);
    assert_eq!(initial_token_budget.len(), 1);
    let (initial_first_window_id, initial_previous_window_id, initial_window_id) =
        token_budget_window_ids(&initial_token_budget[0], thread_id);
    let post_compaction_token_budget = token_budget_contexts(&requests[1]);
    assert_eq!(post_compaction_token_budget.len(), 1);
    let (
        post_compaction_first_window_id,
        post_compaction_previous_window_id,
        post_compaction_window_id,
    ) = token_budget_window_ids(&post_compaction_token_budget[0], thread_id);
    assert_eq!(initial_previous_window_id, None);
    assert_eq!(initial_first_window_id, initial_window_id);
    assert_eq!(post_compaction_first_window_id, initial_first_window_id);
    assert_eq!(
        post_compaction_previous_window_id.as_deref(),
        Some(initial_window_id.as_str())
    );
    assert_ne!(post_compaction_window_id, initial_window_id);
    assert!(
        !requests[1].body_contains_text("before compact"),
        "token budget compaction should drop prior user messages"
    );
    assert!(
        !requests[1].body_contains_text("assistant before compact"),
        "token budget compaction should drop prior assistant messages"
    );
    assert!(
        requests[1].body_contains_text("after compact"),
        "follow-up should still include the new turn input"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_budget_compaction_runs_compact_hooks() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let test = test_codex()
        .with_pre_build_hook(write_token_budget_compact_hooks)
        .with_config(|config| {
            config.model_context_window = Some(CONFIGURED_CONTEXT_WINDOW);
            config
                .features
                .enable(Feature::TokenBudget)
                .expect("test config should allow token budget");
            trust_discovered_hooks(config);
        })
        .build(&server)
        .await?;

    test.codex.submit(Op::Compact).await?;

    let pre_compact = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::HookCompleted(completed)
            if completed.run.event_name == HookEventName::PreCompact =>
        {
            Some(completed.clone())
        }
        _ => None,
    })
    .await;
    assert_eq!(pre_compact.run.status, HookRunStatus::Completed);

    let post_compact = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::HookCompleted(completed)
            if completed.run.event_name == HookEventName::PostCompact =>
        {
            Some(completed.clone())
        }
        _ => None,
    })
    .await;
    assert_eq!(post_compact.run.status, HookRunStatus::Completed);
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_budget_mid_turn_auto_compaction_resets_before_active_follow_up() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "remaining-call";
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, "get_context_remaining", "{}"),
                ev_completed_with_tokens("resp-1", /*total_tokens*/ 9_500),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    let mut model_provider = built_in_model_providers(/*openai_base_url*/ None)["openai"].clone();
    model_provider.name = "OpenAI (test)".into();
    model_provider.base_url = Some(format!("{}/v1", server.uri()));
    model_provider.supports_websockets = false;
    let test = test_codex()
        .with_config(move |config| {
            config.model_provider = model_provider;
            config.model_context_window = Some(10_000);
            config
                .features
                .enable(Feature::TokenBudget)
                .expect("test config should allow token budget");
        })
        .build(&server)
        .await?;

    test.submit_turn("trigger mid-turn auto compaction").await?;

    let requests = responses.requests();
    assert_eq!(
        requests.len(),
        2,
        "token-budget auto-compaction should reset locally before the continuation"
    );
    assert!(
        !requests[1].input().iter().any(|item| {
            item.get("type").and_then(Value::as_str) == Some("function_call_output")
                && item.get("call_id").and_then(Value::as_str) == Some(call_id)
        }),
        "fresh token-budget windows should drop active tool output with the prior history"
    );

    let thread_id = test.session_configured.thread_id;
    let initial_token_budget = token_budget_contexts(&requests[0]);
    assert_eq!(initial_token_budget.len(), 1);
    let (initial_first_window_id, _, initial_window_id) =
        token_budget_window_ids(&initial_token_budget[0], thread_id);
    let follow_up_token_budget = token_budget_contexts(&requests[1]);
    assert_eq!(follow_up_token_budget.len(), 1);
    let (follow_up_first_window_id, follow_up_previous_window_id, follow_up_window_id) =
        token_budget_window_ids(&follow_up_token_budget[0], thread_id);
    assert_eq!(follow_up_first_window_id, initial_first_window_id);
    assert_eq!(
        follow_up_previous_window_id.as_deref(),
        Some(initial_window_id.as_str())
    );
    assert!(
        !requests[1].body_contains_text("trigger mid-turn auto compaction"),
        "fresh token-budget windows should drop prior user messages"
    );
    assert_ne!(
        follow_up_window_id, initial_window_id,
        "mid-turn token-budget auto-compaction should reset the context window"
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
    let thread_id = test.session_configured.thread_id;
    let initial_token_budget = token_budget_contexts(&requests[0]);
    assert_eq!(initial_token_budget.len(), 1);
    let (initial_first_window_id, _, initial_window_id) =
        token_budget_window_ids(&initial_token_budget[0], thread_id);
    let new_window_token_budget = token_budget_contexts(&requests[2]);
    assert_eq!(new_window_token_budget.len(), 1);
    let (new_first_window_id, new_previous_window_id, new_window_id) =
        token_budget_window_ids(&new_window_token_budget[0], thread_id);
    assert_eq!(new_first_window_id, initial_first_window_id);
    assert_eq!(
        new_previous_window_id.as_deref(),
        Some(initial_window_id.as_str())
    );
    assert_ne!(new_window_id, initial_window_id);
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
    let snapshot = snapshot
        .replace(&thread_id.to_string(), "<THREAD_ID>")
        .replace(&new_first_window_id, "<FIRST_WINDOW_ID>")
        .replace(&new_window_id, "<WINDOW_ID>");
    insta::assert_snapshot!(
        "token_budget_new_context_window_tool_full_context",
        snapshot
    );

    Ok(())
}
