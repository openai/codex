#![allow(clippy::expect_used)]

use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use codex_core::CodexAuth;
use codex_core::compact::SUMMARY_PREFIX;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ItemCompletedEvent;
use codex_core::protocol::ItemStartedEvent;
use codex_core::protocol::Op;
use codex_core::protocol::RolloutItem;
use codex_core::protocol::RolloutLine;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::user_input::UserInput;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::context_snapshot::ContextSnapshotRenderMode;
use core_test_support::responses;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use wiremock::ResponseTemplate;

fn approx_token_count(text: &str) -> i64 {
    i64::try_from(text.len().saturating_add(3) / 4).unwrap_or(i64::MAX)
}

fn estimate_compact_input_tokens(request: &responses::ResponsesRequest) -> i64 {
    request.input().into_iter().fold(0i64, |acc, item| {
        acc.saturating_add(approx_token_count(&item.to_string()))
    })
}

fn estimate_compact_payload_tokens(request: &responses::ResponsesRequest) -> i64 {
    estimate_compact_input_tokens(request)
        .saturating_add(approx_token_count(&request.instructions_text()))
}

const DUMMY_FUNCTION_NAME: &str = "test_tool";
const PRETURN_CONTEXT_DIFF_CWD_MARKER: &str = "PRETURN_CONTEXT_DIFF_CWD";
const PRETURN_CONTEXT_DIFF_CWD: &str = "/tmp/PRETURN_CONTEXT_DIFF_CWD";

fn summary_with_prefix(summary: &str) -> String {
    format!("{SUMMARY_PREFIX}\n{summary}")
}

fn user_message_item(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: text.to_string(),
        }],
        end_turn: None,
        phase: None,
    }
}

fn context_snapshot_options() -> ContextSnapshotOptions {
    ContextSnapshotOptions::default().render_mode(ContextSnapshotRenderMode::KindOnly)
}

fn format_labeled_requests_snapshot(
    scenario: &str,
    sections: &[(&str, &responses::ResponsesRequest)],
) -> String {
    context_snapshot::format_labeled_requests_snapshot(
        scenario,
        sections,
        &context_snapshot_options(),
    )
}

fn json_fragment(text: &str) -> String {
    serde_json::to_string(text)
        .expect("serialize text to JSON")
        .trim_matches('"')
        .to_string()
}

fn request_contains_text(request: &responses::ResponsesRequest, text: &str) -> bool {
    request
        .body_json()
        .to_string()
        .contains(&json_fragment(text))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_compact_replaces_history_for_followups() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = TestCodexHarness::with_builder(
        test_codex().with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing()),
    )
    .await?;
    let codex = harness.test().codex.clone();

    let responses_mock = responses::mount_sse_sequence(
        harness.server(),
        vec![
            responses::sse(vec![
                responses::ev_assistant_message("m1", "FIRST_REMOTE_REPLY"),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("m2", "AFTER_COMPACT_REPLY"),
                responses::ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let compacted_history = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "REMOTE_COMPACTED_SUMMARY".to_string(),
            }],
            end_turn: None,
            phase: None,
        },
        ResponseItem::Compaction {
            encrypted_content: "ENCRYPTED_COMPACTION_SUMMARY".to_string(),
        },
    ];
    let compact_mock = responses::mount_compact_json_once(
        harness.server(),
        serde_json::json!({ "output": compacted_history.clone() }),
    )
    .await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello remote compact".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex.submit(Op::Compact).await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "after compact".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let compact_request = compact_mock.single_request();
    assert_eq!(compact_request.path(), "/v1/responses/compact");
    assert_eq!(
        compact_request.header("chatgpt-account-id").as_deref(),
        Some("account_id")
    );
    assert_eq!(
        compact_request.header("authorization").as_deref(),
        Some("Bearer Access Token")
    );
    let compact_body = compact_request.body_json();
    assert_eq!(
        compact_body.get("model").and_then(|v| v.as_str()),
        Some(harness.test().session_configured.model.as_str())
    );
    let compact_body_text = compact_body.to_string();
    assert!(
        compact_body_text.contains("hello remote compact"),
        "expected compact request to include user history"
    );
    assert!(
        compact_body_text.contains("FIRST_REMOTE_REPLY"),
        "expected compact request to include assistant history"
    );

    let follow_up_body = responses_mock
        .requests()
        .last()
        .expect("follow-up request missing")
        .body_json()
        .to_string();
    assert!(
        follow_up_body.contains("REMOTE_COMPACTED_SUMMARY"),
        "expected follow-up request to use compacted history"
    );
    assert!(
        follow_up_body.contains("ENCRYPTED_COMPACTION_SUMMARY"),
        "expected follow-up request to include compaction summary item"
    );
    assert!(
        !follow_up_body.contains("FIRST_REMOTE_REPLY"),
        "expected follow-up request to drop pre-compaction assistant messages"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_compact_runs_automatically() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = TestCodexHarness::with_builder(
        test_codex().with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing()),
    )
    .await?;
    let codex = harness.test().codex.clone();

    mount_sse_once(
        harness.server(),
        sse(vec![
            responses::ev_shell_command_call("m1", "echo 'hi'"),
            responses::ev_completed_with_tokens("resp-1", 100000000), // over token limit
        ]),
    )
    .await;
    let responses_mock = mount_sse_once(
        harness.server(),
        responses::sse(vec![
            responses::ev_assistant_message("m2", "AFTER_COMPACT_REPLY"),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    let compacted_history = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "REMOTE_COMPACTED_SUMMARY".to_string(),
            }],
            end_turn: None,
            phase: None,
        },
        ResponseItem::Compaction {
            encrypted_content: "ENCRYPTED_COMPACTION_SUMMARY".to_string(),
        },
    ];
    let compact_mock = responses::mount_compact_json_once(
        harness.server(),
        serde_json::json!({ "output": compacted_history.clone() }),
    )
    .await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello remote compact".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    let message = wait_for_event_match(&codex, |event| match event {
        EventMsg::ContextCompacted(_) => Some(true),
        _ => None,
    })
    .await;
    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    assert!(message);
    assert_eq!(compact_mock.requests().len(), 1);
    let follow_up_body = responses_mock.single_request().body_json().to_string();
    assert!(follow_up_body.contains("REMOTE_COMPACTED_SUMMARY"));
    assert!(follow_up_body.contains("ENCRYPTED_COMPACTION_SUMMARY"));

    Ok(())
}

#[cfg_attr(target_os = "windows", ignore)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_compact_trims_function_call_history_to_fit_context_window() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let first_user_message = "turn with retained shell call";
    let second_user_message = "turn with trimmed shell call";
    let retained_call_id = "retained-call";
    let trimmed_call_id = "trimmed-call";
    let retained_command = "echo retained-shell-output";
    let trimmed_command = "yes x | head -n 3000";

    let harness = TestCodexHarness::with_builder(
        test_codex()
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_config(|config| {
                config.model_context_window = Some(2_000);
                config.model_auto_compact_token_limit = Some(200_000);
            }),
    )
    .await?;
    let codex = harness.test().codex.clone();

    responses::mount_sse_sequence(
        harness.server(),
        vec![
            sse(vec![
                responses::ev_shell_command_call(retained_call_id, retained_command),
                responses::ev_completed("retained-call-response"),
            ]),
            sse(vec![
                responses::ev_assistant_message("retained-assistant", "retained complete"),
                responses::ev_completed("retained-final-response"),
            ]),
            sse(vec![
                responses::ev_shell_command_call(trimmed_call_id, trimmed_command),
                responses::ev_completed("trimmed-call-response"),
            ]),
        ],
    )
    .await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: first_user_message.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: second_user_message.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let compact_mock =
        responses::mount_compact_json_once(harness.server(), serde_json::json!({ "output": [] }))
            .await;

    codex.submit(Op::Compact).await?;
    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let compact_request = compact_mock.single_request();
    let user_messages = compact_request.message_input_texts("user");
    assert!(
        user_messages
            .iter()
            .any(|message| message == first_user_message),
        "expected compact request to retain earlier user history"
    );
    assert!(
        user_messages
            .iter()
            .any(|message| message == second_user_message),
        "expected compact request to retain the user boundary message"
    );

    assert!(
        compact_request.has_function_call(retained_call_id)
            && compact_request
                .function_call_output_text(retained_call_id)
                .is_some(),
        "expected compact request to keep the older function call/result pair"
    );
    assert!(
        !compact_request.has_function_call(trimmed_call_id)
            && compact_request
                .function_call_output_text(trimmed_call_id)
                .is_none(),
        "expected compact request to drop the trailing function call/result pair past the boundary"
    );

    assert_eq!(
        compact_request.inputs_of_type("function_call").len(),
        1,
        "expected exactly one function call after trimming"
    );
    assert_eq!(
        compact_request.inputs_of_type("function_call_output").len(),
        1,
        "expected exactly one function call output after trimming"
    );

    Ok(())
}

#[cfg_attr(target_os = "windows", ignore)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auto_remote_compact_trims_function_call_history_to_fit_context_window() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let first_user_message = "turn with retained shell call";
    let second_user_message = "turn with trimmed shell call";
    let retained_call_id = "retained-call";
    let trimmed_call_id = "trimmed-call";
    let retained_command = "echo retained-shell-output";
    let trimmed_command = "yes x | head -n 3000";
    let harness = TestCodexHarness::with_builder(
        test_codex()
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_config(|config| {
                config.model_context_window = Some(2_000);
                config.model_auto_compact_token_limit = Some(200_000);
            }),
    )
    .await?;
    let codex = harness.test().codex.clone();

    responses::mount_sse_sequence(
        harness.server(),
        vec![
            sse(vec![
                responses::ev_shell_command_call(retained_call_id, retained_command),
                responses::ev_completed_with_tokens("retained-call-response", 100),
            ]),
            sse(vec![
                responses::ev_assistant_message("retained-assistant", "retained complete"),
                responses::ev_completed("retained-final-response"),
            ]),
            sse(vec![
                responses::ev_shell_command_call(trimmed_call_id, trimmed_command),
                responses::ev_completed_with_tokens("trimmed-call-response", 100),
            ]),
            sse(vec![responses::ev_completed_with_tokens(
                "trimmed-final-response",
                500_000,
            )]),
        ],
    )
    .await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: first_user_message.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: second_user_message.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let compact_mock =
        responses::mount_compact_json_once(harness.server(), serde_json::json!({ "output": [] }))
            .await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "turn that triggers auto compact".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    assert_eq!(
        compact_mock.requests().len(),
        1,
        "expected exactly one remote compact request"
    );

    let compact_request = compact_mock.single_request();
    let user_messages = compact_request.message_input_texts("user");
    assert!(
        user_messages
            .iter()
            .any(|message| message == first_user_message),
        "expected compact request to retain earlier user history"
    );
    assert!(
        user_messages
            .iter()
            .any(|message| message == second_user_message),
        "expected compact request to retain the user boundary message"
    );

    assert!(
        compact_request.has_function_call(retained_call_id)
            && compact_request
                .function_call_output_text(retained_call_id)
                .is_some(),
        "expected compact request to keep the older function call/result pair"
    );
    assert!(
        !compact_request.has_function_call(trimmed_call_id)
            && compact_request
                .function_call_output_text(trimmed_call_id)
                .is_none(),
        "expected compact request to drop the trailing function call/result pair past the boundary"
    );

    assert_eq!(
        compact_request.inputs_of_type("function_call").len(),
        1,
        "expected exactly one function call after trimming"
    );
    assert_eq!(
        compact_request.inputs_of_type("function_call_output").len(),
        1,
        "expected exactly one function call output after trimming"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auto_remote_compact_failure_stops_agent_loop() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = TestCodexHarness::with_builder(
        test_codex()
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_config(|config| {
                config.model_auto_compact_token_limit = Some(120);
            }),
    )
    .await?;
    let codex = harness.test().codex.clone();

    mount_sse_once(
        harness.server(),
        sse(vec![
            responses::ev_assistant_message("initial-assistant", "initial turn complete"),
            responses::ev_completed_with_tokens("initial-response", 500_000),
        ]),
    )
    .await;

    let first_compact_mock = responses::mount_compact_json_once(
        harness.server(),
        serde_json::json!({ "output": "invalid compact payload shape" }),
    )
    .await;
    let post_compact_turn_mock = mount_sse_once(
        harness.server(),
        sse(vec![
            responses::ev_assistant_message("post-compact-assistant", "should not run"),
            responses::ev_completed("post-compact-response"),
        ]),
    )
    .await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "turn that exceeds token threshold".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "turn that triggers auto compact".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    let error_message = wait_for_event_match(&codex, |event| match event {
        EventMsg::Error(err) => Some(err.message.clone()),
        _ => None,
    })
    .await;
    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    assert!(
        !error_message.contains(
            "Incoming user message and/or turn context is too large to fit in context window"
        ),
        "non-context compaction failures should surface real error messages, got {error_message}"
    );
    assert!(
        error_message.contains("invalid compact payload shape")
            || error_message.contains("invalid type: string"),
        "expected remote compact parse failure to surface, got {error_message}"
    );
    assert_eq!(
        first_compact_mock.requests().len(),
        1,
        "expected first remote compact attempt with incoming items"
    );
    assert!(
        post_compact_turn_mock.requests().is_empty(),
        "expected agent loop to stop after compaction failure"
    );

    Ok(())
}

#[cfg_attr(target_os = "windows", ignore)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_compact_trim_estimate_uses_session_base_instructions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let first_user_message = "turn with baseline shell call";
    let second_user_message = "turn with trailing shell call";
    let baseline_retained_call_id = "baseline-retained-call";
    let baseline_trailing_call_id = "baseline-trailing-call";
    let override_retained_call_id = "override-retained-call";
    let override_trailing_call_id = "override-trailing-call";
    let retained_command = "printf retained-shell-output";
    let trailing_command = "printf trailing-shell-output";

    let baseline_harness = TestCodexHarness::with_builder(
        test_codex()
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_config(|config| {
                config.model_context_window = Some(200_000);
            }),
    )
    .await?;
    let baseline_codex = baseline_harness.test().codex.clone();

    responses::mount_sse_sequence(
        baseline_harness.server(),
        vec![
            sse(vec![
                responses::ev_shell_command_call(baseline_retained_call_id, retained_command),
                responses::ev_completed("baseline-retained-call-response"),
            ]),
            sse(vec![
                responses::ev_assistant_message("baseline-retained-assistant", "retained complete"),
                responses::ev_completed("baseline-retained-final-response"),
            ]),
            sse(vec![
                responses::ev_shell_command_call(baseline_trailing_call_id, trailing_command),
                responses::ev_completed("baseline-trailing-call-response"),
            ]),
            sse(vec![responses::ev_completed(
                "baseline-trailing-final-response",
            )]),
        ],
    )
    .await;

    baseline_codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: first_user_message.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&baseline_codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    baseline_codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: second_user_message.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&baseline_codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let baseline_compact_mock = responses::mount_compact_json_once(
        baseline_harness.server(),
        serde_json::json!({ "output": [] }),
    )
    .await;

    baseline_codex.submit(Op::Compact).await?;
    wait_for_event(&baseline_codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let baseline_compact_request = baseline_compact_mock.single_request();
    assert!(
        baseline_compact_request.has_function_call(baseline_retained_call_id),
        "expected baseline compact request to retain older function call history"
    );
    assert!(
        baseline_compact_request.has_function_call(baseline_trailing_call_id),
        "expected baseline compact request to retain trailing function call history"
    );

    let baseline_input_tokens = estimate_compact_input_tokens(&baseline_compact_request);
    let baseline_payload_tokens = estimate_compact_payload_tokens(&baseline_compact_request);

    let override_base_instructions =
        format!("REMOTE_BASE_INSTRUCTIONS_OVERRIDE {}", "x".repeat(120_000));
    let override_context_window = baseline_payload_tokens.saturating_add(1_000);
    let pretrim_override_estimate =
        baseline_input_tokens.saturating_add(approx_token_count(&override_base_instructions));
    assert!(
        pretrim_override_estimate > override_context_window,
        "expected override instructions to push pre-trim estimate past the context window"
    );

    let override_harness = TestCodexHarness::with_builder(
        test_codex()
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_config({
                let override_base_instructions = override_base_instructions.clone();
                move |config| {
                    config.model_context_window = Some(override_context_window);
                    config.base_instructions = Some(override_base_instructions);
                }
            }),
    )
    .await?;
    let override_codex = override_harness.test().codex.clone();

    responses::mount_sse_sequence(
        override_harness.server(),
        vec![
            sse(vec![
                responses::ev_shell_command_call(override_retained_call_id, retained_command),
                responses::ev_completed("override-retained-call-response"),
            ]),
            sse(vec![
                responses::ev_assistant_message("override-retained-assistant", "retained complete"),
                responses::ev_completed("override-retained-final-response"),
            ]),
            sse(vec![
                responses::ev_shell_command_call(override_trailing_call_id, trailing_command),
                responses::ev_completed("override-trailing-call-response"),
            ]),
            sse(vec![responses::ev_completed(
                "override-trailing-final-response",
            )]),
        ],
    )
    .await;

    override_codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: first_user_message.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&override_codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    override_codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: second_user_message.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&override_codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let override_compact_mock = responses::mount_compact_json_once(
        override_harness.server(),
        serde_json::json!({ "output": [] }),
    )
    .await;

    override_codex.submit(Op::Compact).await?;
    wait_for_event(&override_codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let override_compact_request = override_compact_mock.single_request();
    assert_eq!(
        override_compact_request.instructions_text(),
        override_base_instructions
    );
    assert!(
        override_compact_request.has_function_call(override_retained_call_id),
        "expected remote compact request to preserve older function call history"
    );
    assert!(
        !override_compact_request.has_function_call(override_trailing_call_id),
        "expected remote compact request to trim trailing function call history with override instructions"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_manual_compact_emits_context_compaction_items() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = TestCodexHarness::with_builder(
        test_codex().with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing()),
    )
    .await?;
    let codex = harness.test().codex.clone();

    mount_sse_once(
        harness.server(),
        sse(vec![
            responses::ev_assistant_message("m1", "REMOTE_REPLY"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;

    let compacted_history = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "REMOTE_COMPACTED_SUMMARY".to_string(),
            }],
            end_turn: None,
            phase: None,
        },
        ResponseItem::Compaction {
            encrypted_content: "ENCRYPTED_COMPACTION_SUMMARY".to_string(),
        },
    ];
    let compact_mock = responses::mount_compact_json_once(
        harness.server(),
        serde_json::json!({ "output": compacted_history.clone() }),
    )
    .await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "manual remote compact".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    codex.submit(Op::Compact).await?;

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
    assert_eq!(compact_mock.requests().len(), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_manual_compact_failure_emits_task_error_event() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = TestCodexHarness::with_builder(
        test_codex().with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing()),
    )
    .await?;
    let codex = harness.test().codex.clone();

    mount_sse_once(
        harness.server(),
        sse(vec![
            responses::ev_assistant_message("m1", "REMOTE_REPLY"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;

    let compact_mock = responses::mount_compact_json_once(
        harness.server(),
        serde_json::json!({ "output": "invalid compact payload shape" }),
    )
    .await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "manual remote compact".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    codex.submit(Op::Compact).await?;

    let error_message = wait_for_event_match(&codex, |event| match event {
        EventMsg::Error(err) => Some(err.message.clone()),
        _ => None,
    })
    .await;
    assert!(
        error_message.contains("Error running remote compact task"),
        "expected remote compact task error prefix, got {error_message}"
    );
    assert!(
        error_message.contains("invalid compact payload shape")
            || error_message.contains("invalid type: string"),
        "expected invalid compact payload details, got {error_message}"
    );
    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    assert_eq!(compact_mock.requests().len(), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// TODO(ccunningham): Re-enable after the follow-up compaction behavior PR lands.
// Current main behavior for rollout replacement-history persistence is known-incorrect.
#[ignore = "behavior change covered in follow-up compaction PR"]
async fn remote_compact_persists_replacement_history_in_rollout() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = TestCodexHarness::with_builder(
        test_codex().with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing()),
    )
    .await?;
    let codex = harness.test().codex.clone();
    let rollout_path = harness
        .test()
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path");

    let responses_mock = responses::mount_sse_once(
        harness.server(),
        responses::sse(vec![
            responses::ev_assistant_message("m1", "COMPACT_BASELINE_REPLY"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;

    let compacted_history = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "COMPACTED_USER_SUMMARY".to_string(),
            }],
            end_turn: None,
            phase: None,
        },
        ResponseItem::Compaction {
            encrypted_content: "ENCRYPTED_COMPACTION_SUMMARY".to_string(),
        },
        ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "COMPACTED_ASSISTANT_NOTE".to_string(),
            }],
            end_turn: None,
            phase: None,
        },
    ];
    let compact_mock = responses::mount_compact_json_once(
        harness.server(),
        serde_json::json!({ "output": compacted_history.clone() }),
    )
    .await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "needs compaction".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex.submit(Op::Compact).await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex.submit(Op::Shutdown).await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ShutdownComplete)).await;

    assert_eq!(responses_mock.requests().len(), 1);
    assert_eq!(compact_mock.requests().len(), 1);

    let rollout_text = fs::read_to_string(&rollout_path)?;
    let mut saw_compacted_history = false;
    for line in rollout_text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
    {
        let Ok(entry) = serde_json::from_str::<RolloutLine>(line) else {
            continue;
        };
        if let RolloutItem::Compacted(compacted) = entry.item
            && compacted.message.is_empty()
            && let Some(replacement_history) = compacted.replacement_history.as_ref()
        {
            let has_compacted_user_summary = replacement_history.iter().any(|item| {
                matches!(
                    item,
                    ResponseItem::Message { role, content, .. }
                        if role == "user"
                            && content.iter().any(|part| matches!(
                                part,
                                ContentItem::InputText { text } if text == "COMPACTED_USER_SUMMARY"
                            ))
                )
            });
            let has_compaction_item = replacement_history.iter().any(|item| {
                matches!(
                    item,
                    ResponseItem::Compaction { encrypted_content }
                        if encrypted_content == "ENCRYPTED_COMPACTION_SUMMARY"
                )
            });
            let has_compacted_assistant_note = replacement_history.iter().any(|item| {
                matches!(
                    item,
                    ResponseItem::Message { role, content, .. }
                        if role == "assistant"
                            && content.iter().any(|part| matches!(
                                part,
                                ContentItem::OutputText { text } if text == "COMPACTED_ASSISTANT_NOTE"
                            ))
                )
            });
            let has_permissions_developer_message = replacement_history.iter().any(|item| {
                matches!(
                    item,
                    ResponseItem::Message { role, content, .. }
                        if role == "developer"
                            && content.iter().any(|part| matches!(
                                part,
                                ContentItem::InputText { text }
                                    if text.contains("<permissions instructions>")
                            ))
                )
            });

            if has_compacted_user_summary && has_compaction_item && has_compacted_assistant_note {
                assert!(
                    !has_permissions_developer_message,
                    "manual remote compact rollout replacement history should not inject permissions context"
                );
                saw_compacted_history = true;
                break;
            }
        }
    }

    assert!(
        saw_compacted_history,
        "expected rollout to persist remote compaction history"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_compact_and_resume_refresh_stale_developer_instructions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = wiremock::MockServer::start().await;
    let stale_developer_message = "STALE_DEVELOPER_INSTRUCTIONS_SHOULD_BE_REMOVED";

    let mut start_builder =
        test_codex().with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let initial = start_builder.build(&server).await?;
    let home = initial.home.clone();
    let rollout_path = initial
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path");

    let responses_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_assistant_message("m1", "BASELINE_REPLY"),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("m2", "AFTER_COMPACT_REPLY"),
                responses::ev_completed("resp-2"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("m3", "AFTER_RESUME_REPLY"),
                responses::ev_completed("resp-3"),
            ]),
        ],
    )
    .await;

    let compacted_history = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "REMOTE_COMPACTED_SUMMARY".to_string(),
            }],
            end_turn: None,
            phase: None,
        },
        ResponseItem::Message {
            id: None,
            role: "developer".to_string(),
            content: vec![ContentItem::InputText {
                text: stale_developer_message.to_string(),
            }],
            end_turn: None,
            phase: None,
        },
        ResponseItem::Compaction {
            encrypted_content: "ENCRYPTED_COMPACTION_SUMMARY".to_string(),
        },
    ];
    let compact_mock = responses::mount_compact_json_once(
        &server,
        serde_json::json!({ "output": compacted_history }),
    )
    .await;

    initial
        .codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "start remote compact flow".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&initial.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    initial.codex.submit(Op::Compact).await?;
    wait_for_event(&initial.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    initial
        .codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "after compact in same session".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&initial.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    initial.codex.submit(Op::Shutdown).await?;
    wait_for_event(&initial.codex, |ev| {
        matches!(ev, EventMsg::ShutdownComplete)
    })
    .await;

    let mut resume_builder =
        test_codex().with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let resumed = resume_builder.resume(&server, home, rollout_path).await?;

    resumed
        .codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "after resume".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&resumed.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    assert_eq!(compact_mock.requests().len(), 1);
    let requests = responses_mock.requests();
    assert_eq!(requests.len(), 3, "expected three model requests");

    let after_compact_request = &requests[1];
    let after_resume_request = &requests[2];

    let after_compact_body = after_compact_request.body_json().to_string();
    assert!(
        !after_compact_body.contains(stale_developer_message),
        "stale developer instructions should be removed immediately after compaction"
    );
    assert!(
        after_compact_body.contains("<permissions instructions>"),
        "fresh developer instructions should be present after compaction"
    );
    assert!(
        after_compact_body.contains("REMOTE_COMPACTED_SUMMARY"),
        "compacted summary should be present after compaction"
    );

    let after_resume_body = after_resume_request.body_json().to_string();
    assert!(
        !after_resume_body.contains(stale_developer_message),
        "stale developer instructions should be removed after resume"
    );
    assert!(
        after_resume_body.contains("<permissions instructions>"),
        "fresh developer instructions should be present after resume"
    );
    assert!(
        after_resume_body.contains("REMOTE_COMPACTED_SUMMARY"),
        "compacted summary should persist after resume"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_compact_refreshes_stale_developer_instructions_without_resume() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = wiremock::MockServer::start().await;
    let stale_developer_message = "STALE_DEVELOPER_INSTRUCTIONS_SHOULD_BE_REMOVED";

    let mut builder = test_codex().with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let test = builder.build(&server).await?;

    let responses_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_assistant_message("m1", "BASELINE_REPLY"),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("m2", "AFTER_COMPACT_REPLY"),
                responses::ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let compacted_history = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "REMOTE_COMPACTED_SUMMARY".to_string(),
            }],
            end_turn: None,
            phase: None,
        },
        ResponseItem::Message {
            id: None,
            role: "developer".to_string(),
            content: vec![ContentItem::InputText {
                text: stale_developer_message.to_string(),
            }],
            end_turn: None,
            phase: None,
        },
        ResponseItem::Compaction {
            encrypted_content: "ENCRYPTED_COMPACTION_SUMMARY".to_string(),
        },
    ];
    let compact_mock = responses::mount_compact_json_once(
        &server,
        serde_json::json!({ "output": compacted_history }),
    )
    .await;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "start remote compact flow".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "after compact in same session".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    assert_eq!(compact_mock.requests().len(), 1);
    let requests = responses_mock.requests();
    assert_eq!(requests.len(), 2, "expected two model requests");

    let after_compact_body = requests[1].body_json().to_string();
    assert!(
        !after_compact_body.contains(stale_developer_message),
        "stale developer instructions should be removed immediately after compaction"
    );
    assert!(
        after_compact_body.contains("<permissions instructions>"),
        "fresh developer instructions should be present after compaction"
    );
    assert!(
        after_compact_body.contains("REMOTE_COMPACTED_SUMMARY"),
        "compacted summary should be present after compaction"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// TODO(ccunningham): Update once remote pre-turn compaction includes incoming user input.
async fn snapshot_request_shape_remote_pre_turn_compaction_including_incoming_user_message()
-> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = TestCodexHarness::with_builder(
        test_codex()
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_config(|config| {
                config.model_auto_compact_token_limit = Some(200);
            }),
    )
    .await?;
    let codex = harness.test().codex.clone();

    let responses_mock = responses::mount_sse_sequence(
        harness.server(),
        vec![
            responses::sse(vec![
                responses::ev_assistant_message("m1", "REMOTE_FIRST_REPLY"),
                responses::ev_completed_with_tokens("r1", 60),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("m2", "REMOTE_SECOND_REPLY"),
                responses::ev_completed_with_tokens("r2", 500),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("m3", "REMOTE_FINAL_REPLY"),
                responses::ev_completed_with_tokens("r3", 80),
            ]),
        ],
    )
    .await;

    let compacted_history = vec![
        user_message_item("USER_ONE"),
        user_message_item("USER_TWO"),
        user_message_item(&summary_with_prefix("REMOTE_PRE_TURN_SUMMARY")),
    ];
    let compact_mock = responses::mount_compact_json_once(
        harness.server(),
        serde_json::json!({ "output": compacted_history }),
    )
    .await;

    for user in ["USER_ONE", "USER_TWO", "USER_THREE"] {
        if user == "USER_THREE" {
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
                .await?;
        }
        codex
            .submit(Op::UserInput {
                items: vec![UserInput::Text {
                    text: user.to_string(),
                    text_elements: Vec::new(),
                }],
                final_output_json_schema: None,
            })
            .await?;
        wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
    }

    assert_eq!(compact_mock.requests().len(), 1);
    let requests = responses_mock.requests();
    assert_eq!(
        requests.len(),
        3,
        "expected user, user, and post-compact turn"
    );

    let compact_request = compact_mock.single_request();
    insta::assert_snapshot!(
        "remote_pre_turn_compaction_including_incoming_shapes",
        format_labeled_requests_snapshot(
            "Remote pre-turn auto-compaction with a context override emits the context diff in the compact request while excluding the incoming user message.",
            &[
                ("Remote Compaction Request", &compact_request),
                ("Remote Post-Compaction History Layout", &requests[2]),
            ]
        )
    );
    assert!(
        request_contains_text(&compact_request, PRETURN_CONTEXT_DIFF_CWD_MARKER),
        "expected remote compact request to include pre-turn context diff"
    );
    assert!(
        !request_contains_text(&compact_request, "USER_THREE"),
        "current behavior excludes incoming user message from remote pre-turn compaction input"
    );
    assert!(
        request_contains_text(
            &requests[2],
            &summary_with_prefix("REMOTE_PRE_TURN_SUMMARY")
        ),
        "post-compaction request should include remote summary text"
    );
    assert_eq!(
        requests[2]
            .message_input_texts("user")
            .iter()
            .filter(|text| text.as_str() == "USER_THREE")
            .count(),
        1,
        "post-compaction request should contain incoming user exactly once from runtime append"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// TODO(ccunningham): Update once remote pre-turn compaction failure path includes incoming
// user input.
async fn snapshot_request_shape_remote_pre_turn_compaction_failure_stops_without_retry()
-> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = TestCodexHarness::with_builder(
        test_codex()
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_config(|config| {
                config.model_auto_compact_token_limit = Some(200);
            }),
    )
    .await?;
    let codex = harness.test().codex.clone();

    let responses_mock = responses::mount_sse_sequence(
        harness.server(),
        vec![responses::sse(vec![
            responses::ev_assistant_message("m1", "REMOTE_FIRST_REPLY"),
            responses::ev_completed_with_tokens("r1", 500),
        ])],
    )
    .await;

    let first_compact_mock = responses::mount_compact_json_once(
        harness.server(),
        serde_json::json!({ "output": "invalid compact payload shape" }),
    )
    .await;
    let post_compact_turn_mock = responses::mount_sse_once(
        harness.server(),
        responses::sse(vec![
            responses::ev_assistant_message("m2", "REMOTE_POST_COMPACT_SHOULD_NOT_RUN"),
            responses::ev_completed_with_tokens("r2", 80),
        ]),
    )
    .await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "USER_ONE".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "USER_TWO".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    let error_message = wait_for_event_match(&codex, |event| match event {
        EventMsg::Error(err) => Some(err.message.clone()),
        _ => None,
    })
    .await;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    assert_eq!(first_compact_mock.requests().len(), 1);
    let requests = responses_mock.requests();
    assert_eq!(
        requests.len(),
        1,
        "expected no post-compaction follow-up turn request after compact failure"
    );
    assert!(
        post_compact_turn_mock.requests().is_empty(),
        "expected turn to stop after compaction failure"
    );

    let include_attempt_request = first_compact_mock.single_request();
    insta::assert_snapshot!(
        "remote_pre_turn_compaction_failure_shapes",
        format_labeled_requests_snapshot(
            "Remote pre-turn auto-compaction parse failure: compaction request excludes the incoming user message and the turn stops.",
            &[(
                "Remote Compaction Request (Incoming User Excluded)",
                &include_attempt_request
            ),]
        )
    );
    assert!(
        !request_contains_text(&include_attempt_request, "USER_TWO"),
        "current behavior excludes incoming user message from remote pre-turn compaction input"
    );
    assert!(
        error_message.contains("invalid compact payload shape")
            || error_message.contains("invalid type: string"),
        "expected compact parse failure to surface, got {error_message}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// TODO(ccunningham): Update once remote pre-turn compaction context-overflow handling includes
// incoming user input and emits richer oversized-input messaging.
async fn snapshot_request_shape_remote_pre_turn_compaction_context_window_exceeded() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = TestCodexHarness::with_builder(
        test_codex()
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_config(|config| {
                config.model_auto_compact_token_limit = Some(200);
            }),
    )
    .await?;
    let codex = harness.test().codex.clone();

    let responses_mock = responses::mount_sse_sequence(
        harness.server(),
        vec![responses::sse(vec![
            responses::ev_assistant_message("m1", "REMOTE_FIRST_REPLY"),
            responses::ev_completed_with_tokens("r1", 500),
        ])],
    )
    .await;

    let compact_mock = responses::mount_compact_response_once(
        harness.server(),
        ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "error": {
                "code": "context_length_exceeded",
                "message": "Your input exceeds the context window of this model. Please adjust your input and try again."
            }
        })),
    )
    .await;
    let post_compact_turn_mock = responses::mount_sse_once(
        harness.server(),
        responses::sse(vec![
            responses::ev_assistant_message("m2", "REMOTE_POST_COMPACT_SHOULD_NOT_RUN"),
            responses::ev_completed_with_tokens("r2", 80),
        ]),
    )
    .await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "USER_ONE".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "USER_TWO".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    let error_message = wait_for_event_match(&codex, |event| match event {
        EventMsg::Error(err) => Some(err.message.clone()),
        _ => None,
    })
    .await;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    assert_eq!(compact_mock.requests().len(), 1);
    let requests = responses_mock.requests();
    assert_eq!(
        requests.len(),
        1,
        "expected no post-compaction follow-up turn request after compact failure"
    );
    assert!(
        post_compact_turn_mock.requests().is_empty(),
        "expected turn to stop after compaction failure"
    );

    let include_attempt_request = compact_mock.single_request();
    insta::assert_snapshot!(
        "remote_pre_turn_compaction_context_window_exceeded_shapes",
        format_labeled_requests_snapshot(
            "Remote pre-turn auto-compaction context-window failure: compaction request excludes the incoming user message and the turn errors.",
            &[(
                "Remote Compaction Request (Incoming User Excluded)",
                &include_attempt_request
            ),]
        )
    );
    assert!(
        !request_contains_text(&include_attempt_request, "USER_TWO"),
        "current behavior excludes incoming user message from remote pre-turn compaction input"
    );
    assert!(
        error_message.to_lowercase().contains("context window"),
        "expected context window failure to surface, got {error_message}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn snapshot_request_shape_remote_mid_turn_continuation_compaction() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = TestCodexHarness::with_builder(
        test_codex()
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_config(|config| {
                config.model_auto_compact_token_limit = Some(200);
            }),
    )
    .await?;
    let codex = harness.test().codex.clone();

    let responses_mock = responses::mount_sse_sequence(
        harness.server(),
        vec![
            responses::sse(vec![
                responses::ev_function_call("call-remote-mid-turn", DUMMY_FUNCTION_NAME, "{}"),
                responses::ev_completed_with_tokens("r1", 500),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("m2", "REMOTE_MID_TURN_FINAL_REPLY"),
                responses::ev_completed_with_tokens("r2", 80),
            ]),
        ],
    )
    .await;

    let compacted_history = vec![
        user_message_item("USER_ONE"),
        user_message_item(&summary_with_prefix("REMOTE_MID_TURN_SUMMARY")),
    ];
    let compact_mock = responses::mount_compact_json_once(
        harness.server(),
        serde_json::json!({ "output": compacted_history }),
    )
    .await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "USER_ONE".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    assert_eq!(compact_mock.requests().len(), 1);
    let requests = responses_mock.requests();
    assert_eq!(
        requests.len(),
        2,
        "expected initial and post-compact requests"
    );

    let compact_request = compact_mock.single_request();
    insta::assert_snapshot!(
        "remote_mid_turn_compaction_shapes",
        format_labeled_requests_snapshot(
            "Remote mid-turn continuation compaction after tool output: compact request includes tool artifacts and follow-up request includes the summary.",
            &[
                ("Remote Compaction Request", &compact_request),
                ("Remote Post-Compaction History Layout", &requests[1]),
            ]
        )
    );
    assert!(
        !compact_request
            .inputs_of_type("function_call_output")
            .is_empty(),
        "remote mid-turn compaction request should include function call output"
    );
    assert!(
        request_contains_text(
            &requests[1],
            &summary_with_prefix("REMOTE_MID_TURN_SUMMARY")
        ),
        "post-mid-turn request should include remote compaction summary text"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// TODO(ccunningham): Update once manual remote /compact with no prior user turn becomes a no-op.
async fn snapshot_request_shape_remote_manual_compact_without_previous_user_messages() -> Result<()>
{
    skip_if_no_network!(Ok(()));

    let harness = TestCodexHarness::with_builder(
        test_codex().with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing()),
    )
    .await?;
    let codex = harness.test().codex.clone();

    let responses_mock = responses::mount_sse_once(
        harness.server(),
        responses::sse(vec![
            responses::ev_assistant_message("m1", "REMOTE_MANUAL_EMPTY_FOLLOW_UP_REPLY"),
            responses::ev_completed_with_tokens("r1", 80),
        ]),
    )
    .await;

    let compact_mock =
        responses::mount_compact_json_once(harness.server(), serde_json::json!({ "output": [] }))
            .await;

    codex.submit(Op::Compact).await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "USER_ONE".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    assert_eq!(
        compact_mock.requests().len(),
        1,
        "current behavior still issues remote compaction for manual /compact without prior user"
    );
    let compact_request = compact_mock.single_request();
    let follow_up_request = responses_mock.single_request();
    insta::assert_snapshot!(
        "remote_manual_compact_without_prev_user_shapes",
        format_labeled_requests_snapshot(
            "Remote manual /compact with no prior user turn still issues a compact request; follow-up turn carries canonical context and new user message.",
            &[
                ("Remote Compaction Request", &compact_request),
                ("Remote Post-Compaction History Layout", &follow_up_request),
            ]
        )
    );
    assert!(
        !request_contains_text(
            &follow_up_request,
            &summary_with_prefix("REMOTE_MANUAL_EMPTY_SUMMARY"),
        ),
        "post-compact request should not include compact summary when remote compaction is skipped"
    );
    assert!(
        request_contains_text(&follow_up_request, "USER_ONE"),
        "post-compact request should include the submitted user message"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn snapshot_request_shape_remote_manual_compact_with_previous_user_messages() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = TestCodexHarness::with_builder(
        test_codex().with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing()),
    )
    .await?;
    let codex = harness.test().codex.clone();

    let responses_mock = responses::mount_sse_sequence(
        harness.server(),
        vec![
            responses::sse(vec![
                responses::ev_assistant_message("m1", "REMOTE_MANUAL_FIRST_REPLY"),
                responses::ev_completed_with_tokens("r1", 80),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("m2", "REMOTE_MANUAL_FOLLOW_UP_REPLY"),
                responses::ev_completed_with_tokens("r2", 80),
            ]),
        ],
    )
    .await;

    let compacted_history = vec![
        user_message_item("USER_ONE"),
        user_message_item(&summary_with_prefix("REMOTE_MANUAL_WITH_HISTORY_SUMMARY")),
    ];
    let compact_mock = responses::mount_compact_json_once(
        harness.server(),
        serde_json::json!({ "output": compacted_history }),
    )
    .await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "USER_ONE".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex.submit(Op::Compact).await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "USER_TWO".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    assert_eq!(compact_mock.requests().len(), 1);
    let requests = responses_mock.requests();
    assert_eq!(requests.len(), 2, "expected user and post-compact requests");

    let compact_request = compact_mock.single_request();
    insta::assert_snapshot!(
        "remote_manual_compact_with_history_shapes",
        format_labeled_requests_snapshot(
            "Remote manual /compact with prior user history compacts existing history and follow-up includes compact summary plus new user message.",
            &[
                ("Remote Compaction Request", &compact_request),
                ("Remote Post-Compaction History Layout", &requests[1]),
            ]
        )
    );
    assert!(
        request_contains_text(&compact_request, "USER_ONE"),
        "remote compaction request should include existing user history"
    );
    assert!(
        request_contains_text(&requests[1], "USER_TWO"),
        "post-compact request should include latest user message"
    );
    assert!(
        request_contains_text(
            &requests[1],
            &summary_with_prefix("REMOTE_MANUAL_WITH_HISTORY_SUMMARY"),
        ),
        "post-compact request should include remote compact summary text"
    );

    Ok(())
}
