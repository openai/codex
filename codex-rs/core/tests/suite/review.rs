use codex_core::CodexAuth;
use codex_core::CodexConversation;
use codex_core::ContentItem;
use codex_core::ConversationManager;
use codex_core::ModelProviderInfo;
use codex_core::REVIEW_PROMPT;
use codex_core::ResponseItem;
use codex_core::built_in_model_providers;
use codex_core::config::Config;
use codex_core::protocol::ConversationPathResponseEvent;
use codex_core::protocol::ENVIRONMENT_CONTEXT_OPEN_TAG;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExitedReviewModeEvent;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::protocol::ReviewCodeLocation;
use codex_core::protocol::ReviewFinding;
use codex_core::protocol::ReviewLineRange;
use codex_core::protocol::ReviewOutputEvent;
use codex_core::protocol::ReviewRequest;
use codex_core::protocol::RolloutItem;
use codex_core::protocol::RolloutLine;
use core_test_support::load_default_config_for_test;
use core_test_support::load_sse_fixture_with_id_from_str;
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
                prompt: "Please review my changes".to_string(),
                user_facing_hint: "my changes".to_string(),
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
    codex.submit(Op::GetPath).await.unwrap();
    let history_event =
        wait_for_event(&codex, |ev| matches!(ev, EventMsg::ConversationPath(_))).await;
    let path = match history_event {
        EventMsg::ConversationPath(ConversationPathResponseEvent { path, .. }) => path,
        other => panic!("expected ConversationPath event, got {other:?}"),
    };
    let text = std::fs::read_to_string(&path).expect("read rollout file");

    let mut saw_header = false;
    let mut saw_finding_line = false;
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
                if let ContentItem::InputText { text } = c {
                    if text.contains("full review output from reviewer model") {
                        saw_header = true;
                    }
                    if text.contains("- Prefer Stylize helpers — /tmp/file.rs:10-20") {
                        saw_finding_line = true;
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
                prompt: "Plain text review".to_string(),
                user_facing_hint: "plain text review".to_string(),
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

/// When the model returns structured JSON in a review, ensure no AgentMessage
/// is emitted; the UI consumes the structured result via ExitedReviewMode.
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
                prompt: "check structured".to_string(),
                user_facing_hint: "check structured".to_string(),
            },
        })
        .await
        .unwrap();

    // Drain events until TaskComplete; ensure none are AgentMessage.
    use tokio::time::Duration;
    use tokio::time::timeout;
    let mut saw_entered = false;
    let mut saw_exited = false;
    loop {
        let ev = timeout(Duration::from_secs(5), codex.next_event())
            .await
            .expect("timeout waiting for event")
            .expect("stream ended unexpectedly");
        match ev.msg {
            EventMsg::TaskComplete(_) => break,
            EventMsg::AgentMessage(_) => {
                panic!("unexpected AgentMessage during review with structured output")
            }
            EventMsg::EnteredReviewMode(_) => saw_entered = true,
            EventMsg::ExitedReviewMode(_) => saw_exited = true,
            _ => {}
        }
    }
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
        cfg.model = "gpt-4.1".to_string();
        cfg.review_model = "gpt-5".to_string();
    })
    .await;

    codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                prompt: "use custom model".to_string(),
                user_facing_hint: "use custom model".to_string(),
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
    let request = &server.received_requests().await.unwrap()[0];
    let body = request.body_json::<serde_json::Value>().unwrap();
    assert_eq!(body["model"].as_str().unwrap(), "gpt-5");

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
    let mut config = load_default_config_for_test(&codex_home);
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
                "cli_version": "test_version"
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
                prompt: review_prompt.clone(),
                user_facing_hint: review_prompt.clone(),
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

    // Assert the request `input` contains the environment context followed by the review prompt.
    let request = &server.received_requests().await.unwrap()[0];
    let body = request.body_json::<serde_json::Value>().unwrap();
    let input = body["input"].as_array().expect("input array");
    assert_eq!(
        input.len(),
        2,
        "expected environment context and review prompt"
    );

    let env_msg = &input[0];
    assert_eq!(env_msg["type"].as_str().unwrap(), "message");
    assert_eq!(env_msg["role"].as_str().unwrap(), "user");
    let env_text = env_msg["content"][0]["text"].as_str().expect("env text");
    assert!(
        env_text.starts_with(ENVIRONMENT_CONTEXT_OPEN_TAG),
        "environment context must be the first item"
    );
    assert!(
        env_text.contains("<cwd>"),
        "environment context should include cwd"
    );

    let review_msg = &input[1];
    assert_eq!(review_msg["type"].as_str().unwrap(), "message");
    assert_eq!(review_msg["role"].as_str().unwrap(), "user");
    assert_eq!(
        review_msg["content"][0]["text"].as_str().unwrap(),
        format!("{REVIEW_PROMPT}\n\n---\n\nNow, here's your task: Please review only this",)
    );

    // Also verify that a user interruption note was recorded in the rollout.
    codex.submit(Op::GetPath).await.unwrap();
    let history_event =
        wait_for_event(&codex, |ev| matches!(ev, EventMsg::ConversationPath(_))).await;
    let path = match history_event {
        EventMsg::ConversationPath(ConversationPathResponseEvent { path, .. }) => path,
        other => panic!("expected ConversationPath event, got {other:?}"),
    };
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

/// After a review thread finishes, its conversation should not leak into the
/// parent session. A subsequent parent turn must not include any review
/// messages in its request `input`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_history_does_not_leak_into_parent_session() {
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
                prompt: "Start a review".to_string(),
                user_facing_hint: "Start a review".to_string(),
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
            items: vec![InputItem::Text {
                text: followup.clone(),
            }],
        })
        .await
        .unwrap();
    let _complete = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Inspect the second request (parent turn) input contents.
    // Parent turns include session initial messages (user_instructions, environment_context).
    // Critically, no messages from the review thread should appear.
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2);
    let body = requests[1].body_json::<serde_json::Value>().unwrap();
    let input = body["input"].as_array().expect("input array");

    // Must include the followup as the last item for this turn
    let last = input.last().expect("at least one item in input");
    assert_eq!(last["role"].as_str().unwrap(), "user");
    let last_text = last["content"][0]["text"].as_str().unwrap();
    assert_eq!(last_text, followup);

    // Ensure no review-thread content leaked into the parent request
    let contains_review_prompt = input
        .iter()
        .any(|msg| msg["content"][0]["text"].as_str().unwrap_or_default() == "Start a review");
    let contains_review_assistant = input.iter().any(|msg| {
        msg["content"][0]["text"].as_str().unwrap_or_default() == "review assistant output"
    });
    assert!(
        !contains_review_prompt,
        "review prompt leaked into parent turn input"
    );
    assert!(
        !contains_review_assistant,
        "review assistant output leaked into parent turn input"
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
    let mut config = load_default_config_for_test(codex_home);
    config.model_provider = model_provider;
    mutator(&mut config);
    let conversation_manager =
        ConversationManager::with_auth(CodexAuth::from_api_key("Test API Key"));
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
    let mut config = load_default_config_for_test(codex_home);
    config.model_provider = model_provider;
    mutator(&mut config);
    let conversation_manager =
        ConversationManager::with_auth(CodexAuth::from_api_key("Test API Key"));
    let auth_manager =
        codex_core::AuthManager::from_auth_for_testing(CodexAuth::from_api_key("Test API Key"));
    conversation_manager
        .resume_conversation_from_rollout(config, resume_path, auth_manager)
        .await
        .expect("resume conversation")
        .conversation
}
