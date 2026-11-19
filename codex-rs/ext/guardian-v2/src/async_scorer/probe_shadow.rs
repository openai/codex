//! Logs existing probe flags alongside async results without waiting for other review stages.
//! Action IDs join these records to tool observations and synchronous review outcomes.

use std::time::Duration;

use codex_core::context::GuardianContextMode;
use codex_extension_api::ToolCallSource;
use codex_extension_api::ToolStartInput;

pub(super) struct ProbeComparison {
    probe: Option<bool>,
    scope: &'static str,
    model: String,
    thread_id: String,
    turn_id: String,
    call_id: String,
}

impl ProbeComparison {
    pub(super) fn capture(input: &ToolStartInput<'_>) -> Self {
        Self {
            probe: input.guardian_probe,
            scope: match input.source {
                ToolCallSource::Direct => "tool_item",
                ToolCallSource::CodeMode { .. } => "enclosing_item",
            },
            model: input.sampling_model.to_owned(),
            thread_id: input.thread_store.level_id().to_owned(),
            turn_id: input.turn_id.to_owned(),
            call_id: input.call_id.to_owned(),
        }
    }

    pub(super) fn record(
        &self,
        context_mode: GuardianContextMode,
        outcome: &str,
        risk: Option<&str>,
        duration: Duration,
        failure_reason: Option<&str>,
    ) {
        tracing::event!(
            target: "codex_otel.log_only",
            tracing::Level::INFO,
            event.name = "codex.guardian_probe_comparison",
            stage = "async_scorer",
            thread_id = %self.thread_id,
            turn_id = %self.turn_id,
            call_id = %self.call_id,
            model = %self.model,
            guardian_context_mode = context_mode.as_str(),
            scope = self.scope,
            probe_review = match self.probe {
                Some(true) => "required",
                Some(false) => "not_required",
                None => "not_reported",
            },
            async_outcome = outcome,
            async_risk = risk.unwrap_or("not_reported"),
            failure_reason,
            duration_ms = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
            "Guardian probe comparison"
        );
    }
}
