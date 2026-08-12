use std::sync::Arc;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CodeModeNestedToolCall;
use codex_code_mode_protocol::CodeModeSessionDelegate;
use codex_code_mode_protocol::NotificationFuture;
use codex_code_mode_protocol::RuntimeResponse;
use codex_code_mode_protocol::StartedCell;
use codex_code_mode_protocol::ToolInvocationFuture;
use codex_code_mode_protocol::WaitOutcome;
use tokio_util::sync::CancellationToken;

pub(super) struct GenerationDelegate {
    pub(super) delegate: Arc<dyn CodeModeSessionDelegate>,
    pub(super) generation: u64,
}

impl CodeModeSessionDelegate for GenerationDelegate {
    fn invoke_tool<'a>(
        &'a self,
        mut invocation: CodeModeNestedToolCall,
        cancellation_token: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        invocation.cell_id = public_cell_id(self.generation, &invocation.cell_id);
        self.delegate.invoke_tool(invocation, cancellation_token)
    }

    fn notify<'a>(
        &'a self,
        call_id: String,
        cell_id: CellId,
        text: String,
        cancellation_token: CancellationToken,
    ) -> NotificationFuture<'a> {
        self.delegate.notify(
            call_id,
            public_cell_id(self.generation, &cell_id),
            text,
            cancellation_token,
        )
    }

    fn cell_closed(&self, cell_id: &CellId) {
        self.delegate
            .cell_closed(&public_cell_id(self.generation, cell_id));
    }
}

fn public_cell_id(generation: u64, cell_id: &CellId) -> CellId {
    if generation == 1 {
        cell_id.clone()
    } else {
        CellId::new(format!("g{generation}:{cell_id}"))
    }
}

pub(super) fn remote_cell_id(generation: u64, cell_id: &CellId) -> Result<CellId, String> {
    if generation == 1 {
        return Ok(cell_id.clone());
    }

    let prefix = format!("g{generation}:");
    cell_id
        .as_str()
        .strip_prefix(&prefix)
        .map(|cell_id| CellId::new(cell_id.to_string()))
        .ok_or_else(|| "cell belongs to a stale code-mode host generation".to_string())
}

pub(super) fn public_started_cell(generation: u64, started: StartedCell) -> StartedCell {
    if generation == 1 {
        return started;
    }
    let cell_id = public_cell_id(generation, &started.cell_id);
    StartedCell::from_future(cell_id, async move {
        started
            .initial_response()
            .await
            .map(|response| public_runtime_response(generation, response))
    })
}

fn public_runtime_response(generation: u64, response: RuntimeResponse) -> RuntimeResponse {
    match response {
        RuntimeResponse::Yielded {
            cell_id,
            content_items,
        } => RuntimeResponse::Yielded {
            cell_id: public_cell_id(generation, &cell_id),
            content_items,
        },
        RuntimeResponse::Terminated {
            cell_id,
            content_items,
        } => RuntimeResponse::Terminated {
            cell_id: public_cell_id(generation, &cell_id),
            content_items,
        },
        RuntimeResponse::Result {
            cell_id,
            content_items,
            error_text,
        } => RuntimeResponse::Result {
            cell_id: public_cell_id(generation, &cell_id),
            content_items,
            error_text,
        },
    }
}

pub(super) fn public_wait_outcome(generation: u64, outcome: WaitOutcome) -> WaitOutcome {
    match outcome {
        WaitOutcome::LiveCell(response) => {
            WaitOutcome::LiveCell(public_runtime_response(generation, response))
        }
        WaitOutcome::MissingCell(response) => {
            WaitOutcome::MissingCell(public_runtime_response(generation, response))
        }
    }
}

#[cfg(test)]
#[path = "generation_tests.rs"]
mod tests;
