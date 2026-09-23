//! Async questions notify once on arrival and honor the terminal notification settings.

use super::*;
use codex_protocol::items::AsyncUserInputQuestion;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn live_async_question_notifies_once_and_takes_priority_over_turn_completion() {
    let (mut chat, _rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
    let item = AppServerThreadItem::AgentMessage {
        id: "question".into(),
        text: String::new(),
        phase: None,
        memory_citation: None,
        delivery: None,
        questions: Some(vec![AsyncUserInputQuestion {
            title: "Which environment?".into(),
            options: None,
        }]),
    };
    for kind in [
        ReplayKind::ResumeInitialMessages,
        ReplayKind::ThreadSnapshot,
    ] {
        chat.replay_thread_item(item.clone(), "turn".into(), kind);
        assert!(chat.pending_notification.is_none());
    }

    let notification = ServerNotification::ItemCompleted(ItemCompletedNotification {
        item,
        thread_id: "thread".into(),
        turn_id: "turn".into(),
        completed_at_ms: 0,
    });
    chat.handle_server_notification(notification.clone(), /*replay_kind*/ None);
    chat.notify(Notification::AgentTurnComplete {
        response: "Done".into(),
    });
    insta::assert_snapshot!(
        chat.pending_notification.take().unwrap().display(),
        @"Question: Which environment?"
    );

    chat.handle_server_notification(notification.clone(), /*replay_kind*/ None);
    assert!(chat.pending_notification.is_none());
    chat.bottom_pane.clear_pending_questions();
    chat.handle_server_notification(notification, /*replay_kind*/ None);
    assert!(chat.pending_notification.is_none());
}

#[tokio::test]
async fn async_question_notifications_respect_settings_and_ignore_empty_batches() {
    for (settings, expected) in [
        (Notifications::Enabled(true), true),
        (Notifications::Enabled(false), false),
        (Notifications::Custom(vec!["async-question".into()]), true),
        (
            Notifications::Custom(vec!["agent-turn-complete".into()]),
            false,
        ),
    ] {
        let (mut chat, _rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
        chat.local_settings.tui.notification_settings.notifications = settings;
        chat.add_async_questions("empty", &[]);
        assert!(chat.pending_notification.is_none());
        chat.add_async_questions(
            "question",
            &[AsyncUserInputQuestion {
                title: "Which way?".into(),
                options: None,
            }],
        );
        assert_eq!(chat.pending_notification.is_some(), expected);
    }
}

#[tokio::test]
async fn async_question_notification_summarizes_batches_and_bounds_long_titles() {
    let (mut chat, _rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
    let questions = vec![
        AsyncUserInputQuestion {
            title: "Which environment should we deploy this change to?".into(),
            options: None,
        },
        AsyncUserInputQuestion {
            title: "Any details?".into(),
            options: None,
        },
    ];
    chat.add_async_questions("batch", &questions);
    insta::assert_snapshot!(
        chat.pending_notification.take().unwrap().display(),
        @"Question: 2 questions requested"
    );
    chat.add_async_questions("single", &questions[..1]);
    insta::assert_snapshot!(
        chat.pending_notification.take().unwrap().display(),
        @"Question: Which environment should we..."
    );
}

#[tokio::test]
async fn async_question_notification_expires_when_turn_ends() {
    let mut outcomes = Vec::new();
    for (status, expected) in [
        (AppServerTurnStatus::Completed, Some("Agent turn complete")),
        (AppServerTurnStatus::Interrupted, None),
        (AppServerTurnStatus::Failed, None),
    ] {
        let (mut chat, _rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
        handle_turn_started(&mut chat, "turn");
        chat.add_async_questions(
            "question",
            &[AsyncUserInputQuestion {
                title: "Which way?".into(),
                options: None,
            }],
        );
        chat.handle_server_notification(
            ServerNotification::TurnCompleted(TurnCompletedNotification {
                thread_id: "thread".into(),
                turn: app_server_turn(
                    "ended-turn",
                    status.clone(),
                    /*duration_ms*/ None,
                    /*error*/ None,
                ),
            }),
            /*replay_kind*/ None,
        );
        assert_eq!(
            (
                chat.pending_notification
                    .as_ref()
                    .map(Notification::display),
                chat.bottom_pane.question_editor().unanswered_count(),
            ),
            (expected.map(str::to_string), 0),
        );
        outcomes.push((
            status,
            chat.pending_notification
                .as_ref()
                .map(Notification::display),
        ));
    }
    insta::assert_debug_snapshot!("async_question_turn_end_notifications", outcomes);
}
