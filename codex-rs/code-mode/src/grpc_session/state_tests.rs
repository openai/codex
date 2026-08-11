use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::grpc;
use codex_code_mode_protocol::host::MAX_PENDING_DELEGATE_CALLS;
use pretty_assertions::assert_eq;
use uuid::Uuid;

use super::CallbackAdmission;
use super::SessionState;

fn request(execution_id: &str) -> grpc::ExecuteRequest {
    grpc::ExecuteRequest {
        session_id: "session".to_string(),
        execution_id: execution_id.to_string(),
        tool_call_id: "call".to_string(),
        source: String::new(),
        enabled_tools: vec![grpc::ToolDefinition {
            name: "tool".to_string(),
            tool_name: Some(grpc::ToolName {
                name: "tool".to_string(),
                namespace: None,
            }),
            description: String::new(),
            kind: grpc::ToolKind::Function as i32,
            input_schema_json: None,
            output_schema_json: None,
        }],
        yield_time_ms: None,
        max_output_tokens: None,
    }
}

fn tool_call(execution_id: &str, invocation_id: u128) -> grpc::ToolCall {
    let invocation_id = Uuid::from_u128(invocation_id).to_string();
    grpc::ToolCall {
        session_id: "session".to_string(),
        execution_id: execution_id.to_string(),
        cell_id: "cell".to_string(),
        invocation_id: invocation_id.clone(),
        runtime_tool_call_id: format!("runtime-{invocation_id}"),
        tool_name: Some(grpc::ToolName {
            name: "tool".to_string(),
            namespace: None,
        }),
        tool_kind: grpc::ToolKind::Function as i32,
        input_json: None,
        sequence: 1,
    }
}

fn notification(execution_id: &str, notification_id: u128) -> grpc::Notification {
    grpc::Notification {
        notification_id: Uuid::from_u128(notification_id).to_string(),
        execution_id: execution_id.to_string(),
        cell_id: "cell".to_string(),
        call_id: "call".to_string(),
        text: "hello".to_string(),
    }
}

#[test]
fn cell_closure_drains_notifications_and_cancels_tool_callbacks() {
    let mut state = SessionState::default();
    state
        .begin_execution(&request("execution"))
        .expect("register execution");
    let first = state
        .admit_invocation(&tool_call("execution", /*invocation_id*/ 1))
        .expect("accept early invocation");
    let CallbackAdmission::Active(first_cancellation) = first else {
        panic!("first callback was not admitted");
    };
    let CallbackAdmission::Active(notification_cancellation) = state
        .admit_notification(&notification("execution", /*notification_id*/ 1))
        .expect("admit notification")
    else {
        panic!("notification was not admitted");
    };
    assert_eq!(
        state
            .close_cell(grpc::CellClosed {
                execution_id: "execution".to_string(),
                cell_id: "cell".to_string(),
                final_tool_call_sequence: 3,
            })
            .expect("record cell closure"),
        None
    );
    assert!(first_cancellation.is_cancelled());
    assert!(!notification_cancellation.is_cancelled());
    state
        .admit_execution("execution", "cell")
        .expect("admit started cell");
    assert!(matches!(
        state
            .admit_invocation(&tool_call("execution", /*invocation_id*/ 2))
            .expect("reject invocation for a closed cell"),
        CallbackAdmission::Closed
    ));
    assert_eq!(
        state
            .mark_execution_ready("execution")
            .expect("claim started cell"),
        None
    );
    assert_eq!(
        state.finish_notification("execution"),
        Some(CellId::new("cell".to_string()))
    );
    assert!(notification_cancellation.is_cancelled());
}

#[test]
fn cell_closure_waits_until_the_started_cell_is_claimed() {
    let mut state = SessionState::default();
    state
        .begin_execution(&request("execution"))
        .expect("register execution");

    assert_eq!(
        state
            .close_cell(grpc::CellClosed {
                execution_id: "execution".to_string(),
                cell_id: "cell".to_string(),
                final_tool_call_sequence: 0,
            })
            .expect("record early cell closure"),
        None
    );
    state
        .admit_execution("execution", "cell")
        .expect("admit started cell");
    assert_eq!(
        state
            .mark_execution_ready("execution")
            .expect("claim started cell"),
        Some(CellId::new("cell".to_string()))
    );
}

#[test]
fn oversized_cell_ids_are_rejected_before_admission() {
    let mut state = SessionState::default();
    state
        .begin_execution(&request("execution"))
        .expect("register execution");

    assert_eq!(
        state.admit_execution("execution", &"x".repeat(grpc::MAX_IDENTIFIER_BYTES + 1)),
        Err(format!(
            "gRPC code-mode host returned cell ID exceeding {} bytes",
            grpc::MAX_IDENTIFIER_BYTES
        ))
    );
    assert_eq!(state.remove_execution("execution"), None);
}

#[test]
fn callbacks_cannot_claim_another_executions_cell() {
    let mut state = SessionState::default();
    state
        .begin_execution(&request("first"))
        .expect("register first execution");
    state
        .begin_execution(&request("second"))
        .expect("register second execution");
    state
        .admit_invocation(&tool_call("first", /*invocation_id*/ 1))
        .expect("allow the first execution to claim its cell");

    let expected = "code-mode host reused active cell ID cell".to_string();
    assert_eq!(
        state
            .admit_invocation(&tool_call("second", /*invocation_id*/ 2))
            .err(),
        Some(expected.clone())
    );
    assert_eq!(
        state
            .admit_notification(&notification("second", /*notification_id*/ 1))
            .err(),
        Some(expected.clone())
    );
    assert_eq!(
        state
            .close_cell(grpc::CellClosed {
                execution_id: "second".to_string(),
                cell_id: "cell".to_string(),
                final_tool_call_sequence: 0,
            })
            .err(),
        Some(expected.clone())
    );
    assert_eq!(state.admit_execution("second", "cell"), Err(expected));
}

#[test]
fn abandonment_before_start_ignores_later_cell_closure() {
    let mut state = SessionState::default();
    state
        .begin_execution(&request("execution"))
        .expect("register execution");

    assert_eq!(state.remove_execution("execution"), None);
    assert_eq!(
        state
            .close_cell(grpc::CellClosed {
                execution_id: "execution".to_string(),
                cell_id: "cell".to_string(),
                final_tool_call_sequence: 0,
            })
            .expect("ignore closure for abandoned execution"),
        None
    );
    assert!(state.close(/*failure*/ None).is_empty());
}

#[test]
fn abandonment_revokes_callbacks_and_ignores_late_events() {
    let mut state = SessionState::default();
    state
        .begin_execution(&request("execution"))
        .expect("register execution");
    let CallbackAdmission::Active(invocation) = state
        .admit_invocation(&tool_call("execution", /*invocation_id*/ 1))
        .expect("admit callback before execution starts")
    else {
        panic!("invocation was not admitted");
    };
    assert_eq!(
        state.remove_execution("execution"),
        Some(CellId::new("cell".to_string()))
    );
    assert!(invocation.is_cancelled());
    assert!(matches!(
        state
            .admit_invocation(&tool_call("execution", /*invocation_id*/ 2))
            .expect("reject delayed tool invocation"),
        CallbackAdmission::Closed
    ));
    assert_eq!(
        state
            .close_cell(grpc::CellClosed {
                execution_id: "execution".to_string(),
                cell_id: "cell".to_string(),
                final_tool_call_sequence: 1,
            })
            .expect("ignore delayed cell closure"),
        None
    );
    assert!(state.close(/*failure*/ None).is_empty());
}

#[test]
fn invocation_cancellation_revokes_delegate_and_late_completion() {
    let mut state = SessionState::default();
    state
        .begin_execution(&request("execution"))
        .expect("register execution");
    state
        .admit_execution("execution", "cell")
        .expect("admit execution");
    state
        .mark_execution_ready("execution")
        .expect("claim execution");
    let invocation = tool_call("execution", /*invocation_id*/ 1);
    let CallbackAdmission::Active(cancellation) = state
        .admit_invocation(&invocation)
        .expect("accept invocation")
    else {
        panic!("invocation was not admitted");
    };

    state
        .cancel_invocation(&invocation.invocation_id)
        .expect("cancel invocation");

    assert!(cancellation.is_cancelled());
    state.finish_invocation(&invocation.invocation_id);
    assert_eq!(
        state
            .close_cell(grpc::CellClosed {
                execution_id: "execution".to_string(),
                cell_id: "cell".to_string(),
                final_tool_call_sequence: 1,
            })
            .expect("close cell"),
        Some(CellId::new("cell".to_string()))
    );
}

#[test]
fn duplicate_invocation_ids_are_rejected() {
    let mut state = SessionState::default();
    state
        .begin_execution(&request("execution"))
        .expect("register execution");
    state
        .admit_execution("execution", "cell")
        .expect("admit execution");
    let invocation = tool_call("execution", /*invocation_id*/ 1);
    state
        .admit_invocation(&invocation)
        .expect("accept invocation");
    state.finish_invocation(&invocation.invocation_id);

    assert!(state.admit_invocation(&invocation).is_err());
}

#[test]
fn tool_callbacks_must_match_the_executions_enabled_tools() {
    let mut state = SessionState::default();
    state
        .begin_execution(&request("execution"))
        .expect("register execution");

    let mut disabled = tool_call("execution", /*invocation_id*/ 1);
    disabled.tool_name = Some(grpc::ToolName {
        name: "hidden".to_string(),
        namespace: None,
    });
    assert!(matches!(
        state.admit_invocation(&disabled),
        Ok(CallbackAdmission::Rejected(error))
            if error == "code-mode tool hidden is not enabled for this execution"
    ));

    let mut wrong_namespace = tool_call("execution", /*invocation_id*/ 2);
    wrong_namespace.tool_name = Some(grpc::ToolName {
        name: "tool".to_string(),
        namespace: Some("private".to_string()),
    });
    assert!(matches!(
        state.admit_invocation(&wrong_namespace),
        Ok(CallbackAdmission::Rejected(_))
    ));

    let mut wrong_kind = tool_call("execution", /*invocation_id*/ 3);
    wrong_kind.tool_kind = grpc::ToolKind::Freeform as i32;
    assert!(matches!(
        state.admit_invocation(&wrong_kind),
        Ok(CallbackAdmission::Rejected(_))
    ));

    let mut explicit_default_namespace = tool_call("execution", /*invocation_id*/ 4);
    explicit_default_namespace.tool_name = Some(grpc::ToolName {
        name: "tool".to_string(),
        namespace: Some("functions".to_string()),
    });
    assert!(matches!(
        state.admit_invocation(&explicit_default_namespace),
        Ok(CallbackAdmission::Active(_))
    ));
    assert_eq!(state.require_open(), Ok(()));
}

#[test]
fn execution_call_ids_must_be_bounded() {
    let mut state = SessionState::default();
    let mut oversized = request("execution");
    oversized.tool_call_id = "x".repeat(grpc::MAX_IDENTIFIER_BYTES + 1);

    assert_eq!(
        state.begin_execution(&oversized),
        Err(format!(
            "gRPC code-mode host returned tool call ID exceeding {} bytes",
            grpc::MAX_IDENTIFIER_BYTES
        ))
    );
    assert_eq!(state.remove_execution("execution"), None);
}

#[test]
fn notification_call_ids_must_match_their_execution() {
    let mut state = SessionState::default();
    state
        .begin_execution(&request("execution"))
        .expect("register execution");

    let mut oversized = notification("execution", /*notification_id*/ 1);
    oversized.call_id = "x".repeat(grpc::MAX_IDENTIFIER_BYTES + 1);
    assert_eq!(
        state.admit_notification(&oversized).err(),
        Some(format!(
            "gRPC code-mode host returned notification call ID exceeding {} bytes",
            grpc::MAX_IDENTIFIER_BYTES
        ))
    );

    let mut mismatched = notification("execution", /*notification_id*/ 2);
    mismatched.call_id = "other-call".to_string();
    assert_eq!(
        state.admit_notification(&mismatched).err(),
        Some("code-mode notification call ID does not match its execution".to_string())
    );
    assert!(matches!(
        state.admit_notification(&notification("execution", /*notification_id*/ 3)),
        Ok(CallbackAdmission::Active(_))
    ));
}

#[test]
fn malformed_callback_ids_are_rejected_before_retention() {
    let mut state = SessionState::default();
    state
        .begin_execution(&request("execution"))
        .expect("register execution");
    let mut invalid_invocation = tool_call("execution", /*invocation_id*/ 1);
    invalid_invocation.invocation_id = "not-a-uuid".to_string();

    assert_eq!(
        state.admit_invocation(&invalid_invocation).err(),
        Some("code-mode tool invocation ID must be a UUID".to_string())
    );
    assert_eq!(
        state.cancel_invocation("not-a-uuid"),
        Err("code-mode tool invocation ID must be a UUID".to_string())
    );
    assert_eq!(
        state.cancel_invocation(&"x".repeat(grpc::MAX_IDENTIFIER_BYTES + 1)),
        Err("code-mode tool invocation ID must be a UUID".to_string())
    );

    let mut invalid_notification = notification("execution", /*notification_id*/ 1);
    invalid_notification.notification_id = "not-a-uuid".to_string();
    assert_eq!(
        state.admit_notification(&invalid_notification).err(),
        Some("code-mode notification ID must be a UUID".to_string())
    );

    let invocation = tool_call("execution", /*invocation_id*/ 2);
    state
        .cancel_invocation(&invocation.invocation_id)
        .expect("remember valid cancellation");
    assert!(matches!(
        state.admit_invocation(&invocation),
        Ok(CallbackAdmission::Cancelled)
    ));
}

#[test]
fn notifications_and_tools_share_the_pending_delegate_limit() {
    let mut state = SessionState::default();
    state
        .begin_execution(&request("execution"))
        .expect("register execution");

    for index in 0..MAX_PENDING_DELEGATE_CALLS {
        assert!(matches!(
            state.admit_notification(&notification("execution", index as u128 + 1)),
            Ok(CallbackAdmission::Active(_))
        ));
    }

    assert!(matches!(
        state.admit_notification(&notification("execution", /*notification_id*/ 2_000)),
        Ok(CallbackAdmission::Rejected(error))
            if error == "code-mode host exceeded its pending delegate callback limit"
    ));
    assert!(matches!(
        state.admit_invocation(&tool_call("execution", /*invocation_id*/ 1)),
        Ok(CallbackAdmission::Rejected(error))
            if error == "code-mode host exceeded its pending delegate callback limit"
    ));
    assert_eq!(state.require_open(), Ok(()));
    assert_eq!(state.finish_notification("execution"), None);
    assert!(matches!(
        state.admit_invocation(&tool_call("execution", /*invocation_id*/ 2)),
        Ok(CallbackAdmission::Active(_))
    ));
}

#[test]
fn terminated_cells_cancel_pending_notifications() {
    let mut state = SessionState::default();
    state
        .begin_execution(&request("execution"))
        .expect("register execution");
    let CallbackAdmission::Active(cancellation) = state
        .admit_notification(&notification("execution", /*notification_id*/ 1))
        .expect("admit notification")
    else {
        panic!("notification was not admitted");
    };

    state.cancel_notifications(&CellId::new("cell".to_string()));

    assert!(cancellation.is_cancelled());
    assert_eq!(state.finish_notification("execution"), None);
}

#[test]
fn disconnect_revokes_callbacks_and_returns_each_live_cell_once() {
    let mut state = SessionState::default();
    state
        .begin_execution(&request("execution"))
        .expect("register execution");
    state
        .admit_execution("execution", "cell")
        .expect("admit execution");
    let CallbackAdmission::Active(cancellation) = state
        .admit_invocation(&tool_call("execution", /*invocation_id*/ 1))
        .expect("accept invocation")
    else {
        panic!("invocation was not admitted");
    };
    let CallbackAdmission::Active(notification_cancellation) = state
        .admit_notification(&notification("execution", /*notification_id*/ 1))
        .expect("admit notification")
    else {
        panic!("notification was not admitted");
    };

    assert_eq!(
        state.close(Some("lease closed".to_string())),
        vec![CellId::new("cell".to_string())]
    );
    assert!(cancellation.is_cancelled());
    assert!(notification_cancellation.is_cancelled());
    assert!(state.close(/*failure*/ None).is_empty());
    assert_eq!(state.require_open(), Err("lease closed".to_string()));
}
