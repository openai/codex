//! Subagent notifications must preserve one authoritative answer across streaming boundaries.

use super::*;
use pretty_assertions::assert_eq;

const PREFIX: &str = "The top is PR #42.\n\n";
const SUFFIX: &str = "Branch: feature/rollout.";
const ACTIVITY: &str = "• Completed `/root/stack_tip`\n";

#[derive(Debug, PartialEq, Eq)]
enum FinalizedEvent {
    Answer(String),
    History(String),
}

fn complete_subagent(chat: &mut ChatWidget, id: &str) {
    let item = AppServerThreadItem::SubAgentActivity {
        id: id.to_string(),
        kind: codex_app_server_protocol::SubAgentActivityKind::Completed,
        agent_thread_id: ThreadId::new().to_string(),
        agent_path: "/root/stack_tip".to_string(),
    };
    for notification in [
        ServerNotification::ItemStarted(ItemStartedNotification {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            started_at_ms: 0,
            item: item.clone(),
        }),
        ServerNotification::ItemCompleted(ItemCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            completed_at_ms: 0,
            item,
        }),
    ] {
        chat.handle_server_notification(notification, /*replay_kind*/ None);
    }
}

/// Inspect publication order, excluding provisional stream cells and completion timestamps.
fn drain_finalized_transcript(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
) -> (Vec<FinalizedEvent>, String) {
    let mut events = Vec::new();
    let mut rendered = Vec::new();
    while let Ok(event) = rx.try_recv() {
        match event {
            AppEvent::ConsolidateAgentMessage {
                source,
                cwd,
                inline_visualization_context,
                ..
            } => {
                let cell = history_cell::AgentMarkdownCell::new_with_inline_visualizations(
                    source.clone(),
                    &cwd,
                    inline_visualization_context,
                );
                rendered.push(lines_to_single_string(&cell.display_lines(/*width*/ 80)));
                events.push(FinalizedEvent::Answer(source));
            }
            AppEvent::InsertHistoryCell(cell)
                if !cell.as_any().is::<history_cell::AgentMessageCell>()
                    && !cell.as_any().is::<history_cell::FinalMessageSeparator>() =>
            {
                let text = lines_to_single_string(&cell.display_lines(/*width*/ 80));
                rendered.push(text.clone());
                events.push(FinalizedEvent::History(text));
            }
            _ => {}
        }
    }
    (events, rendered.join("\n"))
}

#[tokio::test]
async fn subagent_completion_preserves_stream_and_authoritative_answer() {
    let answer = format!("{PREFIX}{SUFFIX}");
    for (before_activity, after_activity) in [
        // The reported session: all deltas precede activity, then the answer item completes.
        (answer.as_str(), ""),
        // The same activity can arrive before the remaining answer deltas.
        (PREFIX, SUFFIX),
        // Authoritative completion must still repair a dropped final delta.
        (PREFIX, ""),
    ] {
        let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
        handle_turn_started(&mut chat, "turn-1");
        handle_agent_message_delta(&mut chat, before_activity);
        chat.run_commit_tick();

        complete_subagent(&mut chat, "activity-1");
        assert!(chat.stream_controller.is_some());
        assert!(!chat.interrupts.is_empty());
        assert_eq!(
            drain_finalized_transcript(&mut rx),
            (Vec::new(), String::new())
        );

        if !after_activity.is_empty() {
            handle_agent_message_delta(&mut chat, after_activity);
        }
        complete_assistant_message(&mut chat, "msg-1", &answer, Some(MessagePhase::FinalAnswer));
        handle_turn_completed(&mut chat, "turn-1", /*duration_ms*/ None);

        let (events, rendered) = drain_finalized_transcript(&mut rx);
        assert_eq!(
            events,
            vec![
                FinalizedEvent::Answer(answer.clone()),
                FinalizedEvent::History(ACTIVITY.to_string()),
            ],
        );
        assert!(chat.interrupts.is_empty());
        assert_eq!(
            chat.transcript.last_agent_markdown.as_deref(),
            Some(answer.as_str())
        );
        insta::allow_duplicates! {
            insta::assert_snapshot!(rendered, @"
            • The top is PR #42.

              Branch: feature/rollout.

            • Completed `/root/stack_tip`
            ");
        }
    }
}

#[tokio::test]
async fn interrupted_answer_flushes_queued_subagent_activity() {
    let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
    handle_turn_started(&mut chat, "turn-1");
    handle_agent_message_delta(&mut chat, PREFIX);
    chat.run_commit_tick();
    complete_subagent(&mut chat, "activity-1");
    assert!(!chat.interrupts.is_empty());

    handle_turn_interrupted(&mut chat, "turn-1");

    let (events, _rendered) = drain_finalized_transcript(&mut rx);
    assert_eq!(events.len(), 3);
    assert_eq!(
        &events[..2],
        &[
            FinalizedEvent::Answer(PREFIX.trim_end().to_string()),
            FinalizedEvent::History(ACTIVITY.to_string()),
        ],
    );
    let FinalizedEvent::History(interrupted) = &events[2] else {
        panic!("expected interruption notice after the answer and subagent activity");
    };
    assert!(interrupted.starts_with("■ Conversation interrupted"));
    assert!(chat.interrupts.is_empty());
    assert!(chat.stream_controller.is_none());
}

#[tokio::test]
async fn terminal_turn_drains_subagents_without_opening_queued_questions() {
    for status in [AppServerTurnStatus::Completed, AppServerTurnStatus::Failed] {
        let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
        handle_turn_started(&mut chat, "turn-1");
        handle_agent_message_delta(&mut chat, PREFIX);
        chat.run_commit_tick();
        chat.on_request_user_input(ToolRequestUserInputParams {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: "question-1".to_string(),
            questions: vec![ToolRequestUserInputQuestion {
                id: "scope".to_string(),
                header: "Scope".to_string(),
                question: "Which change should I review?".to_string(),
                is_other: false,
                is_secret: false,
                options: None,
            }],
            is_blocking: true,
            auto_resolution_ms: None,
        });
        complete_subagent(&mut chat, "activity-1");
        assert!(chat.interrupts.has_pending_prompt());
        assert!(!chat.bottom_pane.has_active_view());

        let mut expected = vec![
            FinalizedEvent::Answer(PREFIX.trim_end().to_string()),
            FinalizedEvent::History(ACTIVITY.to_string()),
        ];
        let error = (status == AppServerTurnStatus::Failed).then(|| {
            expected.push(FinalizedEvent::History(
                "■ stream disconnected\n".to_string(),
            ));
            AppServerTurnError {
                message: "stream disconnected".to_string(),
                misalignment: None,
                codex_error_info: None,
                additional_details: None,
            }
        });
        // There is no final answer item; turn termination must settle the stream and activity.
        chat.handle_server_notification(
            ServerNotification::TurnCompleted(TurnCompletedNotification {
                thread_id: "thread-1".to_string(),
                turn: app_server_turn("turn-1", status, /*duration_ms*/ None, error),
            }),
            /*replay_kind*/ None,
        );
        assert_eq!(drain_finalized_transcript(&mut rx).0, expected);
        assert!(chat.interrupts.has_pending_prompt());
        assert!(!chat.bottom_pane.has_active_view());
        assert!(!chat.turn_lifecycle.agent_turn_running);
        assert!(chat.stream_controller.is_none());

        // Another child may finish after the parent stops, while the old question remains queued.
        complete_subagent(&mut chat, "activity-2");
        assert_eq!(
            drain_finalized_transcript(&mut rx).0,
            vec![FinalizedEvent::History(ACTIVITY.to_string())],
        );
        assert!(!chat.bottom_pane.has_active_view());
        assert!(chat.interrupts.remove_resolved_prompt(
            &crate::app::app_server_requests::ResolvedAppServerRequest::UserInput {
                call_id: "question-1".to_string(),
            },
        ));
        assert!(chat.interrupts.is_empty());
    }
}
