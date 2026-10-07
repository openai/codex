//! Shared-store sender evidence reaches only the reviewer, including after compacted replay.

use std::sync::Arc;

use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_core::config::Constrained;
use codex_history::RolloutItem;
use codex_login::CodexAuth;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::ThreadRolledBackEvent;
use codex_protocol::turn_input::TurnInput;
use codex_thread_store::InMemoryThreadStore;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test_case::test_case(ThreadHistoryMode::Legacy; "legacy")]
#[test_case::test_case(ThreadHistoryMode::Paginated; "paginated")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cross_host_sender_context(history_mode: ThreadHistoryMode) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let store = Arc::new(InMemoryThreadStore::default());
    let sender = test_codex()
        .with_thread_store(store.clone())
        .with_history_mode(history_mode)
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(super::configure_scenario_catalog)
        .build_with_auto_env(&server)
        .await?;
    // Separate managers share durable storage, as cloud workers do. Neither loads the other.
    let receiver = test_codex()
        .with_thread_store(store)
        .with_history_mode(history_mode)
        .with_config(|config| {
            super::configure_scenario_catalog(config);
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
        })
        .build_with_auto_env(&server)
        .await?;
    let sender_id = sender.session_configured.thread_id;
    assert!(receiver.thread_manager.get_thread(sender_id).await.is_err());
    let mut sender_responses = vec![
        sse(vec![
            ev_assistant_message("reply-0", "Rerun only staging, preserving checkpoints?"),
            ev_completed("done"),
        ]),
        sse(vec![
            ev_assistant_message("reply-1", "I will ask the helper."),
            ev_completed("done"),
        ]),
    ];
    if history_mode == ThreadHistoryMode::Paginated {
        sender_responses.push(sse(vec![
            json!({"type": "response.output_item.done", "item": {
                "type": "compaction", "id": "checkpoint", "encrypted_content": "opaque sender summary"
            }}),
            ev_completed("compacted"),
        ]));
    }
    sender_responses.push(sse(vec![ev_completed("later")]));
    let sender_mock = responses::mount_sse_sequence(&server, sender_responses).await;
    sender.submit_text_turn("Inspect the staging job.").await?;
    sender
        .submit_text_turn("Yes. Staging only; never production.")
        .await?;
    if history_mode == ThreadHistoryMode::Paginated {
        sender.codex.submit(Op::Compact).await?;
        wait_for_event(&sender.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
    }
    sender
        .submit_text_turn("Production is also allowed.")
        .await?;
    let mut requests = sender_mock.requests();
    // Replay a historical rollback: its removed permission must never reach the receiver.
    sender
        .codex
        .append_rollout_items(&[RolloutItem::EventMsg(EventMsg::ThreadRolledBack(
            ThreadRolledBackEvent { num_turns: 1 },
        ))])
        .await?;
    sender.codex.flush_rollout().await?;

    let expected = [
        "user: Inspect the staging job.",
        "assistant: Rerun only staging, preserving checkpoints?",
        "user: Yes. Staging only; never production.",
    ];
    let mock = responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_function_call(
                    "inspect",
                    "exec_command",
                    r#"{"cmd":"exit 0","sandbox_permissions":"require_escalated"}"#,
                ),
                ev_completed("action"),
            ]),
            sse(vec![
                ev_assistant_message(
                    "decision",
                    r#"{"risk_level":"high","user_authorization":"unknown","outcome":"deny"}"#,
                ),
                ev_completed("review"),
            ]),
            sse(vec![ev_completed("done")]),
        ],
    )
    .await;
    let delivery: ResponseItem = serde_json::from_value(json!({
        "type": "function_call_output", "id": "delivery-0",
        "name": "send_message", "namespace": "cloud_threads",
        "output": format!("<codex_delegation><source_thread_id>{sender_id}</source_thread_id><input>Inspect.</input></codex_delegation>"),
    }))?;
    receiver
        .codex
        .start_or_steer_turn(TurnInputRequest::new(TurnInput::ResponseItem(delivery)))
        .await?;
    wait_for_event(&receiver.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let captured = mock.requests();
    assert_eq!(captured.len(), 3);
    let worker = captured[0].body_json().to_string();
    assert!(!worker.contains("never production"));
    let history = receiver.codex.conversation_history_snapshot().await;
    let evidence = &history
        .retained_context()
        .expect("retained context")
        .sender_user_messages()
        .expect("sender evidence")
        .text;
    assert_eq!(
        evidence
            .lines()
            .filter(|line| line.starts_with("user: ") || line.starts_with("assistant: "))
            .collect::<Vec<_>>(),
        expected
    );
    assert!(evidence.len() <= 3_600);
    let review = captured[1].message_input_texts("user").join("\n");
    assert!(review.contains(evidence));
    assert!(!review.contains("Production is also allowed."));
    requests.extend(captured);
    assert!(receiver.thread_manager.get_thread(sender_id).await.is_err());
    if history_mode == ThreadHistoryMode::Paginated {
        let mut snapshot = context_snapshot::format_request_history_snapshot(
            "Two workers share a thread store. The receiver reads the sender's original instructions and preceding assistant question after compaction and rollback, without loading the sender. Only Guardian receives the evidence.",
            &requests,
            &ContextSnapshotOptions::default()
                .rewrite_known_segments()
                .include_request_settings(),
        );
        for (pattern, replacement) in [
            (
                r#"(?m)^(\s*"environment_id": )"(?:local|remote)""#,
                "$1\"<ENVIRONMENT>\"",
            ),
            (
                r#"(The active permission profile for environment )"(?:local|remote)""#,
                "$1\"<ENVIRONMENT>\"",
            ),
            (r#"(?m)^(\s*"cwd": )"[^"]*""#, "$1\"<CWD>\""),
            (
                r#""command": \[\s*(?:"[^"]*",\s*)*"exit 0"\s*\]"#,
                "\"command\": [\"<SHELL>\", \"exit 0\"]",
            ),
        ] {
            snapshot = regex_lite::Regex::new(pattern)?
                .replace_all(&snapshot, replacement)
                .into_owned();
        }
        insta::assert_snapshot!("cross_host_sender_context", snapshot);
    }
    sender.codex.shutdown_and_wait().await?;
    receiver.codex.shutdown_and_wait().await?;
    Ok(())
}
