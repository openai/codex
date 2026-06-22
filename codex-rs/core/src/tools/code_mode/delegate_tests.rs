use std::sync::Arc;
use std::time::Duration;

use codex_code_mode::CellId;
use codex_code_mode::CodeModeService;
use codex_code_mode::CodeModeSessionDelegate;
use codex_code_mode::CreateCellRequest;
use codex_code_mode::ObservationGeneration;
use codex_code_mode::ObserveOutcome;
use codex_code_mode::ObserveRequest;

use super::CodeModeDispatchBroker;

#[test]
fn terminal_notification_removes_the_cell_dispatch_gate() {
    let broker = CodeModeDispatchBroker::new();
    let cell_id = CellId::new("cell-a7".to_string());
    broker.mark_cell_ready_for_dispatch(&cell_id);
    assert!(broker.dispatch_gates.lock().unwrap().contains_key(&cell_id));

    CodeModeSessionDelegate::cell_closed(&broker, &cell_id);

    assert!(!broker.dispatch_gates.lock().unwrap().contains_key(&cell_id));
}

#[test]
fn terminal_notification_before_readiness_does_not_reopen_the_dispatch_gate() {
    let broker = CodeModeDispatchBroker::new();
    let cell_id = CellId::new("cell-a7".to_string());

    CodeModeSessionDelegate::cell_closed(&broker, &cell_id);
    assert!(matches!(
        broker.dispatch_gates.lock().unwrap().get(&cell_id),
        Some(super::DispatchGate::ClosedBeforeReady)
    ));

    broker.mark_cell_ready_for_dispatch(&cell_id);

    assert!(!broker.dispatch_gates.lock().unwrap().contains_key(&cell_id));
}

#[tokio::test]
async fn background_completion_removes_the_dispatch_gate_without_another_observation() {
    let broker = Arc::new(CodeModeDispatchBroker::new());
    let service = CodeModeService::with_delegate(broker.clone());
    let requested_cell_id = CellId::new("cell-a7".to_string());
    let cell_id = service
        .create_cell(CreateCellRequest {
            cell_id: requested_cell_id,
            tool_call_id: "call-1".to_string(),
            enabled_tools: Vec::new(),
            source: concat!(
                "await new Promise(resolve => setTimeout(resolve, 100));",
                "text('done');",
            )
            .to_string(),
        })
        .await
        .unwrap();
    broker.mark_cell_ready_for_dispatch(&cell_id);

    assert_eq!(
        service
            .observe(ObserveRequest {
                cell_id: cell_id.clone(),
                generation: ObservationGeneration::INITIAL,
                yield_time_ms: 1,
            })
            .await
            .unwrap(),
        ObserveOutcome::Yielded {
            cell_id: cell_id.clone(),
            content_items: Vec::new(),
        }
    );
    tokio::time::timeout(Duration::from_secs(/*secs*/ 2), async {
        while broker.dispatch_gates.lock().unwrap().contains_key(&cell_id) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("terminal notification should remove the dispatch gate");

    service.shutdown().await.unwrap();
}
