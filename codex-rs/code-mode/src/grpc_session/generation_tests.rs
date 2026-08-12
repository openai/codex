use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CodeModeNestedToolCall;
use codex_code_mode_protocol::CodeModeSessionDelegate;
use codex_code_mode_protocol::CodeModeToolKind;
use codex_code_mode_protocol::FunctionCallOutputContentItem;
use codex_code_mode_protocol::NotificationFuture;
use codex_code_mode_protocol::RuntimeResponse;
use codex_code_mode_protocol::StartedCell;
use codex_code_mode_protocol::ToolInvocationFuture;
use codex_code_mode_protocol::WaitOutcome;
use codex_protocol::ToolName;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use super::GenerationDelegate;
use super::public_cell_id;
use super::public_started_cell;
use super::public_wait_outcome;
use super::remote_cell_id;

#[derive(Default)]
struct RecordingDelegate {
    calls: Mutex<Vec<CodeModeNestedToolCall>>,
    notifications: Mutex<Vec<(String, CellId, String)>>,
    closed: Mutex<Vec<CellId>>,
}

impl CodeModeSessionDelegate for RecordingDelegate {
    fn invoke_tool<'a>(
        &'a self,
        invocation: CodeModeNestedToolCall,
        _cancellation_token: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        self.calls.lock().expect("calls lock").push(invocation);
        Box::pin(async { Ok(json!({ "ok": true })) })
    }

    fn notify<'a>(
        &'a self,
        call_id: String,
        cell_id: CellId,
        text: String,
        _cancellation_token: CancellationToken,
    ) -> NotificationFuture<'a> {
        self.notifications
            .lock()
            .expect("notifications lock")
            .push((call_id, cell_id, text));
        Box::pin(async { Ok(()) })
    }

    fn cell_closed(&self, cell_id: &CellId) {
        self.closed
            .lock()
            .expect("closed cells lock")
            .push(cell_id.clone());
    }
}

#[test]
fn first_generation_preserves_existing_cell_ids() {
    let cell_id = CellId::new("42".to_string());

    assert_eq!(public_cell_id(/*generation*/ 1, &cell_id), cell_id);
    assert_eq!(remote_cell_id(/*generation*/ 1, &cell_id), Ok(cell_id));
}

#[test]
fn first_generation_preserves_opaque_cell_ids_that_resemble_generation_prefixes() {
    for value in ["graphics:1", "g2:42"] {
        let cell_id = CellId::new(value.to_string());

        assert_eq!(public_cell_id(/*generation*/ 1, &cell_id), cell_id);
        assert_eq!(remote_cell_id(/*generation*/ 1, &cell_id), Ok(cell_id));
    }
}

#[test]
fn later_generations_prefix_public_ids_and_strip_wire_ids() {
    let wire_id = CellId::new("42".to_string());
    let public_id = CellId::new("g2:42".to_string());

    assert_eq!(public_cell_id(/*generation*/ 2, &wire_id), public_id);
    assert_eq!(remote_cell_id(/*generation*/ 2, &public_id), Ok(wire_id));
}

#[test]
fn stale_generation_ids_are_rejected_after_reconnection() {
    for cell_id in [
        "42".to_string(),
        "g1:42".to_string(),
        "g3:42".to_string(),
        "x".repeat(10_000),
    ] {
        assert_eq!(
            remote_cell_id(/*generation*/ 2, &CellId::new(cell_id)),
            Err("cell belongs to a stale code-mode host generation".to_string())
        );
    }
}

#[tokio::test]
async fn reconnect_maps_every_delegate_callback_to_its_generation() {
    let recording = Arc::new(RecordingDelegate::default());
    let delegate = GenerationDelegate {
        delegate: recording.clone(),
        generation: 2,
    };
    let wire_id = CellId::new("42".to_string());
    let public_id = CellId::new("g2:42".to_string());
    let invocation = CodeModeNestedToolCall {
        cell_id: wire_id.clone(),
        runtime_tool_call_id: "runtime-call".to_string(),
        tool_name: ToolName::plain("echo"),
        tool_kind: CodeModeToolKind::Function,
        input: Some(json!({ "value": true })),
    };

    assert_eq!(
        delegate
            .invoke_tool(invocation.clone(), CancellationToken::new())
            .await,
        Ok(json!({ "ok": true }))
    );
    assert_eq!(
        delegate
            .notify(
                "outer-call".to_string(),
                wire_id.clone(),
                "notice".to_string(),
                CancellationToken::new(),
            )
            .await,
        Ok(())
    );
    delegate.cell_closed(&wire_id);

    assert_eq!(
        *recording.calls.lock().expect("calls lock"),
        vec![CodeModeNestedToolCall {
            cell_id: public_id.clone(),
            ..invocation
        }]
    );
    assert_eq!(
        *recording.notifications.lock().expect("notifications lock"),
        vec![(
            "outer-call".to_string(),
            public_id.clone(),
            "notice".to_string()
        )]
    );
    assert_eq!(
        *recording.closed.lock().expect("closed cells lock"),
        vec![public_id]
    );
}

#[tokio::test]
async fn reconnected_execution_maps_started_and_initial_response_ids() {
    let (response_tx, response_rx) = oneshot::channel();
    let wire_id = CellId::new("42".to_string());
    let public_id = CellId::new("g2:42".to_string());
    let claimed = Arc::new(AtomicBool::new(false));
    let initial_response_claimed = Arc::clone(&claimed);
    let response = RuntimeResponse::Result {
        cell_id: wire_id.clone(),
        content_items: vec![FunctionCallOutputContentItem::InputText {
            text: "result".to_string(),
        }],
        error_text: None,
    };
    let started = StartedCell::from_future(wire_id, async move {
        initial_response_claimed.store(true, Ordering::Release);
        response_rx.await.expect("receive initial response")
    });
    let started = public_started_cell(/*generation*/ 2, started);

    assert_eq!(started.cell_id, public_id);
    tokio::task::yield_now().await;
    assert!(!claimed.load(Ordering::Acquire));
    response_tx
        .send(Ok(response))
        .expect("send initial response");
    assert_eq!(
        started.initial_response().await,
        Ok(RuntimeResponse::Result {
            cell_id: public_id,
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "result".to_string(),
            }],
            error_text: None,
        })
    );
}

#[test]
fn reconnected_wait_maps_live_and_missing_outcomes() {
    let public_id = CellId::new("g2:42".to_string());
    let yielded = RuntimeResponse::Yielded {
        cell_id: CellId::new("42".to_string()),
        content_items: Vec::new(),
    };
    let terminated = RuntimeResponse::Terminated {
        cell_id: CellId::new("42".to_string()),
        content_items: Vec::new(),
    };

    assert_eq!(
        public_wait_outcome(/*generation*/ 2, WaitOutcome::LiveCell(yielded)),
        WaitOutcome::LiveCell(RuntimeResponse::Yielded {
            cell_id: public_id.clone(),
            content_items: Vec::new(),
        })
    );
    assert_eq!(
        public_wait_outcome(/*generation*/ 2, WaitOutcome::MissingCell(terminated)),
        WaitOutcome::MissingCell(RuntimeResponse::Terminated {
            cell_id: public_id,
            content_items: Vec::new(),
        })
    );
}
