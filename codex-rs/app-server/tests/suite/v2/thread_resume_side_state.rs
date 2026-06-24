use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::create_fake_rollout;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::rollout_path;
use app_test_support::test_absolute_path;
use app_test_support::to_response;
use codex_app_server_protocol::AutoReviewDecisionSource;
use codex_app_server_protocol::GuardianApprovalReview;
use codex_app_server_protocol::GuardianApprovalReviewAction;
use codex_app_server_protocol::GuardianApprovalReviewStatus;
use codex_app_server_protocol::GuardianCommandSource;
use codex_app_server_protocol::GuardianRiskLevel;
use codex_app_server_protocol::GuardianUserAuthorization;
use codex_app_server_protocol::HookCompletedNotification;
use codex_app_server_protocol::HookEventName;
use codex_app_server_protocol::HookExecutionMode;
use codex_app_server_protocol::HookHandlerType;
use codex_app_server_protocol::HookOutputEntry;
use codex_app_server_protocol::HookOutputEntryKind;
use codex_app_server_protocol::HookRunStatus;
use codex_app_server_protocol::HookRunSummary;
use codex_app_server_protocol::HookScope;
use codex_app_server_protocol::HookSource;
use codex_app_server_protocol::ItemGuardianApprovalReviewCompletedNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::SortDirection;
use codex_app_server_protocol::ThreadResumeInitialTurnsPageParams;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::TurnItemsView;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::GuardianAssessmentAction;
use codex_protocol::protocol::GuardianAssessmentDecisionSource;
use codex_protocol::protocol::GuardianAssessmentEvent;
use codex_protocol::protocol::GuardianAssessmentStatus;
use codex_protocol::protocol::GuardianCommandSource as CoreGuardianCommandSource;
use codex_protocol::protocol::GuardianRiskLevel as CoreGuardianRiskLevel;
use codex_protocol::protocol::GuardianUserAuthorization as CoreGuardianUserAuthorization;
use codex_protocol::protocol::HookCompletedEvent;
use codex_protocol::protocol::HookEventName as CoreHookEventName;
use codex_protocol::protocol::HookExecutionMode as CoreHookExecutionMode;
use codex_protocol::protocol::HookHandlerType as CoreHookHandlerType;
use codex_protocol::protocol::HookOutputEntry as CoreHookOutputEntry;
use codex_protocol::protocol::HookOutputEntryKind as CoreHookOutputEntryKind;
use codex_protocol::protocol::HookRunStatus as CoreHookRunStatus;
use codex_protocol::protocol::HookRunSummary as CoreHookRunSummary;
use codex_protocol::protocol::HookScope as CoreHookScope;
use codex_protocol::protocol::HookSource as CoreHookSource;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::TurnStartedEvent;
use codex_rollout::append_rollout_item_to_path;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

use super::connection_handling_websocket::create_config_toml;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);
const REPLAY_TIMEOUT: Duration = Duration::from_secs(1);
const FILENAME_TIMESTAMP: &str = "2025-01-05T12-00-00";
const META_TIMESTAMP: &str = "2025-01-05T12:00:00Z";

#[tokio::test]
async fn thread_resume_replays_persisted_terminal_auto_approval_review() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;

    let turn_id = "turn-offline-auto-review";
    let review_id = "review-offline-command";
    let target_item_id = "command-offline";
    let command = "rm -f /tmp/offline-review.sqlite";
    let cwd = test_absolute_path("/tmp");
    let thread_id = create_rollout_with_side_state(
        codex_home.path(),
        turn_id,
        EventMsg::GuardianAssessment(GuardianAssessmentEvent {
            id: review_id.to_string(),
            target_item_id: Some(target_item_id.to_string()),
            turn_id: turn_id.to_string(),
            started_at_ms: 1_000,
            completed_at_ms: Some(1_042),
            status: GuardianAssessmentStatus::Approved,
            risk_level: Some(CoreGuardianRiskLevel::Low),
            user_authorization: Some(CoreGuardianUserAuthorization::High),
            rationale: Some("The saved command is safe.".to_string()),
            decision_source: Some(GuardianAssessmentDecisionSource::Agent),
            action: GuardianAssessmentAction::Command {
                source: CoreGuardianCommandSource::Shell,
                command: command.to_string(),
                cwd: cwd.clone(),
            },
        }),
    )
    .await?;

    let mut app_server = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, app_server.initialize()).await??;
    let expected = ItemGuardianApprovalReviewCompletedNotification {
        thread_id,
        turn_id: turn_id.to_string(),
        started_at_ms: 1_000,
        completed_at_ms: 1_042,
        review_id: review_id.to_string(),
        target_item_id: Some(target_item_id.to_string()),
        decision_source: AutoReviewDecisionSource::Agent,
        review: GuardianApprovalReview {
            status: GuardianApprovalReviewStatus::Approved,
            risk_level: Some(GuardianRiskLevel::Low),
            user_authorization: Some(GuardianUserAuthorization::High),
            rationale: Some("The saved command is safe.".to_string()),
        },
        action: GuardianApprovalReviewAction::Command {
            source: GuardianCommandSource::Shell,
            command: command.to_string(),
            cwd,
        },
    };

    // The first resume loads the thread; the second rejoins that same running thread.
    for _ in 0..2 {
        let response =
            resume_with_initial_turns_page(&mut app_server, expected.thread_id.as_str()).await?;
        assert!(response.thread.turns.is_empty());
        assert!(response.initial_turns_page.is_some());

        let notification = timeout(
            REPLAY_TIMEOUT,
            app_server.read_stream_until_notification_message("item/autoApprovalReview/completed"),
        )
        .await??;
        let parsed: ServerNotification = notification.try_into()?;
        let ServerNotification::ItemGuardianApprovalReviewCompleted(actual) = parsed else {
            unreachable!("filtered notification must be a completed auto-approval review");
        };
        assert_eq!(actual, expected);
    }

    Ok(())
}

#[tokio::test]
async fn thread_resume_replays_persisted_completed_hook() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;

    let turn_id = "turn-offline-hook";
    let run_id = "hook-run-offline";
    let source_path = test_absolute_path("/tmp/offline-hook.sh");
    let thread_id = create_rollout_with_side_state(
        codex_home.path(),
        turn_id,
        EventMsg::HookCompleted(HookCompletedEvent {
            turn_id: Some(turn_id.to_string()),
            run: CoreHookRunSummary {
                id: run_id.to_string(),
                event_name: CoreHookEventName::PostToolUse,
                handler_type: CoreHookHandlerType::Command,
                execution_mode: CoreHookExecutionMode::Sync,
                scope: CoreHookScope::Turn,
                source_path: source_path.clone(),
                source: CoreHookSource::Project,
                display_order: 7,
                status: CoreHookRunStatus::Completed,
                status_message: Some("Offline hook completed".to_string()),
                started_at: 1_700_000_000,
                completed_at: Some(1_700_000_042),
                duration_ms: Some(42_000),
                entries: vec![
                    CoreHookOutputEntry {
                        kind: CoreHookOutputEntryKind::Feedback,
                        text: "Apply the saved formatting feedback.".to_string(),
                    },
                    CoreHookOutputEntry {
                        kind: CoreHookOutputEntryKind::Context,
                        text: "Persist this hook context across reconnect.".to_string(),
                    },
                ],
            },
        }),
    )
    .await?;

    let mut app_server = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, app_server.initialize()).await??;
    let expected = HookCompletedNotification {
        thread_id,
        turn_id: Some(turn_id.to_string()),
        run: HookRunSummary {
            id: run_id.to_string(),
            event_name: HookEventName::PostToolUse,
            handler_type: HookHandlerType::Command,
            execution_mode: HookExecutionMode::Sync,
            scope: HookScope::Turn,
            source_path,
            source: HookSource::Project,
            display_order: 7,
            status: HookRunStatus::Completed,
            status_message: Some("Offline hook completed".to_string()),
            started_at: 1_700_000_000,
            completed_at: Some(1_700_000_042),
            duration_ms: Some(42_000),
            entries: vec![
                HookOutputEntry {
                    kind: HookOutputEntryKind::Feedback,
                    text: "Apply the saved formatting feedback.".to_string(),
                },
                HookOutputEntry {
                    kind: HookOutputEntryKind::Context,
                    text: "Persist this hook context across reconnect.".to_string(),
                },
            ],
        },
    };

    // The first resume loads the thread; the second rejoins that same running thread.
    for _ in 0..2 {
        let response =
            resume_with_initial_turns_page(&mut app_server, expected.thread_id.as_str()).await?;
        assert!(response.thread.turns.is_empty());
        assert!(response.initial_turns_page.is_some());

        let notification = timeout(
            REPLAY_TIMEOUT,
            app_server.read_stream_until_notification_message("hook/completed"),
        )
        .await??;
        let parsed: ServerNotification = notification.try_into()?;
        let ServerNotification::HookCompleted(actual) = parsed else {
            unreachable!("filtered notification must be a completed hook run");
        };
        assert_eq!(actual, expected);
    }

    Ok(())
}

async fn create_rollout_with_side_state(
    codex_home: &Path,
    turn_id: &str,
    event: EventMsg,
) -> Result<String> {
    let thread_id = create_fake_rollout(
        codex_home,
        FILENAME_TIMESTAMP,
        META_TIMESTAMP,
        "Saved user message",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let path = rollout_path(codex_home, FILENAME_TIMESTAMP, thread_id.as_str());
    append_rollout_item_to_path(
        path.as_path(),
        &RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: turn_id.to_string(),
            trace_id: None,
            started_at: Some(1),
            model_context_window: None,
            collaboration_mode_kind: Default::default(),
        })),
    )
    .await?;
    append_rollout_item_to_path(path.as_path(), &RolloutItem::EventMsg(event)).await?;
    Ok(thread_id)
}

async fn resume_with_initial_turns_page(
    app_server: &mut TestAppServer,
    thread_id: &str,
) -> Result<ThreadResumeResponse> {
    let request_id = app_server
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread_id.to_string(),
            exclude_turns: true,
            initial_turns_page: Some(ThreadResumeInitialTurnsPageParams {
                limit: Some(5),
                sort_direction: Some(SortDirection::Desc),
                items_view: Some(TurnItemsView::Full),
            }),
            ..Default::default()
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}
