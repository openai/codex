//! Exercises terminal status across live, interrupted, and replayed turns.
//! Replayed results and work already underway must never appear as new completions.

use super::*;
use crate::terminal_program_status::ProgramStatus;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn program_status_tracks_live_results_and_resolved_approvals() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.install_test_ambient_pet_for_tests(/*animations_enabled*/ false);
    let status = |chat: &mut ChatWidget| {
        chat.pre_draw_tick();
        let area = ratatui::layout::Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 24,
        );
        // Inspect the selected sprite even when an approval modal covers it.
        let draw = chat
            .ambient_pet
            .as_ref()
            .unwrap()
            .draw_request(area, area.bottom())
            .unwrap();
        let status = chat.desired_program_status();
        format!("{status:?} {}", draw.frame.display())
    };
    let mut statuses = vec![status(&mut chat)];
    handle_turn_started(&mut chat, "status-turn");
    statuses.push(status(&mut chat));
    handle_exec_approval_request(
        &mut chat,
        "status-approval",
        ExecApprovalRequestEvent {
            kind: Default::default(),
            call_id: "status-call".to_string(),
            approval_id: Some("status-call".to_string()),
            turn_id: "status-turn".to_string(),
            environment_id: None,
            command: vec!["echo".to_string(), "hello".to_string()],
            cwd: AbsolutePathBuf::current_dir().expect("current dir"),
            reason: Some("need confirmation".to_string()),
            network_approval_context: None,
            proposed_execpolicy_amendment: None,
            proposed_network_policy_amendments: None,
            additional_permissions: None,
            available_decisions: None,
        },
    );
    statuses.push(status(&mut chat));
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
    statuses.push(status(&mut chat));
    handle_turn_completed(&mut chat, "status-turn", /*duration_ms*/ None);
    statuses.push(status(&mut chat));
    // A second draw must leave the unread result in place.
    statuses.push(status(&mut chat));
    chat.acknowledge_program_status();
    statuses.push(status(&mut chat));
    handle_turn_started(&mut chat, "cancelled-turn");
    handle_turn_interrupted(&mut chat, "cancelled-turn");
    statuses.push(status(&mut chat));
    handle_turn_started(&mut chat, "failed-turn");
    chat.handle_server_notification(
        ServerNotification::TurnCompleted(TurnCompletedNotification {
            thread_id: String::new(),
            turn: app_server_turn(
                "failed-turn",
                AppServerTurnStatus::Failed,
                /*duration_ms*/ None,
                /*error*/ None,
            ),
        }),
        /*replay_kind*/ None,
    );
    statuses.push(status(&mut chat));
    chat.acknowledge_program_status();
    statuses.push(status(&mut chat));
    insta::assert_snapshot!(statuses.join("\n"), @r"
    Idle frame-0.png
    Working frame-56.png
    Blocked(Permission) frame-48.png
    Working frame-56.png
    Done frame-64.png
    Done frame-64.png
    Idle frame-0.png
    Idle frame-0.png
    Error frame-40.png
    Idle frame-0.png
    ");
}

#[tokio::test]
async fn program_status_does_not_treat_replayed_results_as_new_or_hide_follow_up_work() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    handle_turn_started(&mut chat, "old-turn");
    chat.on_task_complete(
        /*last_agent_message*/ None, /*completion*/ None, /*from_replay*/ true,
    );
    let replayed = chat.desired_program_status();
    handle_turn_started(&mut chat, "live-turn");
    handle_turn_completed(&mut chat, "live-turn", /*duration_ms*/ None);
    let completed = chat.desired_program_status();
    // A queued follow-up is already submitted, before its TurnStarted notification arrives.
    chat.input_queue.user_turn_pending_start = true;
    let pending = chat.desired_program_status();
    assert_eq!(
        [replayed, completed, pending],
        [
            ProgramStatus::Idle,
            ProgramStatus::Done,
            ProgramStatus::Working
        ]
    );
}
