use super::*;
use pretty_assertions::assert_eq;

use crate::terminal_program_status::ProgramStatus::Blocked;
use crate::terminal_program_status::ProgramStatus::Done;
use crate::terminal_program_status::ProgramStatus::Idle;
use crate::terminal_program_status::ProgramStatus::Working;
use crate::terminal_program_status::ProgramStatusKind;

#[tokio::test]
async fn session_status_follows_existing_lifecycle_state() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let mut states = vec![chat.desired_program_status()];

    handle_turn_started(&mut chat, "session-status-turn");
    states.push(chat.desired_program_status());

    handle_exec_approval_request(
        &mut chat,
        "session-status-approval",
        ExecApprovalRequestEvent {
            kind: Default::default(),
            call_id: "session-status-call".to_string(),
            approval_id: Some("session-status-call".to_string()),
            turn_id: "session-status-turn".to_string(),
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
    states.push(chat.desired_program_status());

    chat.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
    states.push(chat.desired_program_status());

    begin_unified_exec_startup(&mut chat, "background-call", "42", "sleep 5");
    handle_turn_completed(&mut chat, "session-status-turn", /*duration_ms*/ None);
    assert!(render_bottom_popup(&chat, /*width*/ 80).contains("1 background terminal running"));
    states.push(chat.desired_program_status());
    chat.acknowledge_program_status();
    states.push(chat.desired_program_status());

    assert_eq!(
        states,
        [
            Idle,
            Working,
            Blocked(ProgramStatusKind::Permission),
            Working,
            Done,
            Idle
        ]
    );
}

#[tokio::test]
async fn session_status_detail_uses_only_foreground_activity() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let detail =
        |chat: &ChatWidget, status| chat.iterm_session_detail(status).map(ToString::to_string);

    chat.bottom_pane.set_task_running(/*running*/ true);
    let generic = detail(&chat, Working);
    chat.set_status_header("Reading source".to_string());
    let working = detail(&chat, Working);
    let waiting = detail(&chat, Blocked(ProgramStatusKind::Question));
    chat.bottom_pane.set_task_running(/*running*/ false);
    let idle = detail(&chat, Idle);
    let background = detail(&chat, Working);

    let details = [generic, working, waiting, idle, background]
        .map(|detail| detail.unwrap_or_else(|| "-".to_string()))
        .join("\n");
    insta::assert_snapshot!(details, @r"
    -
    Reading source
    -
    -
    -
    ");
}
