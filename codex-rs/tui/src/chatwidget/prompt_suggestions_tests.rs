//! Tests follow-up suggestion eligibility and deduplication.

use super::*;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnStartedNotification;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn prompt_suggestion_only_follows_enabled_successful_live_turns() {
    for (enabled, status, replay, expected) in [
        (false, TurnStatus::Completed, None, 0),
        (true, TurnStatus::Failed, None, 0),
        (true, TurnStatus::Interrupted, None, 0),
        (
            true,
            TurnStatus::Completed,
            Some(ReplayKind::ResumeInitialMessages),
            0,
        ),
        (true, TurnStatus::Completed, None, 1),
    ] {
        let (mut chat, _, mut events, _) =
            super::super::tests::helpers::make_chatwidget_manual_with_sender().await;
        let thread_id = ThreadId::new();
        chat.thread_id = Some(thread_id);
        chat.local_settings.tui.prompt_suggestions = enabled;
        chat.prompt_suggestion_summary = Some(codex_protocol::config_types::ReasoningSummary::None);
        let turn = Turn {
            id: "turn".into(),
            items: vec![],
            items_view: Default::default(),
            status: TurnStatus::InProgress,
            error: None,
            started_at: None,
            completed_at: None,
            duration_ms: None,
        };
        chat.handle_server_notification(
            ServerNotification::TurnStarted(TurnStartedNotification {
                thread_id: thread_id.to_string(),
                turn: turn.clone(),
            }),
            replay,
        );
        let completed = ServerNotification::TurnCompleted(TurnCompletedNotification {
            thread_id: thread_id.to_string(),
            turn: Turn { status, ..turn },
        });
        chat.handle_server_notification(completed.clone(), replay);
        chat.handle_server_notification(completed, replay);
        let mut requests = 0;
        while let Ok(event) = events.try_recv() {
            if matches!(event, AppEvent::GeneratePromptSuggestion(_)) {
                requests += 1;
            }
        }
        assert_eq!(requests, expected);
    }
}
