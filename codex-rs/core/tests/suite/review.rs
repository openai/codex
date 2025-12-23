use codex_core::CodexAuth;
use codex_core::CodexConversation;
use codex_core::ContentItem;
use codex_core::ConversationManager;
use codex_core::ModelProviderInfo;
use codex_core::REVIEW_PROMPT;
use codex_core::ResponseItem;
use codex_core::built_in_model_providers;
use codex_core::config::Config;
use codex_core::protocol::ENVIRONMENT_CONTEXT_OPEN_TAG;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExitedReviewModeEvent;
use codex_core::protocol::Op;
use codex_core::protocol::ReviewCodeLocation;
use codex_core::protocol::ReviewFinding;
use codex_core::protocol::ReviewLineRange;
use codex_core::protocol::ReviewOutputEvent;
use codex_core::protocol::ReviewRequest;
use codex_core::protocol::ReviewTarget;
use codex_core::protocol::RolloutItem;
use codex_core::protocol::RolloutLine;
use codex_core::review_format::render_review_output_text;
use codex_protocol::user_input::UserInput;
use core_test_support::load_default_config_for_test;
use core_test_support::load_sse_fixture_with_id_from_str;
use core_test_support::responses::get_responses_requests;
use core_test_support::skip_if_no_network;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::io::AsyncWriteExt as _;
use uuid::Uuid;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

/// Verify that submitting `Op::Review` spawns a child task and emits
/// EnteredReviewMode -> ExitedReviewMode(None) -> TaskComplete
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
    let server = start_responses_server_with_sse(&sse_raw, 1).await;
    let codex_home = TempDir::new().unwrap();
    let codex = new_conversation_for_server(&server, &codex_home, |_| {}).await;

    // Submit review request.
    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Please review my changes".to_string(),
                },
                user_facing_hint: None,
            },
        })
        .await
        .unwrap();

    // Verify lifecycle: Entered -> Exited(Some(review)) -> TaskComplete.
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
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Also verify that a user message with the header and a formatted finding
    // was recorded back in the parent session's rollout.
    let path = codex.rollout_path();
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
    let server = start_responses_server_with_sse(sse_raw, 1).await;
    let codex_home = TempDir::new().unwrap();
    let codex = new_conversation_for_server(&server, &codex_home, |_| {}).await;

    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Plain text review".to_string(),
                },
                user_facing_hint: None,
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
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

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
    let server = start_responses_server_with_sse(sse_raw, 1).await;
    let codex_home = TempDir::new().unwrap();
    let codex = new_conversation_for_server(&server, &codex_home, |_| {}).await;

    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Filter streaming events".to_string(),
                },
                user_facing_hint: None,
            },
        })
        .await
        .unwrap();

    let mut saw_entered = false;
    let mut saw_exited = false;

    // Drain until TaskComplete; assert streaming-related events never surface.
    wait_for_event(&codex, |event| match event {
        EventMsg::TaskComplete(_) => true,
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
    let server = start_responses_server_with_sse(&sse_raw, 1).await;
    let codex_home = TempDir::new().unwrap();
    let codex = new_conversation_for_server(&server, &codex_home, |_| {}).await;

    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "check structured".to_string(),
                },
                user_facing_hint: None,
            },
        })
        .await
        .unwrap();

    // Drain events until TaskComplete; ensure we only see a final
    // AgentMessage (no streaming assistant messages).
    let mut saw_entered = false;
    let mut saw_exited = false;
    let mut agent_messages = 0;
    wait_for_event(&codex, |event| match event {
        EventMsg::TaskComplete(_) => true,
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
    let server = start_responses_server_with_sse(sse_raw, 1).await;
    let codex_home = TempDir::new().unwrap();
    // Choose a review model different from the main model; ensure it is used.
    let codex = new_conversation_for_server(&server, &codex_home, |cfg| {
        cfg.model = Some("gpt-4.1".to_string());
        cfg.review_model = "gpt-5.1".to_string();
    })
    .await;

    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "use custom model".to_string(),
                },
                user_facing_hint: None,
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
                review_output: None
            })
        )
    })
    .await;
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Assert the request body model equals the configured review model
    let requests = get_responses_requests(&server).await;
    let request = requests
        .first()
        .expect("expected POST request to /responses");
    let body = request.body_json::<serde_json::Value>().unwrap();
    assert_eq!(body["model"].as_str().unwrap(), "gpt-5.1");

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
    let server = start_responses_server_with_sse(sse_raw, 1).await;

    // Seed a parent session history via resume file with both user + assistant items.
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home).await;
    config.model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

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
                "instructions": null,
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
        resume_conversation_for_server(&server, &codex_home, session_file.clone(), |_| {}).await;

    // Submit review request; it must start fresh (no parent history in `input`).
    let review_prompt = "Please review only this".to_string();
    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: review_prompt.clone(),
                },
                user_facing_hint: None,
            },
        })
        .await
        .unwrap();

    let _entered = wait_for_event(&codex, |ev| matches!(ev, EventMsg::EnteredReviewMode(_))).await;
    let _closed = wait_for_event(&codex, |ev| {
        matches!(
            ev,
            EventMsg::ExitedReviewMode(ExitedReviewModeEvent {
                review_output: None
            })
        )
    })
    .await;
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Assert the request `input` contains the environment context followed by the user review prompt.
    let requests = get_responses_requests(&server).await;
    let request = requests
        .first()
        .expect("expected POST request to /responses");
    let body = request.body_json::<serde_json::Value>().unwrap();
    let input = body["input"].as_array().expect("input array");
    assert!(
        input.len() >= 2,
        "expected at least environment context and review prompt"
    );

    let env_text = input
        .iter()
        .filter_map(|msg| msg["content"][0]["text"].as_str())
        .find(|text| text.starts_with(ENVIRONMENT_CONTEXT_OPEN_TAG))
        .expect("env text");
    assert!(
        env_text.contains("<cwd>"),
        "environment context should include cwd"
    );

    let review_text = input
        .iter()
        .filter_map(|msg| msg["content"][0]["text"].as_str())
        .find(|text| *text == review_prompt)
        .expect("review prompt text");
    assert_eq!(
        review_text, review_prompt,
        "user message should only contain the raw review prompt"
    );

    // Ensure the REVIEW_PROMPT rubric is sent via instructions.
    let instructions = body["instructions"].as_str().expect("instructions string");
    assert_eq!(instructions, REVIEW_PROMPT);

    // Also verify that a user interruption note was recorded in the rollout.
    let path = codex.rollout_path();
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
    let server = start_responses_server_with_sse(sse_raw, 2).await;
    let codex_home = TempDir::new().unwrap();
    let codex = new_conversation_for_server(&server, &codex_home, |_| {}).await;

    // 1) Run a review turn that produces an assistant message (isolated in child).
    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "Start a review".to_string(),
                },
                user_facing_hint: None,
            },
        })
        .await
        .unwrap();
    let _entered = wait_for_event(&codex, |ev| matches!(ev, EventMsg::EnteredReviewMode(_))).await;
    let _closed = wait_for_event(&codex, |ev| {
        matches!(
            ev,
            EventMsg::ExitedReviewMode(ExitedReviewModeEvent {
                review_output: Some(_)
            })
        )
    })
    .await;
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // 2) Continue in the parent session; request input must not include any review items.
    let followup = "back to parent".to_string();
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: followup.clone(),
            }],
        })
        .await
        .unwrap();
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Inspect the second request (parent turn) input contents.
    // Parent turns include session initial messages (user_instructions, environment_context).
    // Critically, no messages from the review thread should appear.
    let requests = get_responses_requests(&server).await;
    assert_eq!(requests.len(), 2);
    let body = requests[1].body_json::<serde_json::Value>().unwrap();
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

    server.verify().await;
}

/// Start a mock Responses API server and mount the given SSE stream body.
async fn start_responses_server_with_sse(sse_raw: &str, expected_requests: usize) -> MockServer {
    let server = MockServer::start().await;
    let sse = load_sse_fixture_with_id_from_str(sse_raw, &Uuid::new_v4().to_string());
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_raw(sse.clone(), "text/event-stream"),
        )
        .expect(expected_requests as u64)
        .mount(&server)
        .await;
    server
}

/// Create a conversation configured to talk to the provided mock server.
#[expect(clippy::expect_used)]
async fn new_conversation_for_server<F>(
    server: &MockServer,
    codex_home: &TempDir,
    mutator: F,
) -> Arc<CodexConversation>
where
    F: FnOnce(&mut Config),
{
    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };
    let mut config = load_default_config_for_test(codex_home).await;
    config.model_provider = model_provider;
    mutator(&mut config);
    let conversation_manager = ConversationManager::with_models_provider(
        CodexAuth::from_api_key("Test API Key"),
        config.model_provider.clone(),
    );
    conversation_manager
        .new_conversation(config)
        .await
        .expect("create conversation")
        .conversation
}

/// Create a conversation resuming from a rollout file, configured to talk to the provided mock server.
#[expect(clippy::expect_used)]
async fn resume_conversation_for_server<F>(
    server: &MockServer,
    codex_home: &TempDir,
    resume_path: std::path::PathBuf,
    mutator: F,
) -> Arc<CodexConversation>
where
    F: FnOnce(&mut Config),
{
    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };
    let mut config = load_default_config_for_test(codex_home).await;
    config.model_provider = model_provider;
    mutator(&mut config);
    let conversation_manager = ConversationManager::with_models_provider(
        CodexAuth::from_api_key("Test API Key"),
        config.model_provider.clone(),
    );
    let auth_manager =
        codex_core::AuthManager::from_auth_for_testing(CodexAuth::from_api_key("Test API Key"));
    conversation_manager
        .resume_conversation_from_rollout(config, resume_path, auth_manager)
        .await
        .expect("resume conversation")
        .conversation
}
