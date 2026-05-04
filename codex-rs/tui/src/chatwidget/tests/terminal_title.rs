//! Terminal-title focused tests for live chatwidget status-surface behavior.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn terminal_title_shows_action_required_while_exec_approval_is_pending() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.bottom_pane.set_task_running(/*running*/ true);
    chat.refresh_terminal_title();

    let request = ExecApprovalRequestEvent {
        call_id: "call-action-required".into(),
        approval_id: Some("call-action-required".into()),
        turn_id: "turn-action-required".into(),
        command: vec!["bash".into(), "-lc".into(), "echo hello".into()],
        cwd: AbsolutePathBuf::current_dir().expect("current dir"),
        reason: Some("need confirmation".into()),
        network_approval_context: None,
        proposed_execpolicy_amendment: None,
        proposed_network_policy_amendments: None,
        additional_permissions: None,
        available_decisions: None,
    };
    handle_exec_approval_request(&mut chat, "sub-action-required", request);

    chat.pre_draw_tick();

    assert_eq!(
        chat.last_terminal_title,
        Some("[ ! ] Action Required | project".to_string())
    );
    assert!(!chat.should_animate_terminal_title_spinner());

    chat.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
    chat.pre_draw_tick();

    let title = chat
        .last_terminal_title
        .as_deref()
        .expect("terminal title should be restored after approval");
    assert!(title.contains("project"));
    assert!(!title.contains("Action Required"));
    assert!(chat.should_animate_terminal_title_spinner());
}

#[tokio::test]
async fn terminal_title_action_required_respects_spinner_setting() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.config.tui_terminal_title = Some(vec!["project".to_string()]);
    chat.bottom_pane.set_task_running(/*running*/ true);
    chat.refresh_terminal_title();

    let request = ExecApprovalRequestEvent {
        call_id: "call-no-spinner".into(),
        approval_id: Some("call-no-spinner".into()),
        turn_id: "turn-no-spinner".into(),
        command: vec!["bash".into(), "-lc".into(), "echo hello".into()],
        cwd: AbsolutePathBuf::current_dir().expect("current dir"),
        reason: Some("need confirmation".into()),
        network_approval_context: None,
        proposed_execpolicy_amendment: None,
        proposed_network_policy_amendments: None,
        additional_permissions: None,
        available_decisions: None,
    };
    handle_exec_approval_request(&mut chat, "sub-no-spinner", request);

    chat.pre_draw_tick();

    assert_eq!(chat.last_terminal_title, Some("project".to_string()));
    assert!(!chat.should_animate_terminal_title_action_required());
}

#[tokio::test]
async fn terminal_title_action_required_blinks_when_animations_are_enabled() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.bottom_pane.set_task_running(/*running*/ true);
    chat.terminal_title_animation_origin = Instant::now() - std::time::Duration::from_millis(1500);
    chat.refresh_terminal_title();

    let request = ExecApprovalRequestEvent {
        call_id: "call-blink".into(),
        approval_id: Some("call-blink".into()),
        turn_id: "turn-blink".into(),
        command: vec!["bash".into(), "-lc".into(), "echo hello".into()],
        cwd: AbsolutePathBuf::current_dir().expect("current dir"),
        reason: Some("need confirmation".into()),
        network_approval_context: None,
        proposed_execpolicy_amendment: None,
        proposed_network_policy_amendments: None,
        additional_permissions: None,
        available_decisions: None,
    };
    handle_exec_approval_request(&mut chat, "sub-blink", request);

    chat.pre_draw_tick();

    assert_eq!(
        chat.last_terminal_title,
        Some("[ . ] Action Required | project".to_string())
    );
    assert!(chat.should_animate_terminal_title_action_required());
}

#[tokio::test]
async fn terminal_title_activity_indicators_do_not_animate_when_animations_are_disabled() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.config.animations = false;
    chat.bottom_pane.set_task_running(/*running*/ true);
    chat.terminal_title_animation_origin = Instant::now() - std::time::Duration::from_millis(1500);
    chat.refresh_terminal_title();

    assert_eq!(chat.last_terminal_title, Some("project".to_string()));
    assert!(!chat.should_animate_terminal_title_spinner());

    let request = ExecApprovalRequestEvent {
        call_id: "call-no-animations".into(),
        approval_id: Some("call-no-animations".into()),
        turn_id: "turn-no-animations".into(),
        command: vec!["bash".into(), "-lc".into(), "echo hello".into()],
        cwd: AbsolutePathBuf::current_dir().expect("current dir"),
        reason: Some("need confirmation".into()),
        network_approval_context: None,
        proposed_execpolicy_amendment: None,
        proposed_network_policy_amendments: None,
        additional_permissions: None,
        available_decisions: None,
    };
    handle_exec_approval_request(&mut chat, "sub-no-animations", request);

    chat.pre_draw_tick();

    assert_eq!(
        chat.last_terminal_title,
        Some("[ ! ] Action Required | project".to_string())
    );
    assert!(!chat.should_animate_terminal_title_action_required());
}
