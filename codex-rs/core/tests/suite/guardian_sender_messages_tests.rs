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

#[test_case::test_case(None; "default")]
#[test_case::test_case(Some(false); "retired opt out")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_receives_sender_user_messages_by_default(
    thread_context: Option<bool>,
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
    for prompt in [
        "OLDEST",
        "Inspect the experiment.",
        "Add twenty workers.",
        "Only use staging.\nNever production.",
    ] {
        let mock = responses::mount_sse_once(&server, done()).await;
        test.submit_text_turn(prompt).await?;
        requests.extend(mock.requests());
    }
    for (namespace, output, expected) in [
        (
            "codex_app",
            format!(
                "<codex_delegation>\n  <source_thread_id>{sender}</source_thread_id>\n  <input>Inspect.</input>\n</codex_delegation>"
            ),
            vec![
                "user: Inspect the experiment.",
                "user: Add twenty workers.",
                "user: Only use staging.",
                "user: Never production.",
            ],
        ),
        (
            "codex_tui",
            "Inspect again without sender provenance.".to_owned(),
            vec![],
        ),
    ] {
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
            "id": format!("delivery-{namespace}"),
            "name": "send_message_to_thread",
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
        // The delegated agent receives the tool output; only Guardian sees original user text.
        assert!(
            !captured[0]
                .body_json()
                .to_string()
                .contains("Add twenty workers.")
        );
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
                .filter(|line| line.starts_with("user: "))
                .collect::<Vec<_>>(),
            expected
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
