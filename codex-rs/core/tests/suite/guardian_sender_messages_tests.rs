//! Genuine sender context stays reviewer-only and is replaced on every delivery.

use anyhow::Result;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_core::config::Constrained;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::turn_input::TurnInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test_case::test_case(None, "codex_app", "send_message_to_thread", 0; "default")]
#[test_case::test_case(Some(false), "codex_app", "send_message_to_thread", 0; "retired opt out")]
#[test_case::test_case(None, "codex_tui", "send_message_to_thread", 0; "tui")]
#[test_case::test_case(None, "cloud_threads", "send_message", 0; "cloud threads")]
#[test_case::test_case(None, "cloud_threads", "send_message", 850; "assistant context shares user budget")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_receives_sender_user_messages_by_default(
    thread_context: Option<bool>,
    namespace: &str,
    name: &str,
    user_padding: usize,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let test = test_codex()
        .with_pre_build_hook(move |home| {
            if let Some(enabled) = thread_context {
                std::fs::write(
                    home.join("config.toml"),
                    format!("[features.guardianv2]\nthread_context = {enabled}\n"),
                )
                .expect("write compatibility configuration");
            }
        })
        .with_model_info_override("gpt-5.5", |model| {
            model.auto_review_model_override = Some("gpt-5.6-luna".to_owned());
        })
        .with_config(|config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
        })
        .build_with_auto_env(&server)
        .await?;
    let sender = test.session_configured.thread_id;
    let receiver = test
        .thread_manager
        .start_thread(StartThreadOptions {
            environments: Some(test.codex.environment_selections().await),
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?
        .thread;
    let done = || responses::sse(vec![responses::ev_completed("done")]);
    let mut requests = Vec::new();
    let mut expected_messages = Vec::new();
    let mut previous_reply = None;
    for (index, (prompt, reply)) in [
        ("OLDEST", "I can inspect the experiment."),
        (
            "Inspect the experiment.",
            "Rerun only the staging task?\nPreserve its checkpoints?",
        ),
        ("👍", "I will use staging."),
        ("Only use staging.\nNever production.", "LATER CONTEXT"),
    ]
    .into_iter()
    .enumerate()
    {
        let prompt = format!("{prompt}{}", "x".repeat(user_padding));
        let mock = responses::mount_sse_once(
            &server,
            responses::sse(vec![
                responses::ev_assistant_message(&format!("reply-{index}"), reply),
                responses::ev_completed("done"),
            ]),
        )
        .await;
        test.submit_text_turn(&prompt).await?;
        requests.extend(mock.requests());
        if index > 0 {
            if user_padding == 0
                && let Some(reply) = previous_reply
            {
                expected_messages
                    .extend(str::lines(reply).map(|line| format!("assistant: {line}")));
            }
            expected_messages.extend(prompt.lines().map(|line| format!("user: {line}")));
        }
        previous_reply = Some(reply);
    }
    let delegation = if namespace == "cloud_threads" {
        // The cloud producer serializes compact XML, while Desktop/TUI indent it.
        format!(
            "<codex_delegation><source_thread_id>{sender}</source_thread_id><input>Inspect.</input></codex_delegation>"
        )
    } else {
        format!(
            "<codex_delegation>\n  <source_thread_id>{sender}</source_thread_id>\n  <input>Inspect.</input>\n</codex_delegation>"
        )
    };
    for (index, (output, expected)) in [
        (delegation, expected_messages),
        (
            "Inspect again without sender provenance.".to_owned(),
            vec![],
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let command = json!({
            "cmd": "exit 0",
            "sandbox_permissions": "require_escalated",
            "justification": "Inspect the staging experiment.",
        })
        .to_string();
        let mock = responses::mount_sse_sequence(
            &server,
            vec![
                responses::sse(vec![
                    responses::ev_function_call("inspect", "exec_command", &command),
                    responses::ev_completed("action"),
                ]),
                responses::sse(vec![
                    responses::ev_assistant_message(
                        "decision",
                        r#"{"risk_level":"high","user_authorization":"unknown","outcome":"deny"}"#,
                    ),
                    responses::ev_completed("review"),
                ]),
                done(),
            ],
        )
        .await;
        let delivery: ResponseItem = serde_json::from_value(json!({
            "type": "function_call_output",
            "id": format!("delivery-{index}"),
            "name": name,
            "namespace": namespace,
            "output": output,
        }))?;
        receiver
            .start_or_steer_turn(TurnInputRequest::new(TurnInput::ResponseItem(delivery)))
            .await?;
        wait_for_event(&receiver, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
        let captured = mock.requests();
        assert_eq!(captured.len(), 3);
        // Original user and assistant context stays out of the delegated agent's prompt.
        let worker_request = captured[0].body_json().to_string();
        assert!(!worker_request.contains("Never production."));
        assert!(!worker_request.contains("Rerun only the staging task?"));
        let review_body = captured[1].body_json();
        let review = review_body["input"]
            .as_array()
            .expect("review input")
            .iter()
            .filter_map(|item| item["content"].as_array())
            .flatten()
            .filter_map(|item| item["text"].as_str())
            .collect::<String>();
        assert!(review.contains("SENDER USER MESSAGES START"));
        let history = receiver.conversation_history_snapshot().await;
        let snapshot = history
            .retained_context()
            .expect("thread-owned context")
            .sender_user_messages()
            .expect("sender user messages");
        assert_eq!(
            snapshot
                .text
                .lines()
                .filter(|line| line.starts_with("user: ") || line.starts_with("assistant: "))
                .collect::<Vec<_>>(),
            expected
        );
        assert!(snapshot.text.len() <= 3_600);
        assert_eq!(
            snapshot
                .text
                .contains("some original assistant context is unavailable"),
            index == 0 && user_padding > 0
        );
        let start = ">>> SENDER USER MESSAGES START\n";
        let end = ">>> SENDER USER MESSAGES END\n";
        let current = review
            .rsplit_once(start)
            .expect("sender context start")
            .1
            .split(end)
            .next()
            .expect("sender context body");
        assert_eq!(format!("{start}{current}{end}"), snapshot.text);
        requests.extend(captured);
    }
    assert_eq!(requests.len(), 10);
    Ok(())
}
