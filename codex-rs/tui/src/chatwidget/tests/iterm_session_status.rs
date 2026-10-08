use super::*;
use crate::iterm_session_status::ItermSessionStatus::Idle;
use crate::iterm_session_status::ItermSessionStatus::Waiting;
use crate::iterm_session_status::ItermSessionStatus::Working;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn session_status_follows_existing_lifecycle_state() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let mut states = vec![chat.desired_iterm_session_status()];

    chat.bottom_pane.set_task_running(/*running*/ true);
    states.push(chat.desired_iterm_session_status());

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
    states.push(chat.desired_iterm_session_status());

    chat.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
    states.push(chat.desired_iterm_session_status());

    chat.bottom_pane.set_task_running(/*running*/ false);
    states.push(chat.desired_iterm_session_status());

    chat.unified_exec_processes.push(UnifiedExecProcessSummary {
        key: "background-process".to_string(),
        call_id: "background-call".to_string(),
        command_display: "sleep 5".to_string(),
        recent_chunks: Vec::new(),
    });
    states.push(chat.desired_iterm_session_status());
    chat.unified_exec_processes.clear();
    states.push(chat.desired_iterm_session_status());

    assert_eq!(
        states,
        [Idle, Working, Waiting, Working, Idle, Working, Idle]
    );
}
