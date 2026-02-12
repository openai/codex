#![allow(clippy::expect_used)]
use codex_core::CodexAuth;
use codex_core::ModelProviderInfo;
use codex_core::built_in_model_providers;
use codex_core::compact::SUMMARIZATION_PROMPT;
use codex_core::compact::SUMMARY_PREFIX;
use codex_core::config::Config;
use codex_core::features::Feature;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ItemCompletedEvent;
use codex_core::protocol::ItemStartedEvent;
use codex_core::protocol::Op;
use codex_core::protocol::RolloutItem;
use codex_core::protocol::RolloutLine;
use codex_core::protocol::SandboxPolicy;
use codex_core::protocol::WarningEvent;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::items::TurnItem;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::user_input::UserInput;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::context_snapshot::ContextSnapshotRenderMode;
use core_test_support::responses::ev_local_shell_call;
use core_test_support::responses::ev_reasoning_item;
use core_test_support::responses::mount_models_once;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use std::path::PathBuf;

use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_completed_with_tokens;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::mount_compact_json_once;
use core_test_support::responses::mount_response_sequence;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::sse_failed;
use core_test_support::responses::sse_response;
use core_test_support::responses::start_mock_server;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::MockServer;
// --- Test helpers -----------------------------------------------------------

pub(super) const FIRST_REPLY: &str = "FIRST_REPLY";
pub(super) const SUMMARY_TEXT: &str = "SUMMARY_ONLY_CONTEXT";
const THIRD_USER_MSG: &str = "next turn";
const AUTO_SUMMARY_TEXT: &str = "AUTO_SUMMARY";
const FIRST_AUTO_MSG: &str = "token limit start";
const SECOND_AUTO_MSG: &str = "token limit push";
const MULTI_AUTO_MSG: &str = "multi auto";
const SECOND_LARGE_REPLY: &str = "SECOND_LARGE_REPLY";
const FIRST_AUTO_SUMMARY: &str = "FIRST_AUTO_SUMMARY";
const SECOND_AUTO_SUMMARY: &str = "SECOND_AUTO_SUMMARY";
const FINAL_REPLY: &str = "FINAL_REPLY";
const CONTEXT_LIMIT_MESSAGE: &str =
    "Your input exceeds the context window of this model. Please adjust your input and try again.";
const DUMMY_FUNCTION_NAME: &str = "test_tool";
const DUMMY_CALL_ID: &str = "call-multi-auto";
const FUNCTION_CALL_LIMIT_MSG: &str = "function call limit push";
const POST_AUTO_USER_MSG: &str = "post auto follow-up";
const PRETURN_CONTEXT_DIFF_CWD: &str = "/tmp/PRETURN_CONTEXT_DIFF_CWD";

pub(super) const COMPACT_WARNING_MESSAGE: &str = "Heads up: Long threads and multiple compactions can cause the model to be less accurate. Start a new thread when possible to keep threads small and targeted.";

fn auto_summary(summary: &str) -> String {
    summary.to_string()
}

fn summary_with_prefix(summary: &str) -> String {
    format!("{SUMMARY_PREFIX}\n{summary}")
}

fn set_test_compact_prompt(config: &mut Config) {
    config.compact_prompt = Some(SUMMARIZATION_PROMPT.to_string());
}

fn body_contains_text(body: &str, text: &str) -> bool {
    body.contains(&json_fragment(text))
}

fn json_fragment(text: &str) -> String {
    serde_json::to_string(text)
        .expect("serialize text to JSON")
        .trim_matches('"')
        .to_string()
}

fn non_openai_model_provider(server: &MockServer) -> ModelProviderInfo {
    let mut provider = built_in_model_providers()["openai"].clone();
    provider.name = "OpenAI (test)".into();
    provider.base_url = Some(format!("{}/v1", server.uri()));
    provider
}

fn model_info_with_context_window(slug: &str, context_window: i64) -> ModelInfo {
    let models_response: ModelsResponse =
        serde_json::from_str(include_str!("../../models.json")).expect("valid models.json");
    let mut model_info = models_response
        .models
        .into_iter()
        .find(|model| model.slug == slug)
        .unwrap_or_else(|| panic!("model `{slug}` missing from models.json"));
    model_info.context_window = Some(context_window);
    model_info
}

fn assert_pre_sampling_switch_compaction_requests(
    first: &serde_json::Value,
    compact: &serde_json::Value,
    follow_up: &serde_json::Value,
    previous_model: &str,
    next_model: &str,
) {
    assert_eq!(first["model"].as_str(), Some(previous_model));
    assert_eq!(compact["model"].as_str(), Some(previous_model));
    assert_eq!(follow_up["model"].as_str(), Some(next_model));

    let compact_body = compact.to_string();
    assert!(
        body_contains_text(&compact_body, SUMMARIZATION_PROMPT),
        "pre-sampling compact request should include summarization prompt"
    );
    let first_body = first.to_string();
    assert!(
        body_contains_text(&first_body, "<environment_context>"),
        "first request should include canonical environment context"
    );
    let follow_up_body = follow_up.to_string();
    assert!(
        body_contains_text(&follow_up_body, "<environment_context>"),
        "follow-up request should preserve canonical environment context after pre-sampling compaction"
    );
}

async fn assert_compaction_uses_turn_lifecycle_id(codex: &std::sync::Arc<codex_core::CodexThread>) {
    let mut turn_started_id = None;
    let mut turn_completed_id = None;
    let mut compact_started_id = None;
    let mut compact_completed_id = None;

    while turn_completed_id.is_none() {
        let event = codex.next_event().await.expect("next event");
        match event.msg {
            EventMsg::TurnStarted(_) => turn_started_id = Some(event.id.clone()),
            EventMsg::ItemStarted(ItemStartedEvent {
                item: TurnItem::ContextCompaction(_),
                ..
            }) => compact_started_id = Some(event.id.clone()),
            EventMsg::ItemCompleted(ItemCompletedEvent {
                item: TurnItem::ContextCompaction(_),
                ..
            }) => compact_completed_id = Some(event.id.clone()),
            EventMsg::TurnComplete(_) => turn_completed_id = Some(event.id.clone()),
            _ => {}
        }
    }

    let turn_started_id = turn_started_id.expect("turn started id");
    let turn_completed_id = turn_completed_id.expect("turn complete id");

    assert_eq!(
        turn_completed_id, turn_started_id,
        "turn start and complete should use the same event id"
    );
    assert_eq!(
        compact_started_id,
        Some(turn_started_id.clone()),
        "compaction item start should use the turn event id"
    );
    assert_eq!(
        compact_completed_id,
        Some(turn_started_id),
        "compaction item completion should use the turn event id"
    );
}
fn context_snapshot_options() -> ContextSnapshotOptions {
    ContextSnapshotOptions::default()
        .render_mode(ContextSnapshotRenderMode::KindWithTextPrefix { max_chars: 64 })
}

fn format_labeled_requests_snapshot(
    scenario: &str,
    sections: &[(&str, &core_test_support::responses::ResponsesRequest)],
) -> String {
    context_snapshot::format_labeled_requests_snapshot(
        scenario,
        sections,
        &context_snapshot_options(),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn summarize_context_three_requests_and_instructions() {
    skip_if_no_network!();

    // Set up a mock server that we can inspect after the run.
    let server = start_mock_server().await;

    // SSE 1: assistant replies normally so it is recorded in history.
    let sse1 = sse(vec![
        ev_assistant_message("m1", FIRST_REPLY),
        ev_completed("r1"),
    ]);

    // SSE 2: summarizer returns a summary message.
    let sse2 = sse(vec![
        ev_assistant_message("m2", SUMMARY_TEXT),
        ev_completed("r2"),
    ]);

    // SSE 3: minimal completed; we only need to capture the request body.
    let sse3 = sse(vec![ev_completed("r3")]);

    // Mount the three expected requests in sequence so the assertions below can
    // inspect them without relying on specific prompt markers.
    let request_log = mount_sse_sequence(&server, vec![sse1, sse2, sse3]).await;

    // Build config pointing to the mock server and spawn Codex.
    let model_provider = non_openai_model_provider(&server);
    let mut builder = test_codex().with_config(move |config| {
        config.model_provider = model_provider;
        set_test_compact_prompt(config);
        config.model_auto_compact_token_limit = Some(200_000);
    });
    let test = builder.build(&server).await.unwrap();
    let codex = test.codex.clone();
    let rollout_path = test.session_configured.rollout_path.expect("rollout path");

    // 1) Normal user input – should hit server once.
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello world".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    // 2) Summarize – second hit should include the summarization prompt.
    codex.submit(Op::Compact).await.unwrap();
    let warning_event = wait_for_event(&codex, |ev| matches!(ev, EventMsg::Warning(_))).await;
    let EventMsg::Warning(WarningEvent { message }) = warning_event else {
        panic!("expected warning event after compact");
    };
    assert_eq!(message, COMPACT_WARNING_MESSAGE);
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    // 3) Next user input – third hit; history should include only the summary.
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: THIRD_USER_MSG.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    // Inspect the three captured requests.
    let requests = request_log.requests();
    assert_eq!(requests.len(), 3, "expected exactly three requests");
    let body1 = requests[0].body_json();
    let body2 = requests[1].body_json();
    let body3 = requests[2].body_json();

    // Manual compact should keep the baseline developer instructions.
    let instr1 = body1.get("instructions").and_then(|v| v.as_str()).unwrap();
    let instr2 = body2.get("instructions").and_then(|v| v.as_str()).unwrap();
    assert_eq!(
        instr1, instr2,
        "manual compact should keep the standard developer instructions"
    );

    // The summarization request should include the injected user input marker.
    let body2_str = body2.to_string();
    let input2 = body2.get("input").and_then(|v| v.as_array()).unwrap();
    let has_compact_prompt = body_contains_text(&body2_str, SUMMARIZATION_PROMPT);
    assert!(
        has_compact_prompt,
        "compaction request should include the summarize trigger"
    );
    // The last item is the user message created from the injected input.
    let last2 = input2.last().unwrap();
    assert_eq!(last2.get("type").unwrap().as_str().unwrap(), "message");
    assert_eq!(last2.get("role").unwrap().as_str().unwrap(), "user");
    let text2 = last2["content"][0]["text"].as_str().unwrap();
    assert_eq!(
        text2, SUMMARIZATION_PROMPT,
        "expected summarize trigger, got `{text2}`"
    );

    // Third request must contain the refreshed instructions, compacted user history, and new user message.
    let input3 = body3.get("input").and_then(|v| v.as_array()).unwrap();

    assert!(
        input3.len() >= 3,
        "expected refreshed context and new user message in third request"
    );

    let mut messages: Vec<(String, String)> = Vec::new();
    let expected_summary_message = summary_with_prefix(SUMMARY_TEXT);

    for item in input3 {
        if let Some("message") = item.get("type").and_then(|v| v.as_str()) {
            let role = item
                .get("role")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let text = item
                .get("content")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|entry| entry.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            messages.push((role, text));
        }
    }

    // No previous assistant messages should remain and the new user message is present.
    let assistant_count = messages.iter().filter(|(r, _)| r == "assistant").count();
    assert_eq!(assistant_count, 0, "assistant history should be cleared");
    assert!(
        messages
            .iter()
            .any(|(r, t)| r == "user" && t == THIRD_USER_MSG),
        "third request should include the new user message"
    );
    assert!(
        messages
            .iter()
            .any(|(r, t)| r == "user" && t == "hello world"),
        "third request should include the original user message"
    );
    assert!(
        messages
            .iter()
            .any(|(r, t)| r == "user" && t == &expected_summary_message),
        "third request should include the summary message"
    );
    assert!(
        !messages
            .iter()
            .any(|(_, text)| text.contains(SUMMARIZATION_PROMPT)),
        "third request should not include the summarize trigger"
    );

    // Shut down Codex to flush rollout entries before inspecting the file.
    codex.submit(Op::Shutdown).await.unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ShutdownComplete)).await;

    // Verify rollout contains APITurn entries for each API call and a Compacted entry.
    println!("rollout path: {}", rollout_path.display());
    let text = std::fs::read_to_string(&rollout_path).unwrap_or_else(|e| {
        panic!(
            "failed to read rollout file {}: {e}",
            rollout_path.display()
        )
    });
    let mut api_turn_count = 0usize;
    let mut saw_compacted_summary = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(entry): Result<RolloutLine, _> = serde_json::from_str(trimmed) else {
            continue;
        };
        match entry.item {
            RolloutItem::TurnContext(_) => {
                api_turn_count += 1;
            }
            RolloutItem::Compacted(ci) => {
                if ci.message == expected_summary_message {
                    saw_compacted_summary = true;
                }
            }
            _ => {}
        }
    }

    assert!(
        api_turn_count == 3,
        "expected three APITurn entries in rollout"
    );
    assert!(
        saw_compacted_summary,
        "expected a Compacted entry containing the summarizer output"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_compact_uses_custom_prompt() {
    skip_if_no_network!();

    let server = start_mock_server().await;
    let first_turn = sse(vec![
        ev_assistant_message("m0", FIRST_REPLY),
        ev_completed_with_tokens("r0", 80),
    ]);
    let compact_turn = sse(vec![
        ev_assistant_message("m1", SUMMARY_TEXT),
        ev_completed_with_tokens("r1", 100),
    ]);
    let request_log = mount_sse_sequence(&server, vec![first_turn, compact_turn]).await;

    let custom_prompt = "Use this compact prompt instead";

    let model_provider = non_openai_model_provider(&server);
    let mut builder = test_codex().with_config(move |config| {
        config.model_provider = model_provider;
        config.compact_prompt = Some(custom_prompt.to_string());
    });
    let codex = builder
        .build(&server)
        .await
        .expect("create conversation")
        .codex;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "USER_ONE".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .expect("submit first user turn");
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex.submit(Op::Compact).await.expect("trigger compact");
    let warning_event = wait_for_event(&codex, |ev| matches!(ev, EventMsg::Warning(_))).await;
    let EventMsg::Warning(WarningEvent { message }) = warning_event else {
        panic!("expected warning event after compact");
    };
    assert_eq!(message, COMPACT_WARNING_MESSAGE);
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = request_log.requests();
    assert_eq!(
        requests.len(),
        2,
        "expected first turn and compact requests"
    );
    let body = requests[1].body_json();

    let input = body
        .get("input")
        .and_then(|v| v.as_array())
        .expect("input array");
    let mut found_custom_prompt = false;
    let mut found_default_prompt = false;

    for item in input {
        if item["type"].as_str() != Some("message") {
            continue;
        }
        let text = item["content"][0]["text"].as_str().unwrap_or_default();
        if text == custom_prompt {
            found_custom_prompt = true;
        }
        if text == SUMMARIZATION_PROMPT {
            found_default_prompt = true;
        }
    }

    let used_prompt = found_custom_prompt || found_default_prompt;
    if used_prompt {
        assert!(found_custom_prompt, "custom prompt should be injected");
        assert!(
            !found_default_prompt,
            "default prompt should be replaced when a compact prompt is used"
        );
    } else {
        assert!(
            !found_default_prompt,
            "summarization prompt should not appear if compaction omits a prompt"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_compact_emits_api_and_local_token_usage_events() {
    skip_if_no_network!();

    let server = start_mock_server().await;

    let sse_turn = sse(vec![
        ev_assistant_message("m0", FIRST_REPLY),
        ev_completed_with_tokens("r0", 0),
    ]);
    // Compact run where the API reports zero tokens in usage. Our local
    // estimator should still compute a non-zero context size for the compacted
    // history.
    let sse_compact = sse(vec![
        ev_assistant_message("m1", SUMMARY_TEXT),
        ev_completed_with_tokens("r1", 0),
    ]);
    mount_sse_sequence(&server, vec![sse_turn, sse_compact]).await;

    let model_provider = non_openai_model_provider(&server);
    let mut builder = test_codex().with_config(move |config| {
        config.model_provider = model_provider;
        set_test_compact_prompt(config);
    });
    let codex = builder.build(&server).await.unwrap().codex;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "seed compact history".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    // Trigger manual compact and collect TokenCount events for the compact turn.
    codex.submit(Op::Compact).await.unwrap();

    let mut compact_turn_token_totals = Vec::new();
    loop {
        let event = wait_for_event(&codex, |_| true).await;
        match event {
            EventMsg::TokenCount(tc) => {
                if let Some(info) = tc.info {
                    compact_turn_token_totals.push(info.last_token_usage.total_tokens);
                }
            }
            EventMsg::TurnComplete(_) => break,
            _ => {}
        }
    }

    assert!(
        compact_turn_token_totals.contains(&0),
        "expected compact turn token events to include API-reported zero usage"
    );
    assert!(
        compact_turn_token_totals.iter().any(|total| *total > 0),
        "expected compact turn token events to include a non-zero local estimate"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_compact_emits_context_compaction_items() {
    skip_if_no_network!();

    let server = start_mock_server().await;

    let sse1 = sse(vec![
        ev_assistant_message("m1", FIRST_REPLY),
        ev_completed("r1"),
    ]);
    let sse2 = sse(vec![
        ev_assistant_message("m2", SUMMARY_TEXT),
        ev_completed("r2"),
    ]);
    mount_sse_sequence(&server, vec![sse1, sse2]).await;

    let model_provider = non_openai_model_provider(&server);
    let mut builder = test_codex().with_config(move |config| {
        config.model_provider = model_provider;
        set_test_compact_prompt(config);
    });
    let codex = builder.build(&server).await.unwrap().codex;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "manual compact".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();
    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    codex.submit(Op::Compact).await.unwrap();

    let mut started_item = None;
    let mut completed_item = None;
    let mut legacy_event = false;
    let mut saw_turn_complete = false;

    while !saw_turn_complete || started_item.is_none() || completed_item.is_none() || !legacy_event
    {
        let event = codex.next_event().await.unwrap();
        match event.msg {
            EventMsg::ItemStarted(ItemStartedEvent {
                item: TurnItem::ContextCompaction(item),
                ..
            }) => {
                started_item = Some(item);
            }
            EventMsg::ItemCompleted(ItemCompletedEvent {
                item: TurnItem::ContextCompaction(item),
                ..
            }) => {
                completed_item = Some(item);
            }
            EventMsg::ContextCompacted(_) => {
                legacy_event = true;
            }
            EventMsg::TurnComplete(_) => {
                saw_turn_complete = true;
            }
            _ => {}
        }
    }

    let started_item = started_item.expect("context compaction item started");
    let completed_item = completed_item.expect("context compaction item completed");
    assert_eq!(started_item.id, completed_item.id);
    assert!(legacy_event);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn multiple_auto_compact_per_task_runs_after_token_limit_hit() {
    skip_if_no_network!();

    let server = start_mock_server().await;

    let non_openai_provider_name = non_openai_model_provider(&server).name;
    let codex = test_codex()
        .with_config(move |config| {
            config.model_provider.name = non_openai_provider_name;
        })
        .build(&server)
        .await
        .expect("build codex")
        .codex;

    // user message
    let user_message = "create an app";

    // Prepare the mock responses from the model

    // summary texts from model
    let first_summary_text = "The task is to create an app. I started to create a react app.";
    let second_summary_text = "The task is to create an app. I started to create a react app. then I realized that I need to create a node app.";
    let third_summary_text = "The task is to create an app. I started to create a react app. then I realized that I need to create a node app. then I realized that I need to create a python app.";
    // summary texts with prefix
    let prefixed_first_summary = summary_with_prefix(first_summary_text);
    let prefixed_second_summary = summary_with_prefix(second_summary_text);
    let prefixed_third_summary = summary_with_prefix(third_summary_text);
    // token used count after long work
    let token_count_used = 270_000;
    // token used count after compaction
    let token_count_used_after_compaction = 80000;

    // mock responses from the model

    let reasoning_response_1 = ev_reasoning_item("m1", &["I will create a react app"], &[]);
    let encrypted_content_1 = reasoning_response_1["item"]["encrypted_content"]
        .as_str()
        .unwrap();

    // first chunk of work
    let model_reasoning_response_1_sse = sse(vec![
        reasoning_response_1.clone(),
        ev_local_shell_call("r1-shell", "completed", vec!["echo", "make-react"]),
        ev_completed_with_tokens("r1", token_count_used),
    ]);

    // first compaction response
    let model_compact_response_1_sse = sse(vec![
        ev_assistant_message("m2", first_summary_text),
        ev_completed_with_tokens("r2", token_count_used_after_compaction),
    ]);

    let reasoning_response_2 = ev_reasoning_item("m3", &["I will create a node app"], &[]);
    let encrypted_content_2 = reasoning_response_2["item"]["encrypted_content"]
        .as_str()
        .unwrap();

    // second chunk of work
    let model_reasoning_response_2_sse = sse(vec![
        reasoning_response_2.clone(),
        ev_local_shell_call("r3-shell", "completed", vec!["echo", "make-node"]),
        ev_completed_with_tokens("r3", token_count_used),
    ]);

    // second compaction response
    let model_compact_response_2_sse = sse(vec![
        ev_assistant_message("m4", second_summary_text),
        ev_completed_with_tokens("r4", token_count_used_after_compaction),
    ]);

    let reasoning_response_3 = ev_reasoning_item("m6", &["I will create a python app"], &[]);
    let encrypted_content_3 = reasoning_response_3["item"]["encrypted_content"]
        .as_str()
        .unwrap();

    // third chunk of work
    let model_reasoning_response_3_sse = sse(vec![
        ev_reasoning_item("m6", &["I will create a python app"], &[]),
        ev_local_shell_call("r6-shell", "completed", vec!["echo", "make-python"]),
        ev_completed_with_tokens("r6", token_count_used),
    ]);

    // third compaction response
    let model_compact_response_3_sse = sse(vec![
        ev_assistant_message("m7", third_summary_text),
        ev_completed_with_tokens("r7", token_count_used_after_compaction),
    ]);

    // final response
    let model_final_response_sse = sse(vec![
        ev_assistant_message(
            "m8",
            "The task is to create an app. I started to create a react app. then I realized that I need to create a node app. then I realized that I need to create a python app.",
        ),
        ev_completed_with_tokens("r8", token_count_used_after_compaction + 1000),
    ]);

    // mount the mock responses from the model
    let bodies = vec![
        model_reasoning_response_1_sse,
        model_compact_response_1_sse,
        model_reasoning_response_2_sse,
        model_compact_response_2_sse,
        model_reasoning_response_3_sse,
        model_compact_response_3_sse,
        model_final_response_sse,
    ];
    let request_log = mount_sse_sequence(&server, bodies).await;

    // Start the conversation with the user message
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: user_message.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .expect("submit user input");
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    // collect the requests payloads from the model
    let requests_payloads = request_log.requests();
    let body = requests_payloads[0].body_json();
    let input = body.get("input").and_then(|v| v.as_array()).unwrap();

    fn normalize_inputs(values: &[serde_json::Value]) -> Vec<serde_json::Value> {
        values
            .iter()
            .filter(|value| {
                if value
                    .get("type")
                    .and_then(|ty| ty.as_str())
                    .is_some_and(|ty| ty == "function_call_output")
                {
                    return false;
                }

                let text = value
                    .get("content")
                    .and_then(|content| content.as_array())
                    .and_then(|content| content.first())
                    .and_then(|item| item.get("text"))
                    .and_then(|text| text.as_str());

                // Ignore cached prefix messages (project docs + permissions) since they are not
                // relevant to compaction behavior and can change as bundled prompts evolve.
                let role = value.get("role").and_then(|role| role.as_str());
                if role == Some("developer")
                    && text.is_some_and(|text| text.contains("`sandbox_mode`"))
                {
                    return false;
                }
                !text.is_some_and(|text| text.starts_with("# AGENTS.md instructions for "))
            })
            .cloned()
            .collect()
    }

    let initial_input = normalize_inputs(input);
    let environment_message = initial_input[0]["content"][0]["text"].as_str().unwrap();

    // test 1: after compaction, we should have one environment message, one user message, and one user message with summary prefix
    let compaction_indices = [2, 4, 6];
    let expected_summaries = [
        prefixed_first_summary.as_str(),
        prefixed_second_summary.as_str(),
        prefixed_third_summary.as_str(),
    ];
    for (i, expected_summary) in compaction_indices.into_iter().zip(expected_summaries) {
        let body = requests_payloads.clone()[i].body_json();
        let input = body.get("input").and_then(|v| v.as_array()).unwrap();
        let input = normalize_inputs(input);
        assert_eq!(input.len(), 3);
        let environment_message = input[0]["content"][0]["text"].as_str().unwrap();
        let user_message_received = input[1]["content"][0]["text"].as_str().unwrap();
        let summary_message = input[2]["content"][0]["text"].as_str().unwrap();
        assert_eq!(environment_message, environment_message);
        assert_eq!(user_message_received, user_message);
        assert_eq!(
            summary_message, expected_summary,
            "compaction request at index {i} should include the prefixed summary"
        );
    }

    // test 2: the expected requests inputs should be as follows:
    let expected_requests_inputs = json!([
    [
        // 0: first request of the user message.
      {
        "content": [
          {
            "text": environment_message,
            "type": "input_text"
          }
        ],
        "role": "user",
        "type": "message"
      },
      {
        "content": [
          {
            "text": "create an app",
            "type": "input_text"
          }
        ],
        "role": "user",
        "type": "message"
      }
    ]
    ,
    [
        // 1: first automatic compaction request.
      {
        "content": [
          {
            "text": environment_message,
            "type": "input_text"
          }
        ],
        "role": "user",
        "type": "message"
      },
      {
        "content": [
          {
            "text": "create an app",
            "type": "input_text"
          }
        ],
        "role": "user",
        "type": "message"
      },
      {
        "content": null,
        "encrypted_content": encrypted_content_1,
        "summary": [
          {
            "text": "I will create a react app",
            "type": "summary_text"
          }
        ],
        "type": "reasoning"
      },
      {
        "action": {
          "command": [
            "echo",
            "make-react"
          ],
          "env": null,
          "timeout_ms": null,
          "type": "exec",
          "user": null,
          "working_directory": null
        },
        "call_id": "r1-shell",
        "status": "completed",
        "type": "local_shell_call"
      },
      {
        "call_id": "r1-shell",
        "output": "execution error: Io(Os { code: 2, kind: NotFound, message: \"No such file or directory\" })",
        "type": "function_call_output"
      },
      {
        "content": [
          {
            "text": SUMMARIZATION_PROMPT,
            "type": "input_text"
          }
        ],
        "role": "user",
        "type": "message"
      }
    ]
    ,
    [
      // 2: request after first automatic compaction.
      {
        "content": [
          {
            "text": environment_message,
            "type": "input_text"
          }
        ],
        "role": "user",
        "type": "message"
      },
      {
        "content": [
          {
            "text": "create an app",
            "type": "input_text"
          }
        ],
        "role": "user",
        "type": "message"
      },
      {
        "content": [
          {
            "text": prefixed_first_summary.clone(),
            "type": "input_text"
          }
        ],
        "role": "user",
        "type": "message"
      }
    ]
    ,
    [
        // 3: request for second automatic compaction.
      {
        "content": [
          {
            "text": environment_message,
            "type": "input_text"
          }
        ],
        "role": "user",
        "type": "message"
      },
      {
        "content": [
          {
            "text": "create an app",
            "type": "input_text"
          }
        ],
        "role": "user",
        "type": "message"
      },
      {
        "content": [
          {
            "text": prefixed_first_summary.clone(),
            "type": "input_text"
          }
        ],
        "role": "user",
        "type": "message"
      },
      {
        "content": null,
        "encrypted_content": encrypted_content_2,
        "summary": [
          {
            "text": "I will create a node app",
            "type": "summary_text"
          }
        ],
        "type": "reasoning"
      },
      {
        "action": {
          "command": [
            "echo",
            "make-node"
          ],
          "env": null,
          "timeout_ms": null,
          "type": "exec",
          "user": null,
          "working_directory": null
        },
        "call_id": "r3-shell",
        "status": "completed",
        "type": "local_shell_call"
      },
      {
        "call_id": "r3-shell",
        "output": "execution error: Io(Os { code: 2, kind: NotFound, message: \"No such file or directory\" })",
        "type": "function_call_output"
      },
      {
        "content": [
          {
            "text": SUMMARIZATION_PROMPT,
            "type": "input_text"
          }
        ],
        "role": "user",
        "type": "message"
      }
    ]
    ,
    // 4: request after second automatic compaction.
    [
      {
        "content": [
          {
            "text": environment_message,
            "type": "input_text"
          }
        ],
        "role": "user",
        "type": "message"
      },
      {
        "content": [
          {
            "text": "create an app",
            "type": "input_text"
          }
        ],
        "role": "user",
        "type": "message"
      },
      {
        "content": [
          {
            "text": prefixed_second_summary.clone(),
            "type": "input_text"
          }
        ],
        "role": "user",
        "type": "message"
      }
    ]
    ,
    [
      // 5: request for third automatic compaction.
      {
        "content": [
          {
            "text": environment_message,
            "type": "input_text"
          }
        ],
        "role": "user",
        "type": "message"
      },
      {
        "content": [
          {
            "text": "create an app",
            "type": "input_text"
          }
        ],
        "role": "user",
        "type": "message"
      },
      {
        "content": [
          {
            "text": prefixed_second_summary.clone(),
            "type": "input_text"
          }
        ],
        "role": "user",
        "type": "message"
      },
      {
        "content": null,
        "encrypted_content": encrypted_content_3,
        "summary": [
          {
            "text": "I will create a python app",
            "type": "summary_text"
          }
        ],
        "type": "reasoning"
      },
      {
        "action": {
          "command": [
            "echo",
            "make-python"
          ],
          "env": null,
          "timeout_ms": null,
          "type": "exec",
          "user": null,
          "working_directory": null
        },
        "call_id": "r6-shell",
        "status": "completed",
        "type": "local_shell_call"
      },
      {
        "call_id": "r6-shell",
        "output": "execution error: Io(Os { code: 2, kind: NotFound, message: \"No such file or directory\" })",
        "type": "function_call_output"
      },
      {
        "content": [
          {
            "text": SUMMARIZATION_PROMPT,
            "type": "input_text"
          }
        ],
        "role": "user",
        "type": "message"
      }
    ]
    ,
    [
      {
        // 6: request after third automatic compaction.
        "content": [
          {
            "text": environment_message,
            "type": "input_text"
          }
        ],
        "role": "user",
        "type": "message"
      },
      {
        "content": [
          {
            "text": "create an app",
            "type": "input_text"
          }
        ],
        "role": "user",
        "type": "message"
      },
      {
        "content": [
          {
            "text": prefixed_third_summary.clone(),
            "type": "input_text"
          }
        ],
        "role": "user",
        "type": "message"
      }
    ]
    ]);

    for (i, request) in requests_payloads.iter().enumerate() {
        let body = request.body_json();
        let input = body.get("input").and_then(|v| v.as_array()).unwrap();
        let expected_input = expected_requests_inputs[i].as_array().unwrap();
        assert_eq!(normalize_inputs(input), normalize_inputs(expected_input));
    }

    // test 3: the number of requests should be 7
    assert_eq!(requests_payloads.len(), 7);
}

// Windows CI only: bump to 4 workers to prevent SSE/event starvation and test timeouts.
#[cfg_attr(windows, tokio::test(flavor = "multi_thread", worker_threads = 4))]
#[cfg_attr(not(windows), tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn auto_compact_runs_after_token_limit_hit() {
    skip_if_no_network!();

    let server = start_mock_server().await;

    let sse1 = sse(vec![
        ev_assistant_message("m1", FIRST_REPLY),
        ev_completed_with_tokens("r1", 70_000),
    ]);

    let sse2 = sse(vec![
        ev_assistant_message("m2", "SECOND_REPLY"),
        ev_completed_with_tokens("r2", 330_000),
    ]);

    let sse3 = sse(vec![
        ev_assistant_message("m3", AUTO_SUMMARY_TEXT),
        ev_completed_with_tokens("r3", 200),
    ]);
    let sse4 = sse(vec![
        ev_assistant_message("m4", FINAL_REPLY),
        ev_completed_with_tokens("r4", 120),
    ]);
    let prefixed_auto_summary = AUTO_SUMMARY_TEXT;

    let request_log = mount_sse_sequence(&server, vec![sse1, sse2, sse3, sse4]).await;

    let model_provider = non_openai_model_provider(&server);

    let mut builder = test_codex().with_config(move |config| {
        config.model_provider = model_provider;
        set_test_compact_prompt(config);
        config.model_auto_compact_token_limit = Some(200_000);
    });
    let codex = builder.build(&server).await.unwrap().codex;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: FIRST_AUTO_MSG.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: SECOND_AUTO_MSG.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: POST_AUTO_USER_MSG.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = request_log.requests();
    let request_bodies: Vec<String> = requests
        .iter()
        .map(|request| request.body_json().to_string())
        .collect();
    assert_eq!(
        request_bodies.len(),
        4,
        "expected user turns, a compaction request, and the follow-up turn; got {}",
        request_bodies.len()
    );
    let auto_compact_count = request_bodies
        .iter()
        .filter(|body| body_contains_text(body, SUMMARIZATION_PROMPT))
        .count();
    assert_eq!(
        auto_compact_count, 1,
        "expected exactly one auto compact request"
    );
    let auto_compact_index = request_bodies
        .iter()
        .enumerate()
        .find_map(|(idx, body)| body_contains_text(body, SUMMARIZATION_PROMPT).then_some(idx))
        .expect("auto compact request missing");
    assert_eq!(
        auto_compact_index, 2,
        "auto compact should add a third request"
    );

    let follow_up_index = request_bodies
        .iter()
        .enumerate()
        .rev()
        .find_map(|(idx, body)| {
            (body.contains(POST_AUTO_USER_MSG) && !body_contains_text(body, SUMMARIZATION_PROMPT))
                .then_some(idx)
        })
        .expect("follow-up request missing");
    assert_eq!(follow_up_index, 3, "follow-up request should be last");

    let body_first = requests[0].body_json();
    let body_auto = requests[auto_compact_index].body_json();
    let body_follow_up = requests[follow_up_index].body_json();
    let instructions = body_auto
        .get("instructions")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let baseline_instructions = body_first
        .get("instructions")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    assert_eq!(
        instructions, baseline_instructions,
        "auto compact should keep the standard developer instructions",
    );

    let input_auto = body_auto.get("input").and_then(|v| v.as_array()).unwrap();
    let last_auto = input_auto
        .last()
        .expect("auto compact request should append a user message");
    assert_eq!(
        last_auto.get("type").and_then(|v| v.as_str()),
        Some("message")
    );
    assert_eq!(last_auto.get("role").and_then(|v| v.as_str()), Some("user"));
    let last_text = last_auto
        .get("content")
        .and_then(|v| v.as_array())
        .and_then(|items| items.first())
        .and_then(|item| item.get("text"))
        .and_then(|text| text.as_str())
        .unwrap_or_default();
    assert_eq!(
        last_text, SUMMARIZATION_PROMPT,
        "auto compact should send the summarization prompt as a user message",
    );

    let input_follow_up = body_follow_up
        .get("input")
        .and_then(|v| v.as_array())
        .unwrap();
    let user_texts: Vec<String> = input_follow_up
        .iter()
        .filter(|item| item.get("type").and_then(|v| v.as_str()) == Some("message"))
        .filter(|item| item.get("role").and_then(|v| v.as_str()) == Some("user"))
        .filter_map(|item| {
            item.get("content")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|entry| entry.get("text"))
                .and_then(|v| v.as_str())
                .map(std::string::ToString::to_string)
        })
        .collect();
    assert!(
        user_texts.iter().any(|text| text == FIRST_AUTO_MSG),
        "auto compact follow-up request should include the first user message"
    );
    assert!(
        user_texts.iter().any(|text| text == SECOND_AUTO_MSG),
        "auto compact follow-up request should include the second user message"
    );
    assert!(
        user_texts.iter().any(|text| text == POST_AUTO_USER_MSG),
        "auto compact follow-up request should include the new user message"
    );
    assert!(
        user_texts
            .iter()
            .any(|text| text.contains(prefixed_auto_summary)),
        "auto compact follow-up request should include the summary message"
    );
}

// Windows CI only: bump to 4 workers to prevent SSE/event starvation and test timeouts.
#[cfg_attr(windows, tokio::test(flavor = "multi_thread", worker_threads = 4))]
#[cfg_attr(not(windows), tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn auto_compact_emits_context_compaction_items() {
    skip_if_no_network!();

    let server = start_mock_server().await;

    let sse1 = sse(vec![
        ev_assistant_message("m1", FIRST_REPLY),
        ev_completed_with_tokens("r1", 70_000),
    ]);
    let sse2 = sse(vec![
        ev_assistant_message("m2", "SECOND_REPLY"),
        ev_completed_with_tokens("r2", 330_000),
    ]);
    let sse3 = sse(vec![
        ev_assistant_message("m3", AUTO_SUMMARY_TEXT),
        ev_completed_with_tokens("r3", 200),
    ]);
    let sse4 = sse(vec![
        ev_assistant_message("m4", FINAL_REPLY),
        ev_completed_with_tokens("r4", 120),
    ]);

    mount_sse_sequence(&server, vec![sse1, sse2, sse3, sse4]).await;

    let model_provider = non_openai_model_provider(&server);
    let mut builder = test_codex().with_config(move |config| {
        config.model_provider = model_provider;
        set_test_compact_prompt(config);
        config.model_auto_compact_token_limit = Some(200_000);
    });
    let codex = builder.build(&server).await.unwrap().codex;

    let mut started_item = None;
    let mut completed_item = None;
    let mut legacy_event = false;

    for user in [FIRST_AUTO_MSG, SECOND_AUTO_MSG, POST_AUTO_USER_MSG] {
        codex
            .submit(Op::UserInput {
                items: vec![UserInput::Text {
                    text: user.into(),
                    text_elements: Vec::new(),
                }],
                final_output_json_schema: None,
            })
            .await
            .unwrap();

        loop {
            let event = codex.next_event().await.unwrap();
            match event.msg {
                EventMsg::ItemStarted(ItemStartedEvent {
                    item: TurnItem::ContextCompaction(item),
                    ..
                }) => {
                    started_item = Some(item);
                }
                EventMsg::ItemCompleted(ItemCompletedEvent {
                    item: TurnItem::ContextCompaction(item),
                    ..
                }) => {
                    completed_item = Some(item);
                }
                EventMsg::ContextCompacted(_) => {
                    legacy_event = true;
                }
                EventMsg::TurnComplete(_) if !event.id.starts_with("auto-compact-") => {
                    break;
                }
                _ => {}
            }
        }
    }

    let started_item = started_item.expect("context compaction item started");
    let completed_item = completed_item.expect("context compaction item completed");
    assert_eq!(started_item.id, completed_item.id);
    assert!(legacy_event);
}

// Windows CI only: bump to 4 workers to prevent SSE/event starvation and test timeouts.
#[cfg_attr(windows, tokio::test(flavor = "multi_thread", worker_threads = 4))]
#[cfg_attr(not(windows), tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn auto_compact_starts_after_turn_started() {
    skip_if_no_network!();

    let server = start_mock_server().await;

    let sse1 = sse(vec![
        ev_assistant_message("m1", FIRST_REPLY),
        ev_completed_with_tokens("r1", 70_000),
    ]);
    let sse2 = sse(vec![
        ev_assistant_message("m2", "SECOND_REPLY"),
        ev_completed_with_tokens("r2", 330_000),
    ]);
    let sse3 = sse(vec![
        ev_assistant_message("m3", AUTO_SUMMARY_TEXT),
        ev_completed_with_tokens("r3", 200),
    ]);
    let sse4 = sse(vec![
        ev_assistant_message("m4", FINAL_REPLY),
        ev_completed_with_tokens("r4", 120),
    ]);

    mount_sse_sequence(&server, vec![sse1, sse2, sse3, sse4]).await;

    let model_provider = non_openai_model_provider(&server);
    let mut builder = test_codex().with_config(move |config| {
        config.model_provider = model_provider;
        set_test_compact_prompt(config);
        config.model_auto_compact_token_limit = Some(200_000);
    });
    let codex = builder.build(&server).await.unwrap().codex;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: FIRST_AUTO_MSG.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: SECOND_AUTO_MSG.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: POST_AUTO_USER_MSG.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();

    let first = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::TurnStarted(_) => Some("turn"),
        EventMsg::ItemStarted(ItemStartedEvent {
            item: TurnItem::ContextCompaction(_),
            ..
        }) => Some("compaction"),
        _ => None,
    })
    .await;
    assert_eq!(first, "turn", "compaction started before turn started");

    wait_for_event(&codex, |ev| {
        matches!(
            ev,
            EventMsg::ItemStarted(ItemStartedEvent {
                item: TurnItem::ContextCompaction(_),
                ..
            })
        )
    })
    .await;

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auto_compact_runs_after_resume_when_token_usage_is_over_limit() {
    skip_if_no_network!();

    let server = start_mock_server().await;

    let limit = 200_000;
    let over_limit_tokens = 250_000;
    let remote_summary = "REMOTE_COMPACT_SUMMARY";

    let compacted_history = vec![
        codex_protocol::models::ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![codex_protocol::models::ContentItem::OutputText {
                text: remote_summary.to_string(),
            }],
            end_turn: None,
            phase: None,
        },
        codex_protocol::models::ResponseItem::Compaction {
            encrypted_content: "ENCRYPTED_COMPACTION_SUMMARY".to_string(),
        },
    ];
    let compact_mock =
        mount_compact_json_once(&server, serde_json::json!({ "output": compacted_history })).await;

    let mut builder = test_codex().with_config(move |config| {
        set_test_compact_prompt(config);
        config.model_auto_compact_token_limit = Some(limit);
    });
    let initial = builder.build(&server).await.unwrap();
    let home = initial.home.clone();
    let rollout_path = initial
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path");

    // A single over-limit completion should not auto-compact until the next user message.
    mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("m1", FIRST_REPLY),
            ev_completed_with_tokens("r1", over_limit_tokens),
        ]),
    )
    .await;
    initial.submit_turn("OVER_LIMIT_TURN").await.unwrap();

    assert!(
        compact_mock.requests().is_empty(),
        "remote compaction should not run before the next user message"
    );

    let mut resume_builder = test_codex().with_config(move |config| {
        set_test_compact_prompt(config);
        config.model_auto_compact_token_limit = Some(limit);
    });
    let resumed = resume_builder
        .resume(&server, home, rollout_path)
        .await
        .unwrap();

    let follow_up_user = "AFTER_RESUME_USER";
    let sse_follow_up = sse(vec![
        ev_assistant_message("m2", FINAL_REPLY),
        ev_completed("r2"),
    ]);

    let follow_up_matcher = move |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(follow_up_user) && body.contains(remote_summary)
    };
    mount_sse_once_match(&server, follow_up_matcher, sse_follow_up).await;

    resumed
        .codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: follow_up_user.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: resumed.cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: resumed.session_configured.model.clone(),
            effort: None,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await
        .unwrap();

    wait_for_event(&resumed.codex, |event| {
        matches!(event, EventMsg::ContextCompacted(_))
    })
    .await;
    wait_for_event(&resumed.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let compact_requests = compact_mock.requests();
    assert_eq!(
        compact_requests.len(),
        1,
        "remote compaction should run once after resume"
    );
    assert_eq!(
        compact_requests[0].path(),
        "/v1/responses/compact",
        "remote compaction should hit the compact endpoint"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pre_sampling_compact_runs_on_switch_to_smaller_context_model() {
    skip_if_no_network!();

    let server = MockServer::start().await;
    let previous_model = "gpt-5.2-codex";
    let next_model = "gpt-5.1-codex-max";

    let models_mock = mount_models_once(
        &server,
        ModelsResponse {
            models: vec![
                model_info_with_context_window(previous_model, 273_000),
                model_info_with_context_window(next_model, 125_000),
            ],
        },
    )
    .await;

    let request_log = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_assistant_message("m1", "before switch"),
                ev_completed_with_tokens("r1", 120_000),
            ]),
            sse(vec![
                ev_assistant_message("m2", "PRE_SAMPLING_SUMMARY"),
                ev_completed_with_tokens("r2", 10),
            ]),
            sse(vec![
                ev_assistant_message("m3", "after switch"),
                ev_completed_with_tokens("r3", 100),
            ]),
        ],
    )
    .await;

    let model_provider = non_openai_model_provider(&server);
    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model(previous_model)
        .with_config(move |config| {
            config.model_provider = model_provider;
            set_test_compact_prompt(config);
            config.features.enable(Feature::RemoteModels);
        });
    let test = builder.build(&server).await.expect("build test codex");

    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "before switch".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: previous_model.to_string(),
            effort: None,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await
        .expect("submit first user turn");
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "after switch".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: next_model.to_string(),
            effort: None,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await
        .expect("submit second user turn");
    assert_compaction_uses_turn_lifecycle_id(&test.codex).await;

    let requests = request_log.requests();
    assert_eq!(models_mock.requests().len(), 1);
    assert_eq!(
        requests.len(),
        3,
        "expected user, compact, and follow-up requests"
    );
    assert_pre_sampling_switch_compaction_requests(
        &requests[0].body_json(),
        &requests[1].body_json(),
        &requests[2].body_json(),
        previous_model,
        next_model,
    );

    insta::assert_snapshot!(
        "pre_sampling_model_switch_compaction_shapes",
        format_labeled_requests_snapshot(
            "Pre-sampling compaction on model switch to a smaller context window: current behavior compacts using prior-turn history only (incoming user message excluded), and the follow-up request carries compacted history plus the new user message.",
            &[
                ("Initial Request (Previous Model)", &requests[0]),
                ("Pre-sampling Compaction Request", &requests[1]),
                (
                    "Post-Compaction Follow-up Request (Next Model)",
                    &requests[2]
                ),
            ]
        )
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pre_sampling_compact_runs_after_resume_and_switch_to_smaller_model() {
    skip_if_no_network!();

    let server = MockServer::start().await;
    let previous_model = "gpt-5.2-codex";
    let next_model = "gpt-5.1-codex-max";

    let models_mock = mount_models_once(
        &server,
        ModelsResponse {
            models: vec![
                model_info_with_context_window(previous_model, 273_000),
                model_info_with_context_window(next_model, 125_000),
            ],
        },
    )
    .await;

    let request_log = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_assistant_message("m1", "before resume"),
                ev_completed_with_tokens("r1", 120_000),
            ]),
            sse(vec![
                ev_assistant_message("m2", "PRE_SAMPLING_SUMMARY"),
                ev_completed_with_tokens("r2", 10),
            ]),
            sse(vec![
                ev_assistant_message("m3", "after resume"),
                ev_completed_with_tokens("r3", 100),
            ]),
        ],
    )
    .await;

    let model_provider = non_openai_model_provider(&server);
    let mut initial_builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model(previous_model)
        .with_config(move |config| {
            config.model_provider = model_provider;
            set_test_compact_prompt(config);
            config.features.enable(Feature::RemoteModels);
        });
    let initial = initial_builder
        .build(&server)
        .await
        .expect("build initial test codex");
    let home = initial.home.clone();
    let rollout_path = initial
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path");

    initial
        .codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "before resume".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: initial.cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: previous_model.to_string(),
            effort: None,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await
        .expect("submit pre-resume turn");
    wait_for_event(&initial.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    initial
        .codex
        .submit(Op::Shutdown)
        .await
        .expect("shutdown initial session");
    wait_for_event(&initial.codex, |event| {
        matches!(event, EventMsg::ShutdownComplete)
    })
    .await;

    let model_provider = non_openai_model_provider(&server);
    let mut resumed_builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model(previous_model)
        .with_config(move |config| {
            config.model_provider = model_provider;
            set_test_compact_prompt(config);
            config.features.enable(Feature::RemoteModels);
        });
    let resumed = resumed_builder
        .resume(&server, home, rollout_path)
        .await
        .expect("resume codex");

    resumed
        .codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "after resume".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: resumed.cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: next_model.to_string(),
            effort: None,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await
        .expect("submit resumed user turn");
    assert_compaction_uses_turn_lifecycle_id(&resumed.codex).await;

    let requests = request_log.requests();
    assert_eq!(models_mock.requests().len(), 1);
    assert_eq!(
        requests.len(),
        3,
        "expected user, compact, and follow-up requests"
    );
    assert_pre_sampling_switch_compaction_requests(
        &requests[0].body_json(),
        &requests[1].body_json(),
        &requests[2].body_json(),
        previous_model,
        next_model,
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auto_compact_persists_rollout_entries() {
    skip_if_no_network!();

    let server = start_mock_server().await;

    let sse1 = sse(vec![
        ev_assistant_message("m1", FIRST_REPLY),
        ev_completed_with_tokens("r1", 70_000),
    ]);

    let sse2 = sse(vec![
        ev_assistant_message("m2", "SECOND_REPLY"),
        ev_completed_with_tokens("r2", 330_000),
    ]);

    let auto_summary_payload = auto_summary(AUTO_SUMMARY_TEXT);
    let sse3 = sse(vec![
        ev_assistant_message("m3", &auto_summary_payload),
        ev_completed_with_tokens("r3", 200),
    ]);
    let sse4 = sse(vec![
        ev_assistant_message("m4", FINAL_REPLY),
        ev_completed_with_tokens("r4", 120),
    ]);

    let first_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(FIRST_AUTO_MSG)
            && !body.contains(SECOND_AUTO_MSG)
            && !body_contains_text(body, SUMMARIZATION_PROMPT)
    };
    mount_sse_once_match(&server, first_matcher, sse1).await;

    let second_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(SECOND_AUTO_MSG)
            && body.contains(FIRST_AUTO_MSG)
            && !body_contains_text(body, SUMMARIZATION_PROMPT)
    };
    mount_sse_once_match(&server, second_matcher, sse2).await;

    let third_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body_contains_text(body, SUMMARIZATION_PROMPT)
    };
    mount_sse_once_match(&server, third_matcher, sse3).await;

    let fourth_matcher = |req: &wiremock::Request| {
        let body = std::str::from_utf8(&req.body).unwrap_or("");
        body.contains(POST_AUTO_USER_MSG) && !body_contains_text(body, SUMMARIZATION_PROMPT)
    };
    mount_sse_once_match(&server, fourth_matcher, sse4).await;

    let model_provider = non_openai_model_provider(&server);

    let mut builder = test_codex().with_config(move |config| {
        config.model_provider = model_provider;
        set_test_compact_prompt(config);
        config.model_auto_compact_token_limit = Some(200_000);
    });
    let test = builder.build(&server).await.unwrap();
    let codex = test.codex.clone();
    let session_configured = test.session_configured;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: FIRST_AUTO_MSG.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: SECOND_AUTO_MSG.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: POST_AUTO_USER_MSG.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex.submit(Op::Shutdown).await.unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ShutdownComplete)).await;

    let rollout_path = session_configured.rollout_path.expect("rollout path");
    let text = std::fs::read_to_string(&rollout_path).unwrap_or_else(|e| {
        panic!(
            "failed to read rollout file {}: {e}",
            rollout_path.display()
        )
    });

    let mut turn_context_count = 0usize;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(entry): Result<RolloutLine, _> = serde_json::from_str(trimmed) else {
            continue;
        };
        match entry.item {
            RolloutItem::TurnContext(_) => {
                turn_context_count += 1;
            }
            RolloutItem::Compacted(_) => {}
            _ => {}
        }
    }

    assert!(
        turn_context_count >= 2,
        "expected at least two turn context entries, got {turn_context_count}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_compact_retries_after_context_window_error() {
    skip_if_no_network!();

    let server = start_mock_server().await;

    let user_turn = sse(vec![
        ev_assistant_message("m1", FIRST_REPLY),
        ev_completed("r1"),
    ]);
    let compact_failed = sse_failed(
        "resp-fail",
        "context_length_exceeded",
        CONTEXT_LIMIT_MESSAGE,
    );
    let compact_succeeds = sse(vec![
        ev_assistant_message("m2", SUMMARY_TEXT),
        ev_completed("r2"),
    ]);

    let request_log = mount_sse_sequence(
        &server,
        vec![
            user_turn.clone(),
            compact_failed.clone(),
            compact_succeeds.clone(),
        ],
    )
    .await;

    let model_provider = non_openai_model_provider(&server);

    let mut builder = test_codex().with_config(move |config| {
        config.model_provider = model_provider;
        set_test_compact_prompt(config);
        config.model_auto_compact_token_limit = Some(200_000);
    });
    let codex = builder.build(&server).await.unwrap().codex;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "first turn".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex.submit(Op::Compact).await.unwrap();
    let EventMsg::BackgroundEvent(event) =
        wait_for_event(&codex, |ev| matches!(ev, EventMsg::BackgroundEvent(_))).await
    else {
        panic!("expected background event after compact retry");
    };
    assert!(
        event.message.contains("Trimmed 1 older thread item"),
        "background event should mention trimmed item count: {}",
        event.message
    );
    let warning_event = wait_for_event(&codex, |ev| matches!(ev, EventMsg::Warning(_))).await;
    let EventMsg::Warning(WarningEvent { message }) = warning_event else {
        panic!("expected warning event after compact retry");
    };
    assert_eq!(message, COMPACT_WARNING_MESSAGE);
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = request_log.requests();
    assert_eq!(
        requests.len(),
        3,
        "expected user turn and two compact attempts"
    );

    let compact_attempt = requests[1].body_json();
    let retry_attempt = requests[2].body_json();

    let compact_input = compact_attempt["input"]
        .as_array()
        .unwrap_or_else(|| panic!("compact attempt missing input array: {compact_attempt}"));
    let retry_input = retry_attempt["input"]
        .as_array()
        .unwrap_or_else(|| panic!("retry attempt missing input array: {retry_attempt}"));
    let compact_contains_prompt =
        body_contains_text(&compact_attempt.to_string(), SUMMARIZATION_PROMPT);
    let retry_contains_prompt =
        body_contains_text(&retry_attempt.to_string(), SUMMARIZATION_PROMPT);
    assert_eq!(
        compact_contains_prompt, retry_contains_prompt,
        "compact attempts should consistently include or omit the summarization prompt"
    );
    assert_eq!(
        retry_input.len(),
        compact_input.len().saturating_sub(1),
        "retry should drop exactly one history item (before {} vs after {})",
        compact_input.len(),
        retry_input.len()
    );
    if let (Some(first_before), Some(first_after)) = (compact_input.first(), retry_input.first()) {
        assert_ne!(
            first_before, first_after,
            "retry should drop the oldest conversation item"
        );
    } else {
        panic!("expected non-empty compact inputs");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// TODO(ccunningham): Re-enable after the follow-up compaction behavior PR lands.
// Current main behavior around non-context manual /compact failures is known-incorrect.
#[ignore = "behavior change covered in follow-up compaction PR"]
async fn manual_compact_non_context_failure_retries_then_emits_task_error() {
    skip_if_no_network!();

    let server = start_mock_server().await;

    let user_turn = sse(vec![
        ev_assistant_message("m1", FIRST_REPLY),
        ev_completed("r1"),
    ]);
    let compact_failed_1 = sse_failed(
        "resp-fail-1",
        "server_error",
        "temporary compact failure one",
    );
    let compact_failed_2 = sse_failed(
        "resp-fail-2",
        "server_error",
        "temporary compact failure two",
    );

    mount_sse_sequence(&server, vec![user_turn, compact_failed_1, compact_failed_2]).await;

    let mut model_provider = non_openai_model_provider(&server);
    model_provider.stream_max_retries = Some(1);

    let codex = test_codex()
        .with_config(move |config| {
            config.model_provider = model_provider;
            set_test_compact_prompt(config);
            config.model_auto_compact_token_limit = Some(200_000);
        })
        .build(&server)
        .await
        .expect("build codex")
        .codex;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "first turn".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .expect("submit user input");
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex.submit(Op::Compact).await.expect("trigger compact");

    let reconnect_message = wait_for_event_match(&codex, |event| match event {
        EventMsg::StreamError(stream_error) => Some(stream_error.message.clone()),
        _ => None,
    })
    .await;
    assert!(
        reconnect_message.contains("Reconnecting... 1/1"),
        "expected reconnect stream error message, got {reconnect_message}"
    );

    let task_error_message = wait_for_event_match(&codex, |event| match event {
        EventMsg::Error(err) => Some(err.message.clone()),
        _ => None,
    })
    .await;
    assert!(
        task_error_message.contains("Error running local compact task"),
        "expected local compact task error prefix, got {task_error_message}"
    );
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_compact_twice_preserves_latest_user_messages() {
    skip_if_no_network!();

    let first_user_message = "first manual turn";
    let second_user_message = "second manual turn";
    let final_user_message = "post compact follow-up";
    let first_summary = "FIRST_MANUAL_SUMMARY";
    let second_summary = "SECOND_MANUAL_SUMMARY";
    let expected_second_summary = summary_with_prefix(second_summary);

    let server = start_mock_server().await;

    let first_turn = sse(vec![
        ev_assistant_message("m1", FIRST_REPLY),
        ev_completed("r1"),
    ]);
    let first_compact_summary = auto_summary(first_summary);
    let first_compact = sse(vec![
        ev_assistant_message("m2", &first_compact_summary),
        ev_completed("r2"),
    ]);
    let second_turn = sse(vec![
        ev_assistant_message("m3", SECOND_LARGE_REPLY),
        ev_completed("r3"),
    ]);
    let second_compact_summary = auto_summary(second_summary);
    let second_compact = sse(vec![
        ev_assistant_message("m4", &second_compact_summary),
        ev_completed("r4"),
    ]);
    let final_turn = sse(vec![
        ev_assistant_message("m5", FINAL_REPLY),
        ev_completed("r5"),
    ]);

    let responses_mock = mount_sse_sequence(
        &server,
        vec![
            first_turn,
            first_compact,
            second_turn,
            second_compact,
            final_turn,
        ],
    )
    .await;

    let model_provider = non_openai_model_provider(&server);

    let mut builder = test_codex().with_config(move |config| {
        config.model_provider = model_provider;
        set_test_compact_prompt(config);
    });
    let codex = builder.build(&server).await.unwrap().codex;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: first_user_message.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex.submit(Op::Compact).await.unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: second_user_message.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex.submit(Op::Compact).await.unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: final_user_message.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = responses_mock.requests();
    assert_eq!(
        requests.len(),
        5,
        "expected exactly 5 requests (user turn, compact, user turn, compact, final turn)"
    );
    let contains_user_text = |request: &core_test_support::responses::ResponsesRequest,
                              expected: &str| {
        request
            .message_input_texts("user")
            .iter()
            .any(|text| text == expected)
    };

    assert!(
        contains_user_text(&requests[0], first_user_message),
        "first turn request missing first user message"
    );
    assert!(
        !contains_user_text(&requests[0], SUMMARIZATION_PROMPT),
        "first turn request should not include summarization prompt"
    );

    assert!(
        contains_user_text(&requests[1], first_user_message),
        "first compact request should include history before compaction"
    );

    assert!(
        contains_user_text(&requests[2], second_user_message),
        "second turn request missing second user message"
    );
    assert!(
        contains_user_text(&requests[2], first_user_message),
        "second turn request should include the compacted user history"
    );

    assert!(
        contains_user_text(&requests[3], second_user_message),
        "second compact request should include latest history"
    );

    insta::assert_snapshot!(
        "manual_compact_with_history_shapes",
        format_labeled_requests_snapshot(
            "Manual /compact with prior user history compacts existing history and the follow-up turn includes the compact summary plus new user message.",
            &[
                ("Local Compaction Request", &requests[1]),
                ("Local Post-Compaction History Layout", &requests[2]),
            ]
        )
    );

    let first_compact_has_prompt = contains_user_text(&requests[1], SUMMARIZATION_PROMPT);
    let second_compact_has_prompt = contains_user_text(&requests[3], SUMMARIZATION_PROMPT);
    assert_eq!(
        first_compact_has_prompt, second_compact_has_prompt,
        "compact requests should consistently include or omit the summarization prompt"
    );

    let final_request_user_texts = requests
        .last()
        .unwrap_or_else(|| panic!("final turn request missing for {final_user_message}"))
        .message_input_texts("user");
    let Some(first_user_index) = final_request_user_texts
        .iter()
        .position(|text| text == first_user_message)
    else {
        panic!("final request missing first user message: {final_request_user_texts:?}");
    };
    let final_output = &final_request_user_texts[first_user_index..];
    let expected = vec![
        first_user_message.to_string(),
        second_user_message.to_string(),
        expected_second_summary,
        final_user_message.to_string(),
    ];
    let mut final_output_iter = final_output.iter();
    for expected_text in &expected {
        final_output_iter
            .position(|text| text == expected_text)
            .unwrap_or_else(|| {
                panic!(
                    "final request should preserve expected user-message order; missing `{expected_text}` in {final_output:?}"
                )
            });
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auto_compact_allows_multiple_attempts_when_interleaved_with_other_turn_events() {
    skip_if_no_network!();

    let server = start_mock_server().await;

    let sse1 = sse(vec![
        ev_assistant_message("m1", FIRST_REPLY),
        ev_completed_with_tokens("r1", 500),
    ]);
    let first_summary_payload = auto_summary(FIRST_AUTO_SUMMARY);
    let sse2 = sse(vec![
        ev_assistant_message("m2", &first_summary_payload),
        ev_completed_with_tokens("r2", 50),
    ]);
    let sse3 = sse(vec![
        ev_function_call(DUMMY_CALL_ID, DUMMY_FUNCTION_NAME, "{}"),
        ev_completed_with_tokens("r3", 150),
    ]);
    let sse4 = sse(vec![
        ev_assistant_message("m4", SECOND_LARGE_REPLY),
        ev_completed_with_tokens("r4", 450),
    ]);
    let second_summary_payload = auto_summary(SECOND_AUTO_SUMMARY);
    let sse5 = sse(vec![
        ev_assistant_message("m5", &second_summary_payload),
        ev_completed_with_tokens("r5", 60),
    ]);
    let sse6 = sse(vec![
        ev_assistant_message("m6", FINAL_REPLY),
        ev_completed_with_tokens("r6", 120),
    ]);
    let follow_up_user = "FOLLOW_UP_AUTO_COMPACT";
    let final_user = "FINAL_AUTO_COMPACT";

    let request_log = mount_sse_sequence(&server, vec![sse1, sse2, sse3, sse4, sse5, sse6]).await;

    let model_provider = non_openai_model_provider(&server);

    let mut builder = test_codex().with_config(move |config| {
        config.model_provider = model_provider;
        set_test_compact_prompt(config);
        config.model_auto_compact_token_limit = Some(200);
    });
    let codex = builder.build(&server).await.unwrap().codex;

    let mut auto_compact_lifecycle_events = Vec::new();
    for user in [MULTI_AUTO_MSG, follow_up_user, final_user] {
        codex
            .submit(Op::UserInput {
                items: vec![UserInput::Text {
                    text: user.into(),
                    text_elements: Vec::new(),
                }],
                final_output_json_schema: None,
            })
            .await
            .unwrap();

        loop {
            let event = codex.next_event().await.unwrap();
            if event.id.starts_with("auto-compact-")
                && matches!(
                    event.msg,
                    EventMsg::TurnStarted(_) | EventMsg::TurnComplete(_)
                )
            {
                auto_compact_lifecycle_events.push(event);
                continue;
            }
            if let EventMsg::TurnComplete(_) = &event.msg
                && !event.id.starts_with("auto-compact-")
            {
                break;
            }
        }
    }

    assert!(
        auto_compact_lifecycle_events.is_empty(),
        "auto compact should not emit task lifecycle events"
    );

    let request_bodies: Vec<String> = request_log
        .requests()
        .into_iter()
        .map(|request| request.body_json().to_string())
        .collect();
    assert_eq!(
        request_bodies.len(),
        6,
        "expected six requests including two auto compactions"
    );
    assert!(
        request_bodies[0].contains(MULTI_AUTO_MSG),
        "first request should contain the user input"
    );
    assert!(
        body_contains_text(&request_bodies[1], SUMMARIZATION_PROMPT),
        "first auto compact request should include the summarization prompt"
    );
    assert!(
        request_bodies[3].contains(&format!("unsupported call: {DUMMY_FUNCTION_NAME}")),
        "function call output should be sent before the second auto compact"
    );
    assert!(
        body_contains_text(&request_bodies[4], SUMMARIZATION_PROMPT),
        "second auto compact request should include the summarization prompt"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn snapshot_request_shape_mid_turn_continuation_compaction() {
    skip_if_no_network!();

    let server = start_mock_server().await;

    let context_window = 100;
    let limit = context_window * 90 / 100;
    let over_limit_tokens = context_window * 95 / 100 + 1;

    let first_turn = sse(vec![
        ev_function_call(DUMMY_CALL_ID, DUMMY_FUNCTION_NAME, "{}"),
        ev_completed_with_tokens("r1", over_limit_tokens),
    ]);
    let auto_summary_payload = auto_summary(AUTO_SUMMARY_TEXT);
    let auto_compact_turn = sse(vec![
        ev_assistant_message("m2", &auto_summary_payload),
        ev_completed_with_tokens("r3", 10),
    ]);
    let post_auto_compact_turn = sse(vec![
        ev_assistant_message("m3", FINAL_REPLY),
        ev_completed_with_tokens("r4", 10),
    ]);

    // Mount responses in order and keep mocks only for the ones we assert on.
    let first_turn_mock = mount_sse_once(&server, first_turn).await;
    let auto_compact_mock = mount_sse_once(&server, auto_compact_turn).await;
    let post_auto_compact_mock = mount_sse_once(&server, post_auto_compact_turn).await;

    let model_provider = non_openai_model_provider(&server);

    let mut builder = test_codex().with_config(move |config| {
        config.model_provider = model_provider;
        set_test_compact_prompt(config);
        config.model_context_window = Some(context_window);
        config.model_auto_compact_token_limit = Some(limit);
    });
    let codex = builder.build(&server).await.unwrap().codex;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: FUNCTION_CALL_LIMIT_MSG.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |msg| matches!(msg, EventMsg::TurnComplete(_))).await;

    // Assert first request captured expected user message that triggers function call.
    let first_request = first_turn_mock.single_request().input();
    assert!(
        first_request.iter().any(|item| {
            item.get("type").and_then(|value| value.as_str()) == Some("message")
                && item
                    .get("content")
                    .and_then(|content| content.as_array())
                    .and_then(|entries| entries.first())
                    .and_then(|entry| entry.get("text"))
                    .and_then(|value| value.as_str())
                    == Some(FUNCTION_CALL_LIMIT_MSG)
        }),
        "first request should include the user message that triggers the function call"
    );

    let function_call_output = auto_compact_mock
        .single_request()
        .function_call_output(DUMMY_CALL_ID);
    let output_text = function_call_output
        .get("output")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    assert!(
        output_text.contains(DUMMY_FUNCTION_NAME),
        "function call output should be sent before auto compact"
    );

    let auto_compact_body = auto_compact_mock.single_request().body_json().to_string();
    assert!(
        body_contains_text(&auto_compact_body, SUMMARIZATION_PROMPT),
        "mid-turn auto compact request should include the summarization prompt after exceeding 95% (limit {limit})"
    );

    insta::assert_snapshot!(
        "mid_turn_compaction_shapes",
        format_labeled_requests_snapshot(
            "True mid-turn continuation compaction after tool output: compact request includes tool artifacts, and the continuation request includes the summary in the same turn.",
            &[
                (
                    "Local Compaction Request",
                    &auto_compact_mock.single_request()
                ),
                (
                    "Local Post-Compaction History Layout",
                    &post_auto_compact_mock.single_request()
                ),
            ]
        )
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auto_compact_clamps_config_limit_to_context_window() {
    skip_if_no_network!();

    let server = start_mock_server().await;

    let context_window = 100;
    let config_limit = 200;
    let over_limit_tokens = context_window * 90 / 100 + 1;

    let first_turn = sse(vec![
        ev_assistant_message("m1", FIRST_REPLY),
        ev_completed_with_tokens("r1", over_limit_tokens),
    ]);
    let auto_summary_payload = auto_summary(AUTO_SUMMARY_TEXT);
    let auto_compact_turn = sse(vec![
        ev_assistant_message("m2", &auto_summary_payload),
        ev_completed_with_tokens("r2", 10),
    ]);
    let post_auto_compact_turn = sse(vec![ev_completed_with_tokens("r3", 10)]);

    let first_turn_mock = mount_sse_once(&server, first_turn).await;
    let auto_compact_mock = mount_sse_once(&server, auto_compact_turn).await;
    mount_sse_once(&server, post_auto_compact_turn).await;

    let model_provider = non_openai_model_provider(&server);
    let mut builder = test_codex().with_config(move |config| {
        config.model_provider = model_provider;
        set_test_compact_prompt(config);
        config.model_context_window = Some(context_window);
        config.model_auto_compact_token_limit = Some(config_limit);
    });
    let codex = builder.build(&server).await.unwrap();

    codex.submit_turn("OVER_LIMIT_TURN").await.unwrap();
    codex.submit_turn("FOLLOW_UP_AFTER_CLAMP").await.unwrap();

    assert!(
        first_turn_mock.single_request().input().iter().any(|item| {
            item.get("type").and_then(|value| value.as_str()) == Some("message")
                && item
                    .get("content")
                    .and_then(|content| content.as_array())
                    .and_then(|entries| entries.first())
                    .and_then(|entry| entry.get("text"))
                    .and_then(|value| value.as_str())
                    == Some("OVER_LIMIT_TURN")
        }),
        "first request should contain the over-limit user input"
    );

    let auto_compact_body = auto_compact_mock.single_request().body_json().to_string();
    assert!(
        body_contains_text(&auto_compact_body, "OVER_LIMIT_TURN"),
        "auto compact should run when the configured limit clamps to the model context window"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auto_compact_counts_encrypted_reasoning_before_last_user() {
    skip_if_no_network!();

    let server = start_mock_server().await;

    let first_user = "COUNT_PRE_LAST_REASONING";
    let second_user = "TRIGGER_COMPACT_AT_LIMIT";
    let third_user = "AFTER_REMOTE_COMPACT";

    let pre_last_reasoning_content = "a".repeat(2_400);
    let post_last_reasoning_content = "b".repeat(4_000);

    let first_turn = sse(vec![
        ev_reasoning_item("pre-reasoning", &["pre"], &[&pre_last_reasoning_content]),
        ev_completed_with_tokens("r1", 10),
    ]);
    let second_turn = sse(vec![
        ev_reasoning_item("post-reasoning", &["post"], &[&post_last_reasoning_content]),
        ev_completed_with_tokens("r2", 80),
    ]);
    let third_turn = sse(vec![
        ev_assistant_message("m4", FINAL_REPLY),
        ev_completed_with_tokens("r4", 1),
    ]);

    let request_log = mount_sse_sequence(
        &server,
        vec![
            // Turn 1: reasoning before last user (should count).
            first_turn,
            // Turn 2: reasoning after last user (should be ignored for compaction).
            second_turn,
            // Turn 3: next user turn after remote compaction.
            third_turn,
        ],
    )
    .await;

    let compacted_history = vec![
        codex_protocol::models::ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![codex_protocol::models::ContentItem::OutputText {
                text: "REMOTE_COMPACT_SUMMARY".to_string(),
            }],
            end_turn: None,
            phase: None,
        },
        codex_protocol::models::ResponseItem::Compaction {
            encrypted_content: "ENCRYPTED_COMPACTION_SUMMARY".to_string(),
        },
    ];
    let compact_mock =
        mount_compact_json_once(&server, serde_json::json!({ "output": compacted_history })).await;

    let codex = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(|config| {
            set_test_compact_prompt(config);
            config.model_auto_compact_token_limit = Some(300);
        })
        .build(&server)
        .await
        .expect("build codex")
        .codex;

    for (idx, user) in [first_user, second_user, third_user]
        .into_iter()
        .enumerate()
    {
        codex
            .submit(Op::UserInput {
                items: vec![UserInput::Text {
                    text: user.into(),
                    text_elements: Vec::new(),
                }],
                final_output_json_schema: None,
            })
            .await
            .unwrap();
        wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

        if idx < 2 {
            assert!(
                compact_mock.requests().is_empty(),
                "remote compaction should not run before the next user turn"
            );
        }
    }

    let compact_requests = compact_mock.requests();
    assert_eq!(
        compact_requests.len(),
        1,
        "remote compaction should run once after the second turn"
    );
    assert_eq!(
        compact_requests[0].path(),
        "/v1/responses/compact",
        "remote compaction should hit the compact endpoint"
    );

    let requests = request_log.requests();
    assert_eq!(
        requests.len(),
        3,
        "conversation should include three user turns"
    );
    let second_request_body = requests[1].body_json().to_string();
    assert!(
        !second_request_body.contains("REMOTE_COMPACT_SUMMARY"),
        "second turn should not include compacted history"
    );
    let third_request_body = requests[2].body_json().to_string();
    assert!(
        third_request_body.contains("REMOTE_COMPACT_SUMMARY")
            || third_request_body.contains(FINAL_REPLY),
        "third turn should include compacted history"
    );
    assert!(
        third_request_body.contains("ENCRYPTED_COMPACTION_SUMMARY"),
        "third turn should include compaction summary item"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auto_compact_runs_when_reasoning_header_clears_between_turns() {
    skip_if_no_network!();

    let server = start_mock_server().await;

    let first_user = "SERVER_INCLUDED_FIRST";
    let second_user = "SERVER_INCLUDED_SECOND";
    let third_user = "SERVER_INCLUDED_THIRD";

    let pre_last_reasoning_content = "a".repeat(2_400);
    let post_last_reasoning_content = "b".repeat(4_000);

    let first_turn = sse(vec![
        ev_reasoning_item("pre-reasoning", &["pre"], &[&pre_last_reasoning_content]),
        ev_completed_with_tokens("r1", 10),
    ]);
    let second_turn = sse(vec![
        ev_reasoning_item("post-reasoning", &["post"], &[&post_last_reasoning_content]),
        ev_completed_with_tokens("r2", 80),
    ]);
    let third_turn = sse(vec![
        ev_assistant_message("m4", FINAL_REPLY),
        ev_completed_with_tokens("r4", 1),
    ]);

    let responses = vec![
        sse_response(first_turn).insert_header("X-Reasoning-Included", "true"),
        sse_response(second_turn),
        sse_response(third_turn),
    ];
    mount_response_sequence(&server, responses).await;

    let compacted_history = vec![
        codex_protocol::models::ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![codex_protocol::models::ContentItem::OutputText {
                text: "REMOTE_COMPACT_SUMMARY".to_string(),
            }],
            end_turn: None,
            phase: None,
        },
        codex_protocol::models::ResponseItem::Compaction {
            encrypted_content: "ENCRYPTED_COMPACTION_SUMMARY".to_string(),
        },
    ];
    let compact_mock =
        mount_compact_json_once(&server, serde_json::json!({ "output": compacted_history })).await;

    let codex = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(|config| {
            set_test_compact_prompt(config);
            config.model_auto_compact_token_limit = Some(300);
        })
        .build(&server)
        .await
        .expect("build codex")
        .codex;

    for user in [first_user, second_user, third_user] {
        codex
            .submit(Op::UserInput {
                items: vec![UserInput::Text {
                    text: user.into(),
                    text_elements: Vec::new(),
                }],
                final_output_json_schema: None,
            })
            .await
            .unwrap();
        wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
    }

    let compact_requests = compact_mock.requests();
    assert_eq!(
        compact_requests.len(),
        1,
        "remote compaction should run once after the reasoning header clears"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn snapshot_request_shape_pre_turn_compaction_including_incoming_user_message() {
    let server = start_mock_server().await;

    let sse1 = sse(vec![
        ev_assistant_message("m1", FIRST_REPLY),
        ev_completed_with_tokens("r1", 60),
    ]);
    let sse2 = sse(vec![
        ev_assistant_message("m2", "SECOND_REPLY"),
        ev_completed_with_tokens("r2", 500),
    ]);
    let sse3 = sse(vec![
        ev_assistant_message("m3", "PRE_TURN_SUMMARY"),
        ev_completed_with_tokens("r3", 100),
    ]);
    let sse4 = sse(vec![
        ev_assistant_message("m4", FINAL_REPLY),
        ev_completed_with_tokens("r4", 80),
    ]);
    let request_log = mount_sse_sequence(&server, vec![sse1, sse2, sse3, sse4]).await;

    let model_provider = non_openai_model_provider(&server);
    let codex = test_codex()
        .with_config(move |config| {
            config.model_provider = model_provider;
            set_test_compact_prompt(config);
            config.model_auto_compact_token_limit = Some(200);
        })
        .build(&server)
        .await
        .expect("build codex")
        .codex;

    for user in ["USER_ONE", "USER_TWO"] {
        codex
            .submit(Op::UserInput {
                items: vec![UserInput::Text {
                    text: user.to_string(),
                    text_elements: Vec::new(),
                }],
                final_output_json_schema: None,
            })
            .await
            .expect("submit user input");
        wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
    }
    codex
        .submit(Op::OverrideTurnContext {
            cwd: Some(PathBuf::from(PRETURN_CONTEXT_DIFF_CWD)),
            approval_policy: None,
            sandbox_policy: None,
            windows_sandbox_level: None,
            model: None,
            effort: None,
            summary: None,
            collaboration_mode: None,
            personality: None,
        })
        .await
        .expect("override turn context");
    let image_url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR4nGNgYAAAAAMAASsJTYQAAAAASUVORK5CYII="
        .to_string();
    codex
        .submit(Op::UserInput {
            items: vec![
                UserInput::Image {
                    image_url: image_url.clone(),
                },
                UserInput::Text {
                    text: "USER_THREE".to_string(),
                    text_elements: Vec::new(),
                },
            ],
            final_output_json_schema: None,
        })
        .await
        .expect("submit user input");
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = request_log.requests();
    assert_eq!(requests.len(), 4, "expected user, user, compact, follow-up");

    insta::assert_snapshot!(
        "pre_turn_compaction_including_incoming_shapes",
        format_labeled_requests_snapshot(
            "Pre-turn auto-compaction with a context override includes incoming user content in the compact request and preserves it after compaction.",
            &[
                ("Local Compaction Request", &requests[2]),
                ("Local Post-Compaction History Layout", &requests[3]),
            ]
        )
    );
    let compact_request_user_texts = requests[2].message_input_texts("user");
    assert!(
        compact_request_user_texts
            .iter()
            .any(|text| text == "USER_THREE"),
        "pre-turn compaction request should include the incoming user message"
    );
    let compact_request_user_images = requests[2].message_input_image_urls("user");
    assert!(
        compact_request_user_images
            .iter()
            .any(|url| url == image_url.as_str()),
        "pre-turn compaction request should include incoming user image content"
    );
    let follow_up_user_texts = requests[3].message_input_texts("user");
    assert!(
        follow_up_user_texts.iter().any(|text| text == "USER_THREE"),
        "expected post-compaction follow-up request to keep incoming user text"
    );
    let follow_up_user_images = requests[3].message_input_image_urls("user");
    assert!(
        follow_up_user_images
            .iter()
            .any(|url| url == image_url.as_str()),
        "expected post-compaction follow-up request to keep incoming user image content"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// TODO(ccunningham): Update once pre-turn compaction context-overflow handling includes incoming
// user input and emits richer oversized-input messaging.
async fn snapshot_request_shape_pre_turn_compaction_context_window_exceeded() {
    let server = start_mock_server().await;

    let first_turn = sse(vec![
        ev_assistant_message("m1", FIRST_REPLY),
        ev_completed_with_tokens("r1", 500),
    ]);
    let mut responses = vec![first_turn];
    responses.extend(
        (0..6).map(|_| {
            sse_failed(
                "compact-failed",
                "context_length_exceeded",
                "Your input exceeds the context window of this model. Please adjust your input and try again.",
            )
        }),
    );
    let request_log = mount_sse_sequence(&server, responses).await;

    let mut model_provider = non_openai_model_provider(&server);
    model_provider.stream_max_retries = Some(0);
    let codex = test_codex()
        .with_config(move |config| {
            config.model_provider = model_provider;
            set_test_compact_prompt(config);
            config.model_auto_compact_token_limit = Some(200);
        })
        .build(&server)
        .await
        .expect("build codex")
        .codex;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "USER_ONE".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .expect("submit first user");
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "USER_TWO".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .expect("submit second user");
    let error_message = wait_for_event_match(&codex, |event| match event {
        EventMsg::Error(err) => Some(err.message.clone()),
        _ => None,
    })
    .await;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = request_log.requests();
    assert!(
        requests.len() >= 2,
        "expected first turn and at least one compaction request"
    );

    insta::assert_snapshot!(
        "pre_turn_compaction_context_window_exceeded_shapes",
        format_labeled_requests_snapshot(
            "Pre-turn auto-compaction context-window failure: compaction request includes the incoming user message and the turn errors.",
            &[(
                "Local Compaction Request (Incoming User Included)",
                &requests[1]
            ),]
        )
    );

    assert!(
        error_message.contains("ran out of room in the model's context window"),
        "expected context window exceeded message, got {error_message}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pre_turn_local_compaction_trim_retries_keep_incoming_items() {
    skip_if_no_network!();

    let server = start_mock_server().await;
    let compact_failed = sse_failed(
        "compact-failed",
        "context_length_exceeded",
        CONTEXT_LIMIT_MESSAGE,
    );
    let request_log = mount_sse_sequence(
        &server,
        vec![
            compact_failed.clone(),
            compact_failed.clone(),
            compact_failed.clone(),
            compact_failed,
        ],
    )
    .await;

    let model_provider = non_openai_model_provider(&server);
    let codex = test_codex()
        .with_config(move |config| {
            config.model_provider = model_provider;
            set_test_compact_prompt(config);
            config.model_auto_compact_token_limit = Some(1);
        })
        .build(&server)
        .await
        .expect("build codex")
        .codex;

    let incoming_text = "PRETURN_INCOMING_USER";
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: incoming_text.to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .expect("submit user input");

    let error_message = wait_for_event_match(&codex, |event| match event {
        EventMsg::Error(err) => Some(err.message.clone()),
        _ => None,
    })
    .await;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    assert!(
        error_message.contains(
            "Incoming user message and/or turn context is too large to fit in context window"
        ),
        "expected oversized incoming-items error, got {error_message}"
    );

    let compact_attempts = request_log.requests();
    assert_eq!(
        compact_attempts.len(),
        4,
        "expected pre-turn compaction to retry while trimming seeded history only"
    );
    for (index, request) in compact_attempts.iter().enumerate() {
        let body = request.body_json().to_string();
        assert!(
            body_contains_text(&body, SUMMARIZATION_PROMPT),
            "request {index} should be a compaction attempt"
        );
        assert!(
            request
                .message_input_texts("user")
                .iter()
                .any(|text| text == incoming_text),
            "request {index} dropped incoming user text during trim retries"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pre_turn_compaction_failure_persists_context_updates_for_next_turn() {
    skip_if_no_network!();

    let server = start_mock_server().await;
    let compact_failed = sse_failed(
        "compact-failed",
        "context_length_exceeded",
        CONTEXT_LIMIT_MESSAGE,
    );
    let third_turn = sse(vec![
        ev_assistant_message("m3", FINAL_REPLY),
        ev_completed_with_tokens("r3", 80),
    ]);
    let request_log = mount_sse_sequence(
        &server,
        vec![
            compact_failed.clone(),
            compact_failed.clone(),
            compact_failed.clone(),
            compact_failed,
            third_turn,
        ],
    )
    .await;

    let mut model_provider = non_openai_model_provider(&server);
    model_provider.stream_max_retries = Some(0);
    let codex = test_codex()
        .with_config(move |config| {
            config.model_provider = model_provider;
            set_test_compact_prompt(config);
            config.model_auto_compact_token_limit = Some(300);
        })
        .build(&server)
        .await
        .expect("build codex")
        .codex;

    // Seed `previous_context` without adding a user turn to history.
    codex
        .submit(Op::UserTurn {
            items: Vec::new(),
            final_output_json_schema: None,
            cwd: PathBuf::from("."),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            model: "gpt-5.2-codex".to_string(),
            effort: None,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await
        .expect("submit empty settings-only turn");

    let oversized_input = "X".repeat(2_000);
    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: oversized_input,
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: PathBuf::from("."),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: "gpt-5.2-codex".to_string(),
            effort: None,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await
        .expect("submit oversized turn that triggers pre-turn compaction failure");
    let error_message = wait_for_event_match(&codex, |event| match event {
        EventMsg::Error(err) => Some(err.message.clone()),
        _ => None,
    })
    .await;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
    assert!(
        error_message.contains(
            "Incoming user message and/or turn context is too large to fit in context window"
        ),
        "expected oversized incoming-items error, got {error_message}"
    );

    let follow_up_text = "after failed pre-turn compact";
    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: follow_up_text.to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: PathBuf::from("."),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: "gpt-5.2-codex".to_string(),
            effort: None,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await
        .expect("submit follow-up turn");
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = request_log.requests();
    assert_eq!(
        requests.len(),
        5,
        "expected four failed pre-turn compaction attempts and one follow-up model request"
    );

    let follow_up_request = requests.last().expect("missing follow-up request");
    let follow_up_developer_texts = follow_up_request.message_input_texts("developer");
    assert!(
        follow_up_developer_texts
            .iter()
            .any(|text| text.contains("sandbox_mode` is `danger-full-access`")),
        "expected danger-full-access permissions update in follow-up turn after failed pre-turn compaction: {follow_up_developer_texts:?}"
    );
    assert!(
        follow_up_request
            .message_input_texts("user")
            .iter()
            .any(|text| text == follow_up_text),
        "expected follow-up request to include follow-up user message"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn snapshot_request_shape_manual_compact_without_previous_user_messages() {
    let server = start_mock_server().await;

    let follow_up_turn = sse(vec![
        ev_assistant_message("m1", FINAL_REPLY),
        ev_completed_with_tokens("r1", 80),
    ]);
    let request_log = mount_sse_once(&server, follow_up_turn).await;

    let model_provider = non_openai_model_provider(&server);
    let codex = test_codex()
        .with_config(move |config| {
            config.model_provider = model_provider;
            set_test_compact_prompt(config);
        })
        .build(&server)
        .await
        .expect("build codex")
        .codex;

    codex.submit(Op::Compact).await.expect("run /compact");
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "AFTER_MANUAL_EMPTY_COMPACT".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .expect("submit follow-up user input");
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = request_log.requests();
    assert_eq!(
        requests.len(),
        1,
        "manual /compact with no prior user should be a no-op; only the follow-up turn should hit /responses"
    );

    insta::assert_snapshot!(
        "manual_compact_without_prev_user_shapes",
        format_labeled_requests_snapshot(
            "Manual /compact with no prior user turn is a no-op; follow-up turn carries canonical context and the new user message.",
            &[("Local Post-Compaction History Layout", &requests[0]),]
        )
    );
}
