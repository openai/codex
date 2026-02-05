#![allow(clippy::expect_used)]

use std::fs;

use anyhow::Result;
use codex_core::CodexAuth;
use codex_core::features::Feature;
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
use core_test_support::responses;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_compact_replaces_history_for_followups() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = TestCodexHarness::with_builder(
        test_codex()
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_config(|config| {
                config.features.enable(Feature::RemoteCompaction);
            }),
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
        test_codex()
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_config(|config| {
                config.features.enable(Feature::RemoteCompaction);
            }),
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
                config.features.enable(Feature::RemoteCompaction);
                config.model_context_window = Some(2_000);
            }),
    )
    .await?;
    let codex = harness.test().codex.clone();

    let response_log = responses::mount_sse_sequence(
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
            sse(vec![responses::ev_completed("trimmed-final-response")]),
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

    assert!(
        response_log
            .function_call_output_text(retained_call_id)
            .is_some(),
        "expected retained shell call to produce function_call_output before compaction"
    );
    assert!(
        response_log
            .function_call_output_text(trimmed_call_id)
            .is_some(),
        "expected trimmed shell call to produce function_call_output before compaction"
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
                config.features.enable(Feature::RemoteCompaction);
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
                    config.features.enable(Feature::RemoteCompaction);
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
        test_codex()
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_config(|config| {
                config.features.enable(Feature::RemoteCompaction);
            }),
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
async fn remote_compact_persists_replacement_history_in_rollout() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = TestCodexHarness::with_builder(
        test_codex()
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_config(|config| {
                config.features.enable(Feature::RemoteCompaction);
            }),
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
            && compacted.replacement_history.as_ref() == Some(&compacted_history)
        {
            saw_compacted_history = true;
            break;
        }
    }

    assert!(
        saw_compacted_history,
        "expected rollout to persist remote compaction history"
    );

    Ok(())
}
