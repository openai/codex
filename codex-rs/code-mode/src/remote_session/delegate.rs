//! Restores per-execution trace context for callbacks from the shared stdio host.
//!
//! Keep only a W3C snapshot: retaining the caller's tracing span would extend a yielded
//! exec's duration until its background cell closes. The gRPC transport has its own
//! callback propagation and does not use this wrapper.
//! Instrument manually because `#[instrument]` enters the span before its body runs,
//! which is too late to assign the remote OpenTelemetry parent.

use std::sync::Arc;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CodeModeNestedToolCall;
use codex_code_mode_protocol::CodeModeSessionDelegate;
use codex_code_mode_protocol::NotificationFuture;
use codex_code_mode_protocol::ToolInvocationFuture;
use codex_protocol::protocol::W3cTraceContext;
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

pub(super) struct TracedCellDelegate {
    pub(super) delegate: Arc<dyn CodeModeSessionDelegate>,
    pub(super) trace: Option<W3cTraceContext>,
}

impl TracedCellDelegate {
    fn with_parent(&self, span: tracing::Span) -> tracing::Span {
        // Set the remote parent before entering the span: OTEL starts it on entry.
        if let Some(trace) = &self.trace {
            codex_otel::set_parent_from_w3c_trace_context(&span, trace);
        }
        span
    }
}

impl CodeModeSessionDelegate for TracedCellDelegate {
    fn invoke_tool<'a>(
        &'a self,
        invocation: CodeModeNestedToolCall,
        cancellation_token: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        let span = self.with_parent(tracing::info_span!(parent: None, "code_mode.stdio.callback"));
        Box::pin(
            async move {
                self.delegate
                    .invoke_tool(invocation, cancellation_token)
                    .await
            }
            .instrument(span),
        )
    }

    fn notify<'a>(
        &'a self,
        call_id: String,
        cell_id: CellId,
        text: String,
        cancellation_token: CancellationToken,
    ) -> NotificationFuture<'a> {
        let span = self.with_parent(tracing::info_span!(parent: None, "code_mode.stdio.notify"));
        Box::pin(
            async move {
                self.delegate
                    .notify(call_id, cell_id, text, cancellation_token)
                    .await
            }
            .instrument(span),
        )
    }

    fn cell_closed(&self, cell_id: &CellId) {
        self.delegate.cell_closed(cell_id);
    }
}
