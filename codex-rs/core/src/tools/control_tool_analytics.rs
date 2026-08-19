use crate::tools::context::ToolCallSource;
use crate::tools::context::ToolInvocation;
use codex_analytics::ControlToolCallFact;
use codex_analytics::ControlToolCallStatus;

pub(crate) struct ControlToolCallGuard<'a> {
    invocation: &'a ToolInvocation,
    started_at_ms: u64,
    completed_at_ms: Option<u64>,
    status: ControlToolCallStatus,
}

impl<'a> ControlToolCallGuard<'a> {
    pub(crate) fn new(invocation: &'a ToolInvocation) -> Self {
        Self {
            invocation,
            started_at_ms: codex_analytics::now_unix_millis(),
            completed_at_ms: None,
            status: ControlToolCallStatus::Interrupted,
        }
    }

    pub(crate) fn finish(&mut self, status: ControlToolCallStatus) {
        self.completed_at_ms = Some(codex_analytics::now_unix_millis());
        self.status = status;
    }
}

impl Drop for ControlToolCallGuard<'_> {
    fn drop(&mut self) {
        let invocation = self.invocation;
        invocation
            .session
            .services
            .analytics_events_client
            .track_control_tool_call(ControlToolCallFact {
                thread_id: invocation.session.thread_id.to_string(),
                turn_id: invocation.turn.sub_id.clone(),
                call_id: invocation.call_id.clone(),
                cell_id: match &invocation.source {
                    ToolCallSource::CodeMode { cell_id, .. } => Some(cell_id.clone()),
                    ToolCallSource::Direct | ToolCallSource::DirectPlaintextMessage => None,
                },
                tool_name: invocation.tool_name.name.clone(),
                started_at_ms: self.started_at_ms,
                completed_at_ms: self
                    .completed_at_ms
                    .unwrap_or_else(codex_analytics::now_unix_millis),
                status: self.status,
            });
    }
}
