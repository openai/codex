use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::grpc;
use pretty_assertions::assert_eq;

use super::SessionState;

#[test]
fn cell_closure_waits_until_the_started_cell_is_claimed() {
    let mut state = SessionState::default();
    state
        .begin_execution("execution".to_string())
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
        .begin_execution("execution".to_string())
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
fn abandonment_before_start_ignores_later_cell_closure() {
    let mut state = SessionState::default();
    state
        .begin_execution("execution".to_string())
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
fn disconnect_returns_each_live_cell_once() {
    let mut state = SessionState::default();
    state
        .begin_execution("execution".to_string())
        .expect("register execution");
    state
        .admit_execution("execution", "cell")
        .expect("admit execution");

    assert_eq!(
        state.close(Some("lease closed".to_string())),
        vec![CellId::new("cell".to_string())]
    );
    assert!(state.close(/*failure*/ None).is_empty());
    assert_eq!(state.require_open(), Err("lease closed".to_string()));
}
