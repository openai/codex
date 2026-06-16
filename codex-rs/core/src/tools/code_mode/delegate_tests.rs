use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CodeModeSessionDelegate;

use super::CodeModeDispatchBroker;

#[test]
fn terminal_notification_removes_a_ready_cell_dispatch_gate() {
    let broker = CodeModeDispatchBroker::new();
    let cell_id = CellId::new("cell-a7".to_string());
    broker.mark_cell_ready_for_dispatch(&cell_id);
    assert!(
        broker
            .dispatch_gates
            .lock()
            .unwrap()
            .gates
            .contains_key(&cell_id)
    );

    CodeModeSessionDelegate::cell_closed(&broker, &cell_id);

    assert!(
        !broker
            .dispatch_gates
            .lock()
            .unwrap()
            .gates
            .contains_key(&cell_id)
    );
}

#[test]
fn terminal_notification_before_readiness_does_not_leave_a_dispatch_gate() {
    let broker = CodeModeDispatchBroker::new();
    let cell_id = CellId::new("cell-b8".to_string());

    CodeModeSessionDelegate::cell_closed(&broker, &cell_id);
    broker.mark_cell_ready_for_dispatch(&cell_id);

    let dispatch_gates = broker.dispatch_gates.lock().unwrap();
    assert!(!dispatch_gates.gates.contains_key(&cell_id));
    assert!(!dispatch_gates.terminal_before_ready.contains(&cell_id));
}
