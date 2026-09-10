//! Turn token metrics grouped by the model that actually produced each response.
//! Multiple responses from one model contribute a single histogram sample per turn.

use std::collections::BTreeMap;

use codex_otel::SessionTelemetry;
use codex_otel::TURN_TOKEN_USAGE_METRIC;
use codex_protocol::protocol::TokenUsage;

#[derive(Default)]
pub(crate) struct TurnTokenUsage {
    by_model: BTreeMap<String, (SessionTelemetry, TokenUsage)>,
}

impl TurnTokenUsage {
    pub(crate) fn record(&mut self, model: &str, telemetry: SessionTelemetry, usage: &TokenUsage) {
        let (_, total) = self
            .by_model
            .entry(model.to_owned())
            .or_insert_with(|| (telemetry, TokenUsage::default()));
        total.add_assign(usage);
    }

    pub(crate) fn emit(self, fallback: &SessionTelemetry, tmp_mem: (&str, &str)) {
        let mut usage = self.by_model.into_values().collect::<Vec<_>>();
        // Preserve zero-valued completion samples for turns without reported usage.
        // Do not create a sample for the selected model if only compaction ran.
        if usage.is_empty() {
            usage.push((fallback.clone(), TokenUsage::default()));
        }
        for (telemetry, usage) in usage {
            for (token_type, value) in [
                ("total", usage.total_tokens),
                ("input", usage.input_tokens),
                ("cached_input", usage.cached_input()),
                ("cache_write_input", usage.cache_write_input_tokens),
                ("output", usage.output_tokens),
                ("reasoning_output", usage.reasoning_output_tokens),
            ] {
                telemetry.histogram(
                    TURN_TOKEN_USAGE_METRIC,
                    value.max(0),
                    &[("token_type", token_type), tmp_mem],
                );
            }
        }
    }
}
