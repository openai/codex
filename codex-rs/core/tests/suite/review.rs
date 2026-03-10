use codex_core::CodexThread;
use codex_core::REVIEW_PROMPT;
use codex_core::config::Config;
use codex_core::review_format::render_review_output_text;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::ENVIRONMENT_CONTEXT_OPEN_TAG;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExitedReviewModeEvent;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewCodeLocation;
use codex_protocol::protocol::ReviewFinding;
use codex_protocol::protocol::ReviewLineRange;
use codex_protocol::protocol::ReviewOutputEvent;
use codex_protocol::protocol::ReviewRequest;
use codex_protocol::protocol::ReviewTarget;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_protocol::user_input::UserInput;
use core_test_support::load_sse_fixture_with_id_from_str;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_reasoning_item;
use core_test_support::responses::ev_reasoning_item_added;
use core_test_support::responses::ev_reasoning_summary_text_delta;
use core_test_support::responses::ev_reasoning_text_delta;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::sse_failed;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::io::AsyncWriteExt as _;
use uuid::Uuid;
use wiremock::MockServer;

/// Verify that submitting `Op::Review` spawns a child task and emits
/// EnteredReviewMode -> ExitedReviewMode(None) -> TurnComplete
/// in that order when the model returns a structured review JSON payload.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_op_emits_lifecycle_and_review_output() {
    // Skip under Codex sandbox network restrictions.
    skip_if_no_network!();

    // Start mock Responses API server. Return a single assistant message whose
    // text is a JSON-encoded ReviewOutputEvent.
    let review_json = serde_json::json!({
        "findings": [
            {
                "title": "Prefer Stylize helpers",
                "body": "Use .dim()/.bold() chaining instead of manual Style where possible.",
                "confidence_score": 0.9,
                "priority": 1,
                "code_location": {
                    "absolute_file_path": "/tmp/file.rs",
                    "line_range": {"start": 10, "end": 20}
                }
            }
        ],
        "overall_correctness": "good",
        "overall_explanation": "All good with some improvements suggested.",
        "overall_confidence_score": 0.8
    })
    .to_string();
    let sse_template = r#"[
            {"type":"response.output_item.done", "item":{
                "type":"message", "role":"assistant",
                "content":[{"type":"output_text","text":__REVIEW__}]
            }},
            {"type":"response.completed", "response": {"id": "__ID__"}}
        ]"#;
    let review_json_escaped = serde_json::to_string(&review_json).unwrap();
    let sse_raw = sse_template.replace("__REVIEW__", &review_json_escaped);
    let (server, _request_log) = start_responses_server_with_sse(&sse_raw, 1).await;
    let codex_home = Arc::new(TempDir::new().unwrap());
    let codex = new_conversation_for_server(&server, codex_home.clone(), |_| {}).await;

    // Submit review request.
    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Please review my changes".to_string(),
                },
                user_facing_hint: None,
                validate_findings: false,
            },
        })
        .await
        .unwrap();

    // Verify lifecycle: Entered -> Exited(Some(review)) -> TurnComplete.
    let _entered = wait_for_event(&codex, |ev| matches!(ev, EventMsg::EnteredReviewMode(_))).await;
    let closed = wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExitedReviewMode(_))).await;
    let review = match closed {
        EventMsg::ExitedReviewMode(ev) => ev
            .review_output
            .expect("expected ExitedReviewMode with Some(review_output)"),
        other => panic!("expected ExitedReviewMode(..), got {other:?}"),
    };

    // Deep compare full structure using PartialEq (floats are f32 on both sides).
    let expected = ReviewOutputEvent {
        findings: vec![ReviewFinding {
            title: "Prefer Stylize helpers".to_string(),
            body: "Use .dim()/.bold() chaining instead of manual Style where possible.".to_string(),
            confidence_score: 0.9,
            priority: 1,
            code_location: ReviewCodeLocation {
                absolute_file_path: PathBuf::from("/tmp/file.rs"),
                line_range: ReviewLineRange { start: 10, end: 20 },
            },
        }],
        overall_correctness: "good".to_string(),
        overall_explanation: "All good with some improvements suggested.".to_string(),
        overall_confidence_score: 0.8,
    };
    assert_eq!(expected, review);
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    // Also verify that a user message with the header and a formatted finding
    // was recorded back in the parent session's rollout.
    let path = codex.rollout_path().expect("rollout path");
    let text = std::fs::read_to_string(&path).expect("read rollout file");

    let mut saw_header = false;
    let mut saw_finding_line = false;
    let expected_assistant_text = render_review_output_text(&expected);
    let mut saw_assistant_plain = false;
    let mut saw_assistant_xml = false;
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(line).expect("jsonl line");
        let rl: RolloutLine = serde_json::from_value(v).expect("rollout line");
        if let RolloutItem::ResponseItem(ResponseItem::Message { role, content, .. }) = rl.item {
            if role == "user" {
                for c in content {
                    if let ContentItem::InputText { text } = c {
                        if text.contains("full review output from reviewer model") {
                            saw_header = true;
                        }
                        if text.contains("- Prefer Stylize helpers — /tmp/file.rs:10-20") {
                            saw_finding_line = true;
                        }
                    }
                }
            } else if role == "assistant" {
                for c in content {
                    if let ContentItem::OutputText { text } = c {
                        if text.contains("<user_action>") {
                            saw_assistant_xml = true;
                        }
                        if text == expected_assistant_text {
                            saw_assistant_plain = true;
                        }
                    }
                }
            }
        }
    }
    assert!(saw_header, "user header missing from rollout");
    assert!(
        saw_finding_line,
        "formatted finding line missing from rollout"
    );
    assert!(
        saw_assistant_plain,
        "assistant review output missing from rollout"
    );
    assert!(
        !saw_assistant_xml,
        "assistant review output contains user_action markup"
    );

    let _codex_home_guard = codex_home;
    server.verify().await;
}

/// When the model returns plain text that is not JSON, ensure the child
/// lifecycle still occurs and the plain text is surfaced via
/// ExitedReviewMode(Some(..)) as the overall_explanation.
// Windows CI only: bump to 4 workers to prevent SSE/event starvation and test timeouts.
#[cfg_attr(windows, tokio::test(flavor = "multi_thread", worker_threads = 4))]
#[cfg_attr(not(windows), tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn review_op_with_plain_text_emits_review_fallback() {
    skip_if_no_network!();

    let sse_raw = r#"[
        {"type":"response.output_item.done", "item":{
            "type":"message", "role":"assistant",
            "content":[{"type":"output_text","text":"just plain text"}]
        }},
        {"type":"response.completed", "response": {"id": "__ID__"}}
    ]"#;
    let (server, _request_log) = start_responses_server_with_sse(sse_raw, 1).await;
    let codex_home = Arc::new(TempDir::new().unwrap());
    let codex = new_conversation_for_server(&server, codex_home.clone(), |_| {}).await;

    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Plain text review".to_string(),
                },
                user_facing_hint: None,
                validate_findings: false,
            },
        })
        .await
        .unwrap();

    let _entered = wait_for_event(&codex, |ev| matches!(ev, EventMsg::EnteredReviewMode(_))).await;
    let closed = wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExitedReviewMode(_))).await;
    let review = match closed {
        EventMsg::ExitedReviewMode(ev) => ev
            .review_output
            .expect("expected ExitedReviewMode with Some(review_output)"),
        other => panic!("expected ExitedReviewMode(..), got {other:?}"),
    };

    // Expect a structured fallback carrying the plain text.
    let expected = ReviewOutputEvent {
        overall_explanation: "just plain text".to_string(),
        ..Default::default()
    };
    assert_eq!(expected, review);
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let _codex_home_guard = codex_home;
    server.verify().await;
}

#[cfg_attr(windows, tokio::test(flavor = "multi_thread", worker_threads = 4))]
#[cfg_attr(not(windows), tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn review_op_with_plain_text_and_findings_validation_fails_closed() {
    skip_if_no_network!();

    let sse_raw = r#"[
        {"type":"response.output_item.done", "item":{
            "type":"message", "role":"assistant",
            "content":[{"type":"output_text","text":"just plain text"}]
        }},
        {"type":"response.completed", "response": {"id": "__ID__"}}
    ]"#;
    let (server, request_log) = start_responses_server_with_sse(sse_raw, 1).await;
    let codex_home = Arc::new(TempDir::new().unwrap());
    let codex = new_conversation_for_server(&server, codex_home.clone(), |_| {}).await;

    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Plain text review with validation".to_string(),
                },
                user_facing_hint: None,
                validate_findings: true,
            },
        })
        .await
        .unwrap();

    let _entered = wait_for_event(&codex, |ev| matches!(ev, EventMsg::EnteredReviewMode(_))).await;
    let closed = wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExitedReviewMode(_))).await;
    let failure_message = match closed {
        EventMsg::ExitedReviewMode(ev) => {
            assert_eq!(ev.review_output, None);
            ev.failure_message
                .expect("expected explicit failure message")
        }
        other => panic!("expected ExitedReviewMode(..), got {other:?}"),
    };
    assert_eq!(
        failure_message,
        "Reviewer did not return valid structured output, so the review did not complete cleanly."
    );
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let rollout_path = codex.rollout_path().expect("rollout path");
    let rollout = std::fs::read_to_string(&rollout_path).expect("read rollout file");
    assert!(
        rollout.contains("but was interrupted"),
        "malformed reviewer output should persist as an interrupted review"
    );
    assert!(
        !rollout.contains("just plain text"),
        "malformed reviewer output should not be persisted as a successful review result"
    );

    let request_body = request_log.single_request().body_json();
    assert_eq!(
        request_body["text"]["format"]["type"].as_str(),
        Some("json_schema")
    );
    assert_eq!(
        request_body["text"]["format"]["strict"].as_bool(),
        Some(true)
    );

    let _codex_home_guard = codex_home;
    server.verify().await;
}

#[cfg_attr(windows, tokio::test(flavor = "multi_thread", worker_threads = 4))]
#[cfg_attr(not(windows), tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn review_op_with_wrapped_json_and_findings_validation_fails_closed() {
    skip_if_no_network!();

    let wrapped_review = format!(
        "Here is the review:\n{}",
        serde_json::json!({
            "findings": [
                {
                    "title": "Wrapped finding",
                    "body": "This wrapped JSON should not be accepted during validated review.",
                    "confidence_score": 0.8,
                    "priority": 1,
                    "code_location": {
                        "absolute_file_path": "/tmp/wrapped.rs",
                        "line_range": {"start": 10, "end": 11}
                    }
                }
            ],
            "overall_correctness": "patch is incorrect",
            "overall_explanation": "Wrapped JSON should fail closed.",
            "overall_confidence_score": 0.8
        })
    );
    let sse_raw = format!(
        r#"[
        {{"type":"response.output_item.done", "item":{{
            "type":"message", "role":"assistant",
            "content":[{{"type":"output_text","text":{wrapped_review:?}}}]
        }}}},
        {{"type":"response.completed", "response": {{"id": "__ID__"}}}}
    ]"#
    );
    let (server, _request_log) = start_responses_server_with_sse(&sse_raw, 1).await;
    let codex_home = Arc::new(TempDir::new().unwrap());
    let codex = new_conversation_for_server(&server, codex_home.clone(), |_| {}).await;

    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Wrapped JSON review with validation".to_string(),
                },
                user_facing_hint: None,
                validate_findings: true,
            },
        })
        .await
        .unwrap();

    let _entered = wait_for_event(&codex, |ev| matches!(ev, EventMsg::EnteredReviewMode(_))).await;
    let closed = wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExitedReviewMode(_))).await;
    let failure_message = match closed {
        EventMsg::ExitedReviewMode(ev) => {
            assert_eq!(ev.review_output, None);
            ev.failure_message
                .expect("expected explicit failure message")
        }
        other => panic!("expected ExitedReviewMode(..), got {other:?}"),
    };
    assert_eq!(
        failure_message,
        "Reviewer did not return valid structured output, so the review did not complete cleanly."
    );
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let rollout_path = codex.rollout_path().expect("rollout path");
    let rollout = std::fs::read_to_string(&rollout_path).expect("read rollout file");
    assert!(
        rollout.contains("but was interrupted"),
        "wrapped reviewer output should persist as an interrupted review"
    );
    assert!(
        !rollout.contains("Wrapped finding"),
        "wrapped reviewer output should not be persisted as a successful review result"
    );

    let _codex_home_guard = codex_home;
    server.verify().await;
}

/// Ensure review flow suppresses assistant-specific streaming/completion events:
/// - AgentMessageContentDelta
/// - AgentMessageDelta (legacy)
/// - ItemCompleted for TurnItem::AgentMessage
// Windows CI only: bump to 4 workers to prevent SSE/event starvation and test timeouts.
#[cfg_attr(windows, tokio::test(flavor = "multi_thread", worker_threads = 4))]
#[cfg_attr(not(windows), tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn review_filters_agent_message_related_events() {
    skip_if_no_network!();

    // Stream simulating a typing assistant message with deltas and finalization.
    let sse_raw = r#"[
        {"type":"response.output_item.added", "item":{
            "type":"message", "role":"assistant", "id":"msg-1",
            "content":[{"type":"output_text","text":""}]
        }},
        {"type":"response.output_text.delta", "delta":"Hi"},
        {"type":"response.output_text.delta", "delta":" there"},
        {"type":"response.output_item.done", "item":{
            "type":"message", "role":"assistant", "id":"msg-1",
            "content":[{"type":"output_text","text":"Hi there"}]
        }},
        {"type":"response.completed", "response": {"id": "__ID__"}}
    ]"#;
    let (server, _request_log) = start_responses_server_with_sse(sse_raw, 1).await;
    let codex_home = Arc::new(TempDir::new().unwrap());
    let codex = new_conversation_for_server(&server, codex_home.clone(), |_| {}).await;

    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Filter streaming events".to_string(),
                },
                user_facing_hint: None,
                validate_findings: false,
            },
        })
        .await
        .unwrap();

    let mut saw_entered = false;
    let mut saw_exited = false;

    // Drain until TurnComplete; assert streaming-related events never surface.
    wait_for_event(&codex, |event| match event {
        EventMsg::TurnComplete(_) => true,
        EventMsg::EnteredReviewMode(_) => {
            saw_entered = true;
            false
        }
        EventMsg::ExitedReviewMode(_) => {
            saw_exited = true;
            false
        }
        // The following must be filtered by review flow
        EventMsg::AgentMessageContentDelta(_) => {
            panic!("unexpected AgentMessageContentDelta surfaced during review")
        }
        EventMsg::AgentMessageDelta(_) => {
            panic!("unexpected AgentMessageDelta surfaced during review")
        }
        _ => false,
    })
    .await;
    assert!(saw_entered && saw_exited, "missing review lifecycle events");

    let _codex_home_guard = codex_home;
    server.verify().await;
}

/// When the model returns structured JSON in a review, ensure only a single
/// non-streaming AgentMessage is emitted; the UI consumes the structured
/// result via ExitedReviewMode plus a final assistant message.
// Windows CI only: bump to 4 workers to prevent SSE/event starvation and test timeouts.
#[cfg_attr(windows, tokio::test(flavor = "multi_thread", worker_threads = 4))]
#[cfg_attr(not(windows), tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn review_does_not_emit_agent_message_on_structured_output() {
    skip_if_no_network!();

    let review_json = serde_json::json!({
        "findings": [
            {
                "title": "Example",
                "body": "Structured review output.",
                "confidence_score": 0.5,
                "priority": 1,
                "code_location": {
                    "absolute_file_path": "/tmp/file.rs",
                    "line_range": {"start": 1, "end": 2}
                }
            }
        ],
        "overall_correctness": "ok",
        "overall_explanation": "ok",
        "overall_confidence_score": 0.5
    })
    .to_string();
    let sse_template = r#"[
            {"type":"response.output_item.done", "item":{
                "type":"message", "role":"assistant",
                "content":[{"type":"output_text","text":__REVIEW__}]
            }},
            {"type":"response.completed", "response": {"id": "__ID__"}}
        ]"#;
    let review_json_escaped = serde_json::to_string(&review_json).unwrap();
    let sse_raw = sse_template.replace("__REVIEW__", &review_json_escaped);
    let (server, _request_log) = start_responses_server_with_sse(&sse_raw, 1).await;
    let codex_home = Arc::new(TempDir::new().unwrap());
    let codex = new_conversation_for_server(&server, codex_home.clone(), |_| {}).await;

    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "check structured".to_string(),
                },
                user_facing_hint: None,
                validate_findings: false,
            },
        })
        .await
        .unwrap();

    // Drain events until TurnComplete; ensure we only see a final
    // AgentMessage (no streaming assistant messages).
    let mut saw_entered = false;
    let mut saw_exited = false;
    let mut agent_messages = 0;
    wait_for_event(&codex, |event| match event {
        EventMsg::TurnComplete(_) => true,
        EventMsg::AgentMessage(_) => {
            agent_messages += 1;
            false
        }
        EventMsg::EnteredReviewMode(_) => {
            saw_entered = true;
            false
        }
        EventMsg::ExitedReviewMode(_) => {
            saw_exited = true;
            false
        }
        _ => false,
    })
    .await;
    assert_eq!(1, agent_messages, "expected exactly one AgentMessage event");
    assert!(saw_entered && saw_exited, "missing review lifecycle events");

    let _codex_home_guard = codex_home;
    server.verify().await;
}

/// Ensure that when a custom `review_model` is set in the config, the review
/// request uses that model (and not the main chat model).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_uses_custom_review_model_from_config() {
    skip_if_no_network!();

    // Minimal stream: just a completed event
    let sse_raw = r#"[
        {"type":"response.completed", "response": {"id": "__ID__"}}
    ]"#;
    let (server, request_log) = start_responses_server_with_sse(sse_raw, 1).await;
    let codex_home = Arc::new(TempDir::new().unwrap());
    // Choose a review model different from the main model; ensure it is used.
    let codex = new_conversation_for_server(&server, codex_home.clone(), |cfg| {
        cfg.model = Some("gpt-4.1".to_string());
        cfg.review_model = Some("gpt-5.1".to_string());
    })
    .await;

    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "use custom model".to_string(),
                },
                user_facing_hint: None,
                validate_findings: false,
            },
        })
        .await
        .unwrap();

    // Wait for completion
    let _entered = wait_for_event(&codex, |ev| matches!(ev, EventMsg::EnteredReviewMode(_))).await;
    let _closed = wait_for_event(&codex, |ev| {
        matches!(
            ev,
            EventMsg::ExitedReviewMode(ExitedReviewModeEvent {
                review_output: None,
                ..
            })
        )
    })
    .await;
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    // Assert the request body model equals the configured review model
    let request = request_log.single_request();
    assert_eq!(request.path(), "/v1/responses");
    let body = request.body_json();
    assert_eq!(body["model"].as_str().unwrap(), "gpt-5.1");

    let _codex_home_guard = codex_home;
    server.verify().await;
}

/// Ensure that when `review_model` is not set in the config, the review request
/// uses the session model.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_uses_session_model_when_review_model_unset() {
    skip_if_no_network!();

    // Minimal stream: just a completed event
    let sse_raw = r#"[
        {"type":"response.completed", "response": {"id": "__ID__"}}
    ]"#;
    let (server, request_log) = start_responses_server_with_sse(sse_raw, 1).await;
    let codex_home = Arc::new(TempDir::new().unwrap());
    let codex = new_conversation_for_server(&server, codex_home.clone(), |cfg| {
        cfg.model = Some("gpt-4.1".to_string());
        cfg.review_model = None;
    })
    .await;

    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "use session model".to_string(),
                },
                user_facing_hint: None,
                validate_findings: false,
            },
        })
        .await
        .unwrap();

    let _entered = wait_for_event(&codex, |ev| matches!(ev, EventMsg::EnteredReviewMode(_))).await;
    let _closed = wait_for_event(&codex, |ev| {
        matches!(
            ev,
            EventMsg::ExitedReviewMode(ExitedReviewModeEvent {
                review_output: None,
                ..
            })
        )
    })
    .await;
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let request = request_log.single_request();
    assert_eq!(request.path(), "/v1/responses");
    let body = request.body_json();
    assert_eq!(body["model"].as_str().unwrap(), "gpt-4.1");

    let _codex_home_guard = codex_home;
    server.verify().await;
}

/// When a review session begins, it must not prepend prior chat history from
/// the parent session. The request `input` should contain only the review
/// prompt from the user.
// Windows CI only: bump to 4 workers to prevent SSE/event starvation and test timeouts.
#[cfg_attr(windows, tokio::test(flavor = "multi_thread", worker_threads = 4))]
#[cfg_attr(not(windows), tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn review_input_isolated_from_parent_history() {
    skip_if_no_network!();

    // Mock server for the single review request
    let sse_raw = r#"[
        {"type":"response.completed", "response": {"id": "__ID__"}}
    ]"#;
    let (server, request_log) = start_responses_server_with_sse(sse_raw, 1).await;

    // Seed a parent session history via resume file with both user + assistant items.
    let codex_home = Arc::new(TempDir::new().unwrap());

    let session_file = codex_home.path().join("resume.jsonl");
    {
        let mut f = tokio::fs::File::create(&session_file).await.unwrap();
        let convo_id = Uuid::new_v4();
        // Proper session_meta line (enveloped) with a conversation id
        let meta_line = serde_json::json!({
            "timestamp": "2024-01-01T00:00:00.000Z",
            "type": "session_meta",
            "payload": {
                "id": convo_id,
                "timestamp": "2024-01-01T00:00:00Z",
                "cwd": ".",
                "originator": "test_originator",
                "cli_version": "test_version",
                "model_provider": "test-provider"
            }
        });
        f.write_all(format!("{meta_line}\n").as_bytes())
            .await
            .unwrap();

        // Prior user message (enveloped response_item)
        let user = codex_protocol::models::ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![codex_protocol::models::ContentItem::InputText {
                text: "parent: earlier user message".to_string(),
            }],
            end_turn: None,
            phase: None,
        };
        let user_json = serde_json::to_value(&user).unwrap();
        let user_line = serde_json::json!({
            "timestamp": "2024-01-01T00:00:01.000Z",
            "type": "response_item",
            "payload": user_json
        });
        f.write_all(format!("{user_line}\n").as_bytes())
            .await
            .unwrap();

        // Prior assistant message (enveloped response_item)
        let assistant = codex_protocol::models::ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![codex_protocol::models::ContentItem::OutputText {
                text: "parent: assistant reply".to_string(),
            }],
            end_turn: None,
            phase: None,
        };
        let assistant_json = serde_json::to_value(&assistant).unwrap();
        let assistant_line = serde_json::json!({
            "timestamp": "2024-01-01T00:00:02.000Z",
            "type": "response_item",
            "payload": assistant_json
        });
        f.write_all(format!("{assistant_line}\n").as_bytes())
            .await
            .unwrap();
    }
    let codex =
        resume_conversation_for_server(&server, codex_home.clone(), session_file.clone(), |_| {})
            .await;

    // Submit review request; it must start fresh (no parent history in `input`).
    let review_prompt = "Please review only this".to_string();
    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: review_prompt.clone(),
                },
                user_facing_hint: None,
                validate_findings: false,
            },
        })
        .await
        .unwrap();

    let _entered = wait_for_event(&codex, |ev| matches!(ev, EventMsg::EnteredReviewMode(_))).await;
    let _closed = wait_for_event(&codex, |ev| {
        matches!(
            ev,
            EventMsg::ExitedReviewMode(ExitedReviewModeEvent {
                review_output: None,
                ..
            })
        )
    })
    .await;
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    // Assert the request `input` contains the environment context followed by the user review prompt.
    let request = request_log.single_request();
    assert_eq!(request.path(), "/v1/responses");
    let body = request.body_json();
    let input = body["input"].as_array().expect("input array");
    assert!(
        input.len() >= 2,
        "expected at least environment context and review prompt"
    );

    let env_text = input
        .iter()
        .filter_map(|msg| msg.get("content").and_then(|content| content.as_array()))
        .flat_map(|content| content.iter())
        .filter_map(|entry| entry.get("text").and_then(|text| text.as_str()))
        .find(|text| text.starts_with(ENVIRONMENT_CONTEXT_OPEN_TAG))
        .expect("env text");
    assert!(
        env_text.contains("<cwd>"),
        "environment context should include cwd"
    );

    let review_text = input
        .iter()
        .filter_map(|msg| msg.get("content").and_then(|content| content.as_array()))
        .flat_map(|content| content.iter())
        .filter_map(|entry| entry.get("text").and_then(|text| text.as_str()))
        .find(|text| *text == review_prompt)
        .expect("review prompt text");
    assert_eq!(
        review_text, review_prompt,
        "user message should only contain the raw review prompt"
    );

    // Ensure the REVIEW_PROMPT rubric is sent via instructions.
    let instructions = body["instructions"].as_str().expect("instructions string");
    assert!(
        instructions.starts_with(REVIEW_PROMPT),
        "review instructions should start with the review rubric"
    );
    assert!(
        instructions.contains(
            "Follow repository-specific guidance from inherited developer/user instructions"
        ),
        "review instructions should preserve repo guidance for review subagents"
    );

    // Also verify that a user interruption note was recorded in the rollout.
    let path = codex.rollout_path().expect("rollout path");
    let text = std::fs::read_to_string(&path).expect("read rollout file");
    let mut saw_interruption_message = false;
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(line).expect("jsonl line");
        let rl: RolloutLine = serde_json::from_value(v).expect("rollout line");
        if let RolloutItem::ResponseItem(ResponseItem::Message { role, content, .. }) = rl.item
            && role == "user"
        {
            for c in content {
                if let ContentItem::InputText { text } = c
                    && text.contains("User initiated a review task, but was interrupted.")
                {
                    saw_interruption_message = true;
                    break;
                }
            }
        }
        if saw_interruption_message {
            break;
        }
    }
    assert!(
        saw_interruption_message,
        "expected user interruption message in rollout"
    );

    let _codex_home_guard = codex_home;
    server.verify().await;
}

/// After a review thread finishes, its conversation should be visible in the
/// parent session so later turns can reference the results.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_history_surfaces_in_parent_session() {
    skip_if_no_network!();

    // Respond to both the review request and the subsequent parent request.
    let sse_raw = r#"[
        {"type":"response.output_item.done", "item":{
            "type":"message", "role":"assistant",
            "content":[{"type":"output_text","text":"review assistant output"}]
        }},
        {"type":"response.completed", "response": {"id": "__ID__"}}
    ]"#;
    let (server, request_log) = start_responses_server_with_sse(sse_raw, 2).await;
    let codex_home = Arc::new(TempDir::new().unwrap());
    let codex = new_conversation_for_server(&server, codex_home.clone(), |_| {}).await;

    // 1) Run a review turn that produces an assistant message (isolated in child).
    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Start a review".to_string(),
                },
                user_facing_hint: None,
                validate_findings: false,
            },
        })
        .await
        .unwrap();
    let _entered = wait_for_event(&codex, |ev| matches!(ev, EventMsg::EnteredReviewMode(_))).await;
    let _closed = wait_for_event(&codex, |ev| {
        matches!(
            ev,
            EventMsg::ExitedReviewMode(ExitedReviewModeEvent {
                review_output: Some(_),
                ..
            })
        )
    })
    .await;
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    // 2) Continue in the parent session; request input must not include any review items.
    let followup = "back to parent".to_string();
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: followup.clone(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    // Inspect the second request (parent turn) input contents.
    // Parent turns include session initial messages (user_instructions, environment_context).
    // Critically, no messages from the review thread should appear.
    let requests = request_log.requests();
    assert_eq!(requests.len(), 2);
    for request in &requests {
        assert_eq!(request.path(), "/v1/responses");
    }
    let body = requests[1].body_json();
    let input = body["input"].as_array().expect("input array");

    // Must include the followup as the last item for this turn
    let last = input.last().expect("at least one item in input");
    assert_eq!(last["role"].as_str().unwrap(), "user");
    let last_text = last["content"][0]["text"].as_str().unwrap();
    assert_eq!(last_text, followup);

    // Ensure review-thread content is present for downstream turns.
    let contains_review_rollout_user = input.iter().any(|msg| {
        msg["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("User initiated a review task.")
    });
    let contains_review_assistant = input.iter().any(|msg| {
        msg["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("review assistant output")
    });
    assert!(
        contains_review_rollout_user,
        "review rollout user message missing from parent turn input"
    );
    assert!(
        contains_review_assistant,
        "review assistant output missing from parent turn input"
    );

    let _codex_home_guard = codex_home;
    server.verify().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_validation_filters_transcript_and_parent_context() {
    skip_if_no_network!();
    let draft_review_message =
        "Draft review note that should stay hidden until validation completes.";

    let initial_review_json = serde_json::json!({
        "findings": [
            {
                "title": "Stale finding",
                "body": "This report should be discarded by validation.",
                "confidence_score": 0.9,
                "priority": 1,
                "code_location": {
                    "absolute_file_path": "/tmp/stale.rs",
                    "line_range": {"start": 1, "end": 2}
                }
            },
            {
                "title": "Validated finding",
                "body": "This report should survive validation with a clearer explanation.",
                "confidence_score": 0.7,
                "priority": 1,
                "code_location": {
                    "absolute_file_path": "/tmp/validated.rs",
                    "line_range": {"start": 5, "end": 6}
                }
            }
        ],
        "overall_correctness": "incorrect",
        "overall_explanation": "Initial review output before validation.",
        "overall_confidence_score": 0.8
    })
    .to_string();
    let validated_output = ReviewOutputEvent {
        findings: vec![ReviewFinding {
            title: "Validated finding".to_string(),
            body: "This finding survived validation.".to_string(),
            confidence_score: 0.95,
            priority: 1,
            code_location: ReviewCodeLocation {
                absolute_file_path: PathBuf::from("/tmp/validated.rs"),
                line_range: ReviewLineRange { start: 5, end: 6 },
            },
        }],
        overall_correctness: "patch is incorrect".to_string(),
        overall_explanation: "Kept the validated finding after discarding the stale report."
            .to_string(),
        overall_confidence_score: 0.91,
    };
    let validated_json = serde_json::to_string(&validated_output).expect("validated json");

    let server = start_mock_server().await;
    let request_log = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg-draft", draft_review_message),
                ev_assistant_message("msg-1", &initial_review_json),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", &validated_json),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_assistant_message("msg-3", "parent ack"),
                ev_completed("resp-3"),
            ]),
        ],
    )
    .await;

    let codex_home = Arc::new(TempDir::new().unwrap());
    let codex = new_conversation_for_server(&server, codex_home.clone(), |_| {}).await;

    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Validate review findings".to_string(),
                },
                user_facing_hint: None,
                validate_findings: true,
            },
        })
        .await
        .unwrap();

    let _entered = wait_for_event(&codex, |ev| matches!(ev, EventMsg::EnteredReviewMode(_))).await;
    let closed = wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExitedReviewMode(_))).await;
    let review = match closed {
        EventMsg::ExitedReviewMode(ev) => ev
            .review_output
            .expect("expected ExitedReviewMode with Some(review_output)"),
        other => panic!("expected ExitedReviewMode(..), got {other:?}"),
    };
    assert_eq!(validated_output, review);
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let rollout_path = codex.rollout_path().expect("rollout path");
    let rollout = std::fs::read_to_string(&rollout_path).expect("read rollout file");
    assert!(
        rollout.contains("Validated finding"),
        "validated finding missing from rollout"
    );
    assert!(
        !rollout.contains("Stale finding"),
        "stale finding leaked into rollout"
    );
    assert!(
        !rollout.contains(draft_review_message),
        "draft reviewer message leaked into rollout before validation"
    );
    assert!(
        !rollout.contains(
            "Validate the review findings below against the current codebase before they are surfaced to the user."
        ),
        "validator prompt leaked into rollout"
    );

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "back to parent".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = request_log.requests();
    assert_eq!(requests.len(), 3);
    let follow_up_input = requests[2].input();
    let follow_up_text = follow_up_input
        .iter()
        .filter_map(|msg| msg.get("content").and_then(|content| content.as_array()))
        .flat_map(|content| content.iter())
        .filter_map(|entry| entry.get("text").and_then(|text| text.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        follow_up_text.contains("Validated finding"),
        "validated finding missing from downstream parent context"
    );
    assert!(
        !follow_up_text.contains("Stale finding"),
        "stale finding leaked into downstream parent context"
    );
    assert!(
        !follow_up_text.contains(draft_review_message),
        "draft reviewer message leaked into downstream parent context"
    );
    assert!(
        !follow_up_text.contains(
            "Validate the review findings below against the current codebase before they are surfaced to the user."
        ),
        "validator prompt leaked into downstream parent context"
    );

    let _codex_home_guard = codex_home;
    server.verify().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_validation_suppresses_reasoning_and_raw_events_before_exit() {
    skip_if_no_network!();

    let reasoning_summary = "Draft reasoning finding that must stay hidden.";
    let reasoning_raw = "Draft raw reasoning detail that must stay hidden.";
    let initial_review_json = serde_json::json!({
        "findings": [
            {
                "title": "Validated finding",
                "body": "This finding should survive validation.",
                "confidence_score": 0.8,
                "priority": 1,
                "code_location": {
                    "absolute_file_path": "/tmp/validated.rs",
                    "line_range": {"start": 5, "end": 6}
                }
            }
        ],
        "overall_correctness": "incorrect",
        "overall_explanation": "Initial review output before validation.",
        "overall_confidence_score": 0.8
    })
    .to_string();
    let validated_output = ReviewOutputEvent {
        findings: vec![ReviewFinding {
            title: "Validated finding".to_string(),
            body: "This finding survived validation.".to_string(),
            confidence_score: 0.95,
            priority: 1,
            code_location: ReviewCodeLocation {
                absolute_file_path: PathBuf::from("/tmp/validated.rs"),
                line_range: ReviewLineRange { start: 5, end: 6 },
            },
        }],
        overall_correctness: "patch is incorrect".to_string(),
        overall_explanation: "Kept the validated finding.".to_string(),
        overall_confidence_score: 0.91,
    };
    let validated_json = serde_json::to_string(&validated_output).expect("validated json");

    let server = start_mock_server().await;
    let _request_log = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_reasoning_item_added("reasoning-1", &[""]),
                ev_reasoning_summary_text_delta(reasoning_summary),
                ev_reasoning_text_delta(reasoning_raw),
                ev_reasoning_item("reasoning-1", &[reasoning_summary], &[reasoning_raw]),
                ev_assistant_message("msg-1", &initial_review_json),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", &validated_json),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let codex_home = Arc::new(TempDir::new().unwrap());
    let codex = new_conversation_for_server(&server, codex_home.clone(), |_| {}).await;

    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Validate review findings".to_string(),
                },
                user_facing_hint: None,
                validate_findings: true,
            },
        })
        .await
        .unwrap();

    let mut leaked_event_kinds = Vec::new();
    loop {
        match wait_for_event(&codex, |_| true).await {
            EventMsg::EnteredReviewMode(_) => {}
            EventMsg::AgentReasoning(_) => leaked_event_kinds.push("AgentReasoning"),
            EventMsg::AgentReasoningDelta(_) => leaked_event_kinds.push("AgentReasoningDelta"),
            EventMsg::AgentReasoningRawContent(_) => {
                leaked_event_kinds.push("AgentReasoningRawContent");
            }
            EventMsg::AgentReasoningRawContentDelta(_) => {
                leaked_event_kinds.push("AgentReasoningRawContentDelta");
            }
            EventMsg::AgentReasoningSectionBreak(_) => {
                leaked_event_kinds.push("AgentReasoningSectionBreak");
            }
            EventMsg::RawResponseItem(raw)
                if matches!(raw.item, ResponseItem::Reasoning { .. }) =>
            {
                leaked_event_kinds.push("RawResponseItem::Reasoning");
            }
            EventMsg::ReasoningContentDelta(_) => leaked_event_kinds.push("ReasoningContentDelta"),
            EventMsg::ReasoningRawContentDelta(_) => {
                leaked_event_kinds.push("ReasoningRawContentDelta");
            }
            EventMsg::ExitedReviewMode(ev) => {
                assert_eq!(ev.review_output, Some(validated_output.clone()));
                break;
            }
            _ => {}
        }
    }

    assert!(
        leaked_event_kinds.is_empty(),
        "reasoning/raw events leaked before validation finished: {leaked_event_kinds:?}"
    );
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let _codex_home_guard = codex_home;
    server.verify().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_validation_suppresses_first_pass_tool_events_before_exit() {
    skip_if_no_network!();

    let call_id = "review-validation-exec";
    let exec_args = serde_json::json!({
        "cmd": "",
        "yield_time_ms": 1_000,
    })
    .to_string();
    let initial_review_json = serde_json::json!({
        "findings": [
            {
                "title": "Validated finding",
                "body": "This finding should survive validation.",
                "confidence_score": 0.8,
                "priority": 1,
                "code_location": {
                    "absolute_file_path": "/tmp/validated.rs",
                    "line_range": {"start": 5, "end": 6}
                }
            }
        ],
        "overall_correctness": "incorrect",
        "overall_explanation": "Initial review output before validation.",
        "overall_confidence_score": 0.8
    })
    .to_string();
    let validated_output = ReviewOutputEvent {
        findings: vec![ReviewFinding {
            title: "Validated finding".to_string(),
            body: "This finding survived validation.".to_string(),
            confidence_score: 0.95,
            priority: 1,
            code_location: ReviewCodeLocation {
                absolute_file_path: PathBuf::from("/tmp/validated.rs"),
                line_range: ReviewLineRange { start: 5, end: 6 },
            },
        }],
        overall_correctness: "patch is incorrect".to_string(),
        overall_explanation: "Kept the validated finding.".to_string(),
        overall_confidence_score: 0.91,
    };
    let validated_json = serde_json::to_string(&validated_output).expect("validated json");

    let server = start_mock_server().await;
    let _request_log = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, "exec_command", &exec_args),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", &initial_review_json),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_assistant_message("msg-2", &validated_json),
                ev_completed("resp-3"),
            ]),
        ],
    )
    .await;

    let codex_home = Arc::new(TempDir::new().unwrap());
    let codex = new_conversation_for_server(&server, codex_home.clone(), |_| {}).await;

    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Validate review findings".to_string(),
                },
                user_facing_hint: None,
                validate_findings: true,
            },
        })
        .await
        .unwrap();

    let mut leaked_event_kinds = Vec::new();
    loop {
        match wait_for_event(&codex, |_| true).await {
            EventMsg::EnteredReviewMode(_) => {}
            EventMsg::ExecCommandBegin(_) => leaked_event_kinds.push("ExecCommandBegin"),
            EventMsg::ExecCommandOutputDelta(_) => {
                leaked_event_kinds.push("ExecCommandOutputDelta");
            }
            EventMsg::ExecCommandEnd(_) => leaked_event_kinds.push("ExecCommandEnd"),
            EventMsg::ExitedReviewMode(ev) => {
                assert_eq!(ev.review_output, Some(validated_output.clone()));
                break;
            }
            _ => {}
        }
    }

    assert!(
        leaked_event_kinds.is_empty(),
        "tool events leaked before validation finished: {leaked_event_kinds:?}"
    );
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let _codex_home_guard = codex_home;
    server.verify().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_validation_preserves_first_pass_error_diagnostics_before_exit() {
    skip_if_no_network!();

    let server = start_mock_server().await;
    let _request_log = mount_sse_sequence(
        &server,
        vec![sse_failed(
            "resp-1",
            "server_error",
            "simulated first-pass review failure",
        )],
    )
    .await;

    let codex_home = Arc::new(TempDir::new().unwrap());
    let codex = new_conversation_for_server(&server, codex_home.clone(), |config| {
        config.model_provider.stream_max_retries = Some(0);
    })
    .await;

    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Validate review findings".to_string(),
                },
                user_facing_hint: None,
                validate_findings: true,
            },
        })
        .await
        .unwrap();

    let _entered = wait_for_event(&codex, |ev| matches!(ev, EventMsg::EnteredReviewMode(_))).await;
    let error_message = wait_for_event_match(&codex, |event| match event {
        EventMsg::Error(err) => Some(err.message.clone()),
        _ => None,
    })
    .await;
    assert!(
        error_message.contains("simulated first-pass review failure"),
        "expected first-pass error to surface before exit, got {error_message}"
    );

    let exited = wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExitedReviewMode(_))).await;
    let EventMsg::ExitedReviewMode(exited) = exited else {
        unreachable!("wait_for_event returned non-review-exit event");
    };
    assert_eq!(exited.review_output, None);
    assert_eq!(
        exited.failure_message,
        Some(
            "Review was interrupted. Please re-run /review and wait for it to complete."
                .to_string(),
        )
    );

    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let _codex_home_guard = codex_home;
    server.verify().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_validation_failure_exits_with_unstructured_error_output() {
    skip_if_no_network!();

    let initial_review_json = serde_json::json!({
        "findings": [
            {
                "title": "Needs validation",
                "body": "This finding should not be surfaced if validation fails.",
                "confidence_score": 0.8,
                "priority": 1,
                "code_location": {
                    "absolute_file_path": "/tmp/needs-validation.rs",
                    "line_range": {"start": 11, "end": 12}
                }
            }
        ],
        "overall_correctness": "incorrect",
        "overall_explanation": "Initial review output before validation failure.",
        "overall_confidence_score": 0.8
    })
    .to_string();

    let server = start_mock_server().await;
    let request_log = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg-1", &initial_review_json),
                ev_completed("resp-1"),
            ]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_assistant_message("msg-2", "parent ack"),
                ev_completed("resp-3"),
            ]),
        ],
    )
    .await;

    let codex_home = Arc::new(TempDir::new().unwrap());
    let codex = new_conversation_for_server(&server, codex_home.clone(), |_| {}).await;

    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Surface validation failure".to_string(),
                },
                user_facing_hint: None,
                validate_findings: true,
            },
        })
        .await
        .unwrap();

    let _entered = wait_for_event(&codex, |ev| matches!(ev, EventMsg::EnteredReviewMode(_))).await;
    let closed = wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExitedReviewMode(_))).await;
    let failure_message = match closed {
        EventMsg::ExitedReviewMode(ev) => {
            assert_eq!(ev.review_output, None);
            ev.failure_message
                .expect("expected explicit failure message")
        }
        other => panic!("expected ExitedReviewMode(..), got {other:?}"),
    };
    assert_eq!(
        failure_message,
        "Review findings validation did not complete cleanly, so no findings were surfaced."
    );
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let rollout_path = codex.rollout_path().expect("rollout path");
    let rollout = std::fs::read_to_string(&rollout_path).expect("read rollout file");
    assert!(
        rollout.contains("but was interrupted"),
        "validation failure should persist as an interrupted review"
    );
    assert!(
        !rollout.contains("Initial review output before validation failure."),
        "initial findings should not be persisted as a successful validated review"
    );

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "back to parent".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = request_log.requests();
    assert_eq!(requests.len(), 3);
    let follow_up_input = requests[2].input();
    let follow_up_text = follow_up_input
        .iter()
        .filter_map(|msg| msg.get("content").and_then(|content| content.as_array()))
        .flat_map(|content| content.iter())
        .filter_map(|entry| entry.get("text").and_then(|text| text.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        follow_up_text.contains("Review findings validation did not complete cleanly"),
        "validation failure explanation missing from downstream parent context"
    );
    assert!(
        follow_up_text.contains("but was interrupted"),
        "validation failure should be preserved as an interrupted review in downstream context"
    );

    let _codex_home_guard = codex_home;
    server.verify().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_validation_forwards_validator_error_diagnostics_before_exit() {
    skip_if_no_network!();

    let initial_review_json = serde_json::json!({
        "findings": [
            {
                "title": "Needs validator diagnostics",
                "body": "Validator-side errors should be surfaced before the generic failure.",
                "confidence_score": 0.8,
                "priority": 1,
                "code_location": {
                    "absolute_file_path": "/tmp/validator-error.rs",
                    "line_range": {"start": 14, "end": 15}
                }
            }
        ],
        "overall_correctness": "incorrect",
        "overall_explanation": "Initial review output before validator failure.",
        "overall_confidence_score": 0.8
    })
    .to_string();

    let server = start_mock_server().await;
    let _request_log = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg-1", &initial_review_json),
                ev_completed("resp-1"),
            ]),
            sse_failed("resp-2", "server_error", "simulated validator failure"),
        ],
    )
    .await;

    let codex_home = Arc::new(TempDir::new().unwrap());
    let codex = new_conversation_for_server(&server, codex_home.clone(), |config| {
        config.model_provider.stream_max_retries = Some(0);
    })
    .await;

    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Surface validator-side errors".to_string(),
                },
                user_facing_hint: None,
                validate_findings: true,
            },
        })
        .await
        .unwrap();

    let _entered = wait_for_event(&codex, |ev| matches!(ev, EventMsg::EnteredReviewMode(_))).await;
    let error_message = wait_for_event_match(&codex, |event| match event {
        EventMsg::Error(err) => Some(err.message.clone()),
        _ => None,
    })
    .await;
    assert!(
        error_message.contains("simulated validator failure"),
        "expected validator error to surface before exit, got {error_message}"
    );

    let closed = wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExitedReviewMode(_))).await;
    let failure_message = match closed {
        EventMsg::ExitedReviewMode(ev) => {
            assert_eq!(ev.review_output, None);
            ev.failure_message
                .expect("expected explicit failure message")
        }
        other => panic!("expected ExitedReviewMode(..), got {other:?}"),
    };
    assert_eq!(
        failure_message,
        "Review findings validation did not complete cleanly, so no findings were surfaced."
    );
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let _codex_home_guard = codex_home;
    server.verify().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_validation_rejects_findings_not_in_original_review() {
    skip_if_no_network!();

    let initial_review_json = serde_json::json!({
        "findings": [
            {
                "title": "Original finding",
                "body": "This finding is the only one the validator may keep.",
                "confidence_score": 0.8,
                "priority": 1,
                "code_location": {
                    "absolute_file_path": "/tmp/original.rs",
                    "line_range": {"start": 3, "end": 4}
                }
            }
        ],
        "overall_correctness": "incorrect",
        "overall_explanation": "Initial review output before subset enforcement.",
        "overall_confidence_score": 0.8
    })
    .to_string();
    let validator_json = serde_json::json!({
        "findings": [
            {
                "title": "Invented finding",
                "body": "This finding was not in the original review.",
                "confidence_score": 0.92,
                "priority": 1,
                "code_location": {
                    "absolute_file_path": "/tmp/invented.rs",
                    "line_range": {"start": 10, "end": 11}
                }
            }
        ],
        "overall_correctness": "patch is incorrect",
        "overall_explanation": "The validator tried to add a new finding.",
        "overall_confidence_score": 0.92
    })
    .to_string();

    let server = start_mock_server().await;
    let _request_log = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg-1", &initial_review_json),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", &validator_json),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let codex_home = Arc::new(TempDir::new().unwrap());
    let codex = new_conversation_for_server(&server, codex_home.clone(), |_| {}).await;

    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Reject invented validator findings".to_string(),
                },
                user_facing_hint: None,
                validate_findings: true,
            },
        })
        .await
        .unwrap();

    let _entered = wait_for_event(&codex, |ev| matches!(ev, EventMsg::EnteredReviewMode(_))).await;
    let closed = wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExitedReviewMode(_))).await;
    let failure_message = match closed {
        EventMsg::ExitedReviewMode(ev) => {
            assert_eq!(ev.review_output, None);
            ev.failure_message
                .expect("expected explicit failure message")
        }
        other => panic!("expected ExitedReviewMode(..), got {other:?}"),
    };
    assert_eq!(
        failure_message,
        "Review findings validation returned findings that were not present in the original review, so no findings were surfaced."
    );
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let rollout_path = codex.rollout_path().expect("rollout path");
    let rollout = std::fs::read_to_string(&rollout_path).expect("read rollout file");
    assert!(
        rollout.contains("but was interrupted"),
        "invented validator findings should persist as an interrupted review"
    );
    assert!(
        !rollout.contains("Invented finding"),
        "invented validator findings should not be persisted as validated review output"
    );

    let _codex_home_guard = codex_home;
    server.verify().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_validation_treats_wrapped_json_output_as_failure() {
    skip_if_no_network!();

    let initial_review_json = serde_json::json!({
        "findings": [
            {
                "title": "Needs strict validation",
                "body": "Wrapped validator JSON should fail closed.",
                "confidence_score": 0.8,
                "priority": 1,
                "code_location": {
                    "absolute_file_path": "/tmp/strict-validation.rs",
                    "line_range": {"start": 21, "end": 22}
                }
            }
        ],
        "overall_correctness": "incorrect",
        "overall_explanation": "Initial review output before wrapped validator reply.",
        "overall_confidence_score": 0.8
    })
    .to_string();
    let validator_reply = format!(
        "Here is the validated output:\n{}",
        serde_json::json!({
            "findings": [],
            "overall_correctness": "patch is correct",
            "overall_explanation": "This should be rejected because it is wrapped in prose.",
            "overall_confidence_score": 0.95
        })
    );

    let server = start_mock_server().await;
    let _request_log = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg-1", &initial_review_json),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", &validator_reply),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let codex_home = Arc::new(TempDir::new().unwrap());
    let codex = new_conversation_for_server(&server, codex_home.clone(), |_| {}).await;

    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Reject wrapped validator output".to_string(),
                },
                user_facing_hint: None,
                validate_findings: true,
            },
        })
        .await
        .unwrap();

    let _entered = wait_for_event(&codex, |ev| matches!(ev, EventMsg::EnteredReviewMode(_))).await;
    let closed = wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExitedReviewMode(_))).await;
    let failure_message = match closed {
        EventMsg::ExitedReviewMode(ev) => {
            assert_eq!(ev.review_output, None);
            ev.failure_message
                .expect("expected explicit failure message")
        }
        other => panic!("expected ExitedReviewMode(..), got {other:?}"),
    };
    assert_eq!(
        failure_message,
        "Review findings validation did not complete cleanly, so no findings were surfaced."
    );
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let rollout_path = codex.rollout_path().expect("rollout path");
    let rollout = std::fs::read_to_string(&rollout_path).expect("read rollout file");
    assert!(
        rollout.contains("but was interrupted"),
        "wrapped validator output should persist as an interrupted review"
    );
    assert!(
        !rollout.contains("Here is the validated output:"),
        "wrapped validator reply should not be persisted as validated review output"
    );

    let _codex_home_guard = codex_home;
    server.verify().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_validation_realigns_verdict_when_no_findings_remain() {
    skip_if_no_network!();

    let initial_review_json = serde_json::json!({
        "findings": [
            {
                "title": "Discard me",
                "body": "This finding should be dropped by validation.",
                "confidence_score": 0.8,
                "priority": 1,
                "code_location": {
                    "absolute_file_path": "/tmp/discard-me.rs",
                    "line_range": {"start": 3, "end": 4}
                }
            }
        ],
        "overall_correctness": "incorrect",
        "overall_explanation": "Initial review output before verdict realignment.",
        "overall_confidence_score": 0.8
    })
    .to_string();
    let validator_json = serde_json::json!({
        "findings": [],
        "overall_correctness": "patch is incorrect",
        "overall_explanation": "Everything was discarded during validation.",
        "overall_confidence_score": 0.84
    })
    .to_string();

    let server = start_mock_server().await;
    let _request_log = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg-1", &initial_review_json),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", &validator_json),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let codex_home = Arc::new(TempDir::new().unwrap());
    let codex = new_conversation_for_server(&server, codex_home.clone(), |_| {}).await;

    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Realign review verdict".to_string(),
                },
                user_facing_hint: None,
                validate_findings: true,
            },
        })
        .await
        .unwrap();

    let _entered = wait_for_event(&codex, |ev| matches!(ev, EventMsg::EnteredReviewMode(_))).await;
    let closed = wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExitedReviewMode(_))).await;
    let review = match closed {
        EventMsg::ExitedReviewMode(ev) => ev
            .review_output
            .expect("expected ExitedReviewMode with Some(review_output)"),
        other => panic!("expected ExitedReviewMode(..), got {other:?}"),
    };
    assert!(review.findings.is_empty());
    assert_eq!(review.overall_correctness, "patch is correct");
    assert_eq!(
        review.overall_explanation,
        "Everything was discarded during validation."
    );
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let _codex_home_guard = codex_home;
    server.verify().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_validation_normalizes_zero_finding_first_pass_output() {
    skip_if_no_network!();

    let review_json = serde_json::json!({
        "findings": [],
        "overall_correctness": "patch is incorrect",
        "overall_explanation": "",
        "overall_confidence_score": 0.2
    })
    .to_string();

    let sse_raw = r#"[
        {"type":"response.output_item.done", "item":{
            "type":"message", "role":"assistant",
            "content":[{"type":"output_text","text":__REVIEW__}]
        }},
        {"type":"response.completed", "response": {"id": "__ID__"}}
    ]"#
    .replace("__REVIEW__", &serde_json::to_string(&review_json).unwrap());
    let (server, _request_log) = start_responses_server_with_sse(&sse_raw, 1).await;
    let codex_home = Arc::new(TempDir::new().unwrap());
    let codex = new_conversation_for_server(&server, codex_home.clone(), |_| {}).await;

    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Normalize zero-finding review output".to_string(),
                },
                user_facing_hint: None,
                validate_findings: true,
            },
        })
        .await
        .unwrap();

    let _entered = wait_for_event(&codex, |ev| matches!(ev, EventMsg::EnteredReviewMode(_))).await;
    let closed = wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExitedReviewMode(_))).await;
    let review = match closed {
        EventMsg::ExitedReviewMode(ev) => ev
            .review_output
            .expect("expected ExitedReviewMode with Some(review_output)"),
        other => panic!("expected ExitedReviewMode(..), got {other:?}"),
    };

    assert_eq!(
        review,
        ReviewOutputEvent {
            findings: Vec::new(),
            overall_correctness: "patch is correct".to_string(),
            overall_explanation: "No findings.".to_string(),
            overall_confidence_score: 0.2,
        }
    );
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let _codex_home_guard = codex_home;
    server.verify().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_validation_prompt_includes_original_review_scope() {
    skip_if_no_network!();

    let initial_review_json = serde_json::json!({
        "findings": [
            {
                "title": "Needs scope",
                "body": "The validator should receive the original review scope.\n```rust\nlet value = 1;\n```",
                "confidence_score": 0.85,
                "priority": 1,
                "code_location": {
                    "absolute_file_path": "/tmp/original-scope.rs",
                    "line_range": {"start": 30, "end": 31}
                }
            }
        ],
        "overall_correctness": "incorrect",
        "overall_explanation": "Initial review output before scope-aware validation.",
        "overall_confidence_score": 0.85
    })
    .to_string();
    let validated_json = serde_json::json!({
        "findings": [],
        "overall_correctness": "patch is correct",
        "overall_explanation": "No findings remain after validating against the commit scope.",
        "overall_confidence_score": 0.9
    })
    .to_string();

    let server = start_mock_server().await;
    let request_log = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg-1", &initial_review_json),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", &validated_json),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let codex_home = Arc::new(TempDir::new().unwrap());
    let codex = new_conversation_for_server(&server, codex_home.clone(), |_| {}).await;

    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Commit {
                    sha: "abc1234".to_string(),
                    title: Some("Add review loop".to_string()),
                },
                user_facing_hint: None,
                validate_findings: true,
            },
        })
        .await
        .unwrap();

    let _entered = wait_for_event(&codex, |ev| matches!(ev, EventMsg::EnteredReviewMode(_))).await;
    let _closed = wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExitedReviewMode(_))).await;
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = request_log.requests();
    assert_eq!(requests.len(), 2);
    let validator_request = requests[1].body_json();
    let validator_instructions = validator_request["instructions"]
        .as_str()
        .expect("validator instructions string");
    assert_ne!(
        validator_instructions, REVIEW_PROMPT,
        "validator should not run under the full review system prompt"
    );
    assert!(
        validator_instructions.contains("Do not perform a fresh review."),
        "validator instructions should use the dedicated validation prompt"
    );
    let validator_prompt = requests[1]
        .input()
        .iter()
        .filter_map(|msg| msg.get("content").and_then(|content| content.as_array()))
        .flat_map(|content| content.iter())
        .filter_map(|entry| entry.get("text").and_then(|text| text.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        validator_prompt
            .contains("\"original_review_target_summary\": \"commit abc1234: Add review loop\""),
        "validator prompt missing original review target summary"
    );
    assert!(
        validator_prompt.contains("\"original_review_scope\":"),
        "validator prompt missing original review scope instructions"
    );
    assert!(
        validator_prompt
            .contains("Review the code changes introduced by commit abc1234 (\\\"Add review loop\\\"). Provide prioritized, actionable findings."),
        "validator prompt missing escaped original review scope text"
    );
    assert!(
        validator_prompt
            .contains("Do not introduce any new finding that was not already present in the original review output."),
        "validator prompt should forbid introducing brand-new findings"
    );
    assert!(
        validator_prompt
            .contains("Each surviving finding must match one original finding exactly by `title`, `priority`, and `code_location`."),
        "validator prompt should require stable finding identity"
    );
    assert!(
        validator_prompt.contains("```rust\\nlet value = 1;\\n```"),
        "validator prompt should preserve review content containing code fences"
    );
    assert!(
        !validator_prompt.contains("```text"),
        "validator prompt should not wrap review scope in hard-coded text fences"
    );
    assert!(
        !validator_prompt.contains("```json"),
        "validator prompt should not wrap review output in hard-coded json fences"
    );

    let _codex_home_guard = codex_home;
    server.verify().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_validation_can_request_user_input_before_exiting_review_mode() {
    skip_if_no_network!();

    let call_id = "review-validation-question";
    let initial_review_json = serde_json::json!({
        "findings": [
            {
                "title": "Needs clarification",
                "body": "The validator should ask whether this finding is still relevant.",
                "confidence_score": 0.7,
                "priority": 1,
                "code_location": {
                    "absolute_file_path": "/tmp/needs-clarification.rs",
                    "line_range": {"start": 7, "end": 8}
                }
            }
        ],
        "overall_correctness": "incorrect",
        "overall_explanation": "Initial review output before user clarification.",
        "overall_confidence_score": 0.7
    })
    .to_string();
    let request_args = serde_json::json!({
        "questions": [{
            "id": "finding_status",
            "header": "Finding",
            "question": "Does this finding still apply after the refactor?",
            "options": [{
                "label": "No (Recommended)",
                "description": "The finding is outdated and should be discarded."
            }, {
                "label": "Yes",
                "description": "The finding still applies."
            }]
        }]
    })
    .to_string();
    let validated_output = ReviewOutputEvent {
        findings: Vec::new(),
        overall_correctness: "patch is correct".to_string(),
        overall_explanation: "The user confirmed the finding is outdated, so it was discarded."
            .to_string(),
        overall_confidence_score: 0.88,
    };
    let validated_json = serde_json::to_string(&validated_output).expect("validated json");

    let server = start_mock_server().await;
    let request_log = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg-1", &initial_review_json),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_function_call(call_id, "request_user_input", &request_args),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_assistant_message("msg-2", &validated_json),
                ev_completed("resp-3"),
            ]),
        ],
    )
    .await;

    let codex_home = Arc::new(TempDir::new().unwrap());
    let codex = new_conversation_for_server(&server, codex_home.clone(), |_| {}).await;

    codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            windows_sandbox_level: None,
            model: None,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: Some(CollaborationMode {
                mode: ModeKind::Execute,
                settings: Settings {
                    model: "gpt-5.1".to_string(),
                    reasoning_effort: None,
                    developer_instructions: None,
                },
            }),
            personality: None,
        })
        .await
        .unwrap();

    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Validate review findings with clarification".to_string(),
                },
                user_facing_hint: None,
                validate_findings: true,
            },
        })
        .await
        .unwrap();

    let _entered = wait_for_event(&codex, |ev| matches!(ev, EventMsg::EnteredReviewMode(_))).await;
    let request = wait_for_event(&codex, |ev| matches!(ev, EventMsg::RequestUserInput(_))).await;
    let request = match request {
        EventMsg::RequestUserInput(request) => request,
        other => panic!("expected RequestUserInput(..), got {other:?}"),
    };
    assert_eq!(request.call_id, call_id);
    assert_eq!(request.questions.len(), 1);

    let mut answers = HashMap::new();
    answers.insert(
        "finding_status".to_string(),
        RequestUserInputAnswer {
            answers: vec!["outdated".to_string()],
        },
    );
    codex
        .submit(Op::UserInputAnswer {
            id: request.turn_id.clone(),
            response: RequestUserInputResponse { answers },
        })
        .await
        .unwrap();

    let closed = wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExitedReviewMode(_))).await;
    let review = match closed {
        EventMsg::ExitedReviewMode(ev) => ev
            .review_output
            .expect("expected ExitedReviewMode with Some(review_output)"),
        other => panic!("expected ExitedReviewMode(..), got {other:?}"),
    };
    assert_eq!(validated_output, review);
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = request_log.requests();
    assert_eq!(requests.len(), 3);
    let output_text = requests[2]
        .function_call_output_text(call_id)
        .expect("request_user_input output should be included");
    let output_json: serde_json::Value =
        serde_json::from_str(&output_text).expect("valid request_user_input output json");
    assert_eq!(
        output_json,
        serde_json::json!({
            "answers": {
                "finding_status": { "answers": ["outdated"] }
            }
        })
    );

    let _codex_home_guard = codex_home;
    server.verify().await;
}

/// `/review` should use the session's current cwd (including runtime overrides)
/// when resolving base-branch review prompts (merge-base computation).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_uses_overridden_cwd_for_base_branch_merge_base() {
    skip_if_no_network!();

    let sse_raw = r#"[{"type":"response.completed", "response": {"id": "__ID__"}}]"#;
    let (server, request_log) = start_responses_server_with_sse(sse_raw, 1).await;

    let initial_cwd = TempDir::new().unwrap();

    let repo_dir = TempDir::new().unwrap();
    let repo_path = repo_dir.path();

    fn run_git(repo_path: &std::path::Path, args: &[&str]) {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .args(args)
            .output()
            .expect("spawn git");
        assert!(
            output.status.success(),
            "git {:?} failed: stdout={:?} stderr={:?}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    run_git(repo_path, &["init", "-b", "main"]);
    run_git(repo_path, &["config", "user.email", "test@example.com"]);
    run_git(repo_path, &["config", "user.name", "Test User"]);
    std::fs::write(repo_path.join("file.txt"), "hello\n").unwrap();
    run_git(repo_path, &["add", "."]);
    run_git(repo_path, &["commit", "-m", "initial"]);

    let head_sha = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("rev-parse HEAD");
    assert!(head_sha.status.success());
    let head_sha = String::from_utf8(head_sha.stdout)
        .expect("utf8 sha")
        .trim()
        .to_string();

    let codex_home = Arc::new(TempDir::new().unwrap());
    let initial_cwd_path = initial_cwd.path().to_path_buf();
    let codex = new_conversation_for_server(&server, codex_home.clone(), move |config| {
        config.cwd = initial_cwd_path;
    })
    .await;

    codex
        .submit(Op::OverrideTurnContext {
            cwd: Some(repo_path.to_path_buf()),
            approval_policy: None,
            sandbox_policy: None,
            windows_sandbox_level: None,
            model: None,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await
        .unwrap();

    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::BaseBranch {
                    branch: "main".to_string(),
                },
                user_facing_hint: None,
                validate_findings: false,
            },
        })
        .await
        .unwrap();

    let _entered = wait_for_event(&codex, |ev| matches!(ev, EventMsg::EnteredReviewMode(_))).await;
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = request_log.requests();
    assert_eq!(requests.len(), 1);
    for request in &requests {
        assert_eq!(request.path(), "/v1/responses");
    }
    let body = requests[0].body_json();
    let input = body["input"].as_array().expect("input array");

    let saw_merge_base_sha = input
        .iter()
        .filter_map(|msg| msg["content"][0]["text"].as_str())
        .any(|text| text.contains(&head_sha));
    assert!(
        saw_merge_base_sha,
        "expected review prompt to include merge-base sha {head_sha}"
    );

    let _codex_home_guard = codex_home;
    server.verify().await;
}

/// Start a mock Responses API server and mount the given SSE stream body.
async fn start_responses_server_with_sse(
    sse_raw: &str,
    expected_requests: usize,
) -> (MockServer, ResponseMock) {
    let server = start_mock_server().await;
    let sse = load_sse_fixture_with_id_from_str(sse_raw, &Uuid::new_v4().to_string());
    let responses = vec![sse; expected_requests];
    let request_log = mount_sse_sequence(&server, responses).await;
    (server, request_log)
}

/// Create a conversation configured to talk to the provided mock server.
#[expect(clippy::expect_used)]
async fn new_conversation_for_server<F>(
    server: &MockServer,
    codex_home: Arc<TempDir>,
    mutator: F,
) -> Arc<CodexThread>
where
    F: FnOnce(&mut Config) + Send + 'static,
{
    let base_url = format!("{}/v1", server.uri());
    let mut builder = test_codex()
        .with_home(codex_home)
        .with_config(move |config| {
            config.model_provider.base_url = Some(base_url.clone());
            mutator(config);
        });
    builder
        .build(server)
        .await
        .expect("create conversation")
        .codex
}

/// Create a conversation resuming from a rollout file, configured to talk to the provided mock server.
#[expect(clippy::expect_used)]
async fn resume_conversation_for_server<F>(
    server: &MockServer,
    codex_home: Arc<TempDir>,
    resume_path: std::path::PathBuf,
    mutator: F,
) -> Arc<CodexThread>
where
    F: FnOnce(&mut Config) + Send + 'static,
{
    let base_url = format!("{}/v1", server.uri());
    let mut builder = test_codex()
        .with_home(codex_home.clone())
        .with_config(move |config| {
            config.model_provider.base_url = Some(base_url.clone());
            mutator(config);
        });
    builder
        .resume(server, codex_home, resume_path)
        .await
        .expect("resume conversation")
        .codex
}
