use codex_analytics::AnalyticsEventsClient;
use codex_analytics::CodeModeToolCallFact;
use codex_analytics::CodeModeToolCallStatus;
use codex_analytics::TurnAnalyticsMetadata;
use std::sync::Arc;

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
}

impl CodeModeToolCallGuard {
    pub(super) fn new(
        analytics: AnalyticsEventsClient,
        thread_id: String,
        turn_id: String,
        turn_metadata: Arc<dyn TurnAnalyticsMetadata>,
        call_id: String,
        tool_name: &'static str,
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
        }
    }

    pub(super) fn finish(&mut self, success: bool) {
        self.status = if success {
            CodeModeToolCallStatus::Completed
        } else {
            CodeModeToolCallStatus::Failed
        };
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
