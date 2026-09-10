//! Code-mode tool analytics and tracing for handlers and queued nested dispatch.

use codex_analytics::AnalyticsEventsClient;
use codex_analytics::CodeModeToolCallFact;
use codex_analytics::CodeModeToolCallStatus;
use codex_analytics::TurnAnalyticsMetadata;
use codex_protocol::ThreadId;
use std::sync::Arc;
use std::time::Duration;
use tracing::Span;

pub(super) struct CodeModeToolCallGuard {
    analytics: AnalyticsEventsClient,
    thread_id: String,
    turn_id: String,
    turn_metadata: Arc<dyn TurnAnalyticsMetadata>,
    call_id: String,
    pub(super) cell_id: Option<String>,
    tool_name: &'static str,
    started_at_ms: u64,
    status: CodeModeToolCallStatus,
    handler_span: Span,
}

impl CodeModeToolCallGuard {
    pub(super) fn new(
        analytics: AnalyticsEventsClient,
        thread_id: String,
        turn_id: String,
        turn_metadata: Arc<dyn TurnAnalyticsMetadata>,
        call_id: String,
        tool_name: &'static str,
        handler_span: Span,
    ) -> Self {
        Self {
            analytics,
            thread_id,
            turn_id,
            turn_metadata,
            call_id,
            cell_id: None,
            tool_name,
            started_at_ms: codex_analytics::now_unix_millis(),
            status: CodeModeToolCallStatus::Interrupted,
            handler_span,
        }
    }

    pub(super) fn finish(&mut self, success: bool) {
        let mut outcome = "failed";
        self.status = if success {
            outcome = "completed";
            CodeModeToolCallStatus::Completed
        } else {
            CodeModeToolCallStatus::Failed
        };
        self.handler_span.record("outcome", outcome);
    }

    pub(super) fn record_code_mode_host_duration(&self, duration: Duration) {
        let Ok(code_mode_host_duration_ns) = u64::try_from(duration.as_nanos()) else {
            return;
        };
        // Bridge joins this record to the outer tool-completion event. Emit it
        // before the handler returns so consumers never need another timeout.
        tracing::info!(
            target: "codex_code_mode::timing",
            {
                event.name = "codex.code_mode.host_timing",
                conversation_id = %self.thread_id,
                turn_id = %self.turn_id,
                call_id = %self.call_id,
                cell_id = self.cell_id.as_deref(),
                tool_name = self.tool_name,
                code_mode_host_duration_ns,
            },
            "code-mode host operation completed"
        );
    }
}

impl Drop for CodeModeToolCallGuard {
    fn drop(&mut self) {
        self.analytics
            .track_code_mode_tool_call(CodeModeToolCallFact::Completed {
                thread_id: self.thread_id.clone(),
                turn_id: self.turn_id.clone(),
                turn_metadata: self.turn_metadata.clone(),
                call_id: self.call_id.clone(),
                cell_id: self.cell_id.clone(),
                tool_name: self.tool_name.to_string(),
                started_at_ms: self.started_at_ms,
                completed_at_ms: codex_analytics::now_unix_millis(),
                status: self.status,
            });
    }
}

pub(super) struct NestedToolDispatchTrace {
    thread_id: ThreadId,
    pub(super) call_id: String,
    pub(super) interruption: Option<DispatchInterruption>,
    pub(super) span: Span,
}

impl NestedToolDispatchTrace {
    pub(super) fn new(thread_id: ThreadId, call_id: String) -> Self {
        Self {
            thread_id,
            call_id,
            interruption: Some(DispatchInterruption::Abandoned),
            span: Span::current(),
        }
    }
}

impl Drop for NestedToolDispatchTrace {
    fn drop(&mut self) {
        let Some(outcome) = self.interruption.take() else {
            return;
        };
        let outcome = match outcome {
            DispatchInterruption::Cancelled => "cancelled",
            DispatchInterruption::CellClosed => "cell_closed",
            DispatchInterruption::Abandoned => "abandoned",
        };
        tracing::event!(
            name: "codex.code_mode.nested_tool_dispatch_interrupted",
            target: "codex_otel.trace_safe",
            parent: &self.span,
            tracing::Level::INFO,
            event.name = "codex.code_mode.nested_tool_dispatch_interrupted",
            conversation.id = %self.thread_id,
            call_id = self.call_id.as_str(),
            outcome,
        );
    }
}

pub(super) enum DispatchInterruption {
    Cancelled,
    CellClosed,
    Abandoned,
}

pub(super) fn trace_id(id: &str) -> Option<&str> {
    (!id.is_empty() && id.len() <= 256).then_some(id)
}
