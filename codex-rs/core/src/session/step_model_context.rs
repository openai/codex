use std::sync::atomic::AtomicBool;

use codex_otel::SessionTelemetry;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;

/// Immutable model selection and model-derived state used by one sampling step.
///
/// Steps may share the same context when the selection is unchanged. Mutable warning flags are
/// therefore scoped to the selected model context rather than recreated for every request.
#[derive(Debug)]
pub(crate) struct StepModelContext {
    pub(crate) model_info: ModelInfo,
    pub(crate) collaboration_mode: CollaborationMode,
    pub(crate) reasoning_summary: ReasoningSummaryConfig,
    pub(crate) service_tier: Option<String>,
    pub(crate) session_telemetry: SessionTelemetry,
    pub(crate) server_model_warning_emitted: AtomicBool,
    pub(crate) model_verification_emitted: AtomicBool,
}

impl StepModelContext {
    pub(crate) fn reasoning_effort(&self) -> Option<ReasoningEffortConfig> {
        self.collaboration_mode.reasoning_effort()
    }

    pub(crate) fn effective_reasoning_effort(&self) -> Option<ReasoningEffortConfig> {
        if self.model_info.supports_reasoning_summaries {
            self.reasoning_effort()
                .or_else(|| self.model_info.default_reasoning_level.clone())
        } else {
            None
        }
    }

    pub(crate) fn effective_reasoning_effort_for_tracing(&self) -> String {
        self.effective_reasoning_effort()
            .map(|effort| effort.to_string())
            .unwrap_or_else(|| "default".to_string())
    }

    pub(crate) fn model_context_window(&self) -> Option<i64> {
        let effective_context_window_percent = self.model_info.effective_context_window_percent;
        self.model_info
            .resolved_context_window()
            .map(|context_window| {
                context_window.saturating_mul(effective_context_window_percent) / 100
            })
    }
}
