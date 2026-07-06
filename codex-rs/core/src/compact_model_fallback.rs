use codex_analytics::CompactionImplementation;
use codex_analytics::CompactionReason;
use codex_otel::SessionTelemetry;
use codex_protocol::error::CodexErr;
use serde_json::Value;
use tracing::warn;

#[cfg(test)]
#[path = "compact_model_fallback_tests.rs"]
mod tests;

pub(crate) fn is_model_unavailable_error(error: &CodexErr, model: &str) -> bool {
    let CodexErr::InvalidRequest(message) = error else {
        return false;
    };
    let expected =
        format!("The '{model}' model is not supported when using Codex with a ChatGPT account.");
    if message == &expected {
        return true;
    }
    serde_json::from_str::<Value>(message)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .is_some_and(|message| message == expected)
}

pub(crate) fn record_model_fallback(
    session_telemetry: &SessionTelemetry,
    previous_model: &str,
    current_model: &str,
    reason: CompactionReason,
    implementation: CompactionImplementation,
    succeeded: bool,
) {
    let reason_tag = match reason {
        CompactionReason::UserRequested => "user_requested",
        CompactionReason::ContextLimit => "context_limit",
        CompactionReason::ModelDownshift => "model_downshift",
        CompactionReason::CompHashChanged => "comp_hash_changed",
    };
    let implementation_tag = match implementation {
        CompactionImplementation::Responses => "responses",
        CompactionImplementation::ResponsesCompactionV2 => "responses_compaction_v2",
        CompactionImplementation::ResponsesCompact => "responses_compact",
    };
    let outcome = if succeeded { "succeeded" } else { "failed" };
    session_telemetry.counter(
        "codex.compaction.model_fallback",
        /*inc*/ 1,
        &[
            ("reason", reason_tag),
            ("implementation", implementation_tag),
            ("outcome", outcome),
        ],
    );
    warn!(
        previous_model,
        current_model,
        ?reason,
        ?implementation,
        outcome,
        "previous model was unavailable during compaction; retried with current model"
    );
}
