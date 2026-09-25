//! Successful multi-agent spawn latency metrics.
//!
//! Runtime values are collapsed to bounded labels before emission. Failed spawns do not emit
//! partial phase timings.

use crate::agent::types::SpawnAgentForkMode;
use codex_otel::SessionTelemetry;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::ThreadHistoryMode;
use std::time::Duration;

pub(super) struct SpawnMeasurements {
    pub(super) history_mode: ThreadHistoryMode,
    pub(super) residency_reservation: Option<Duration>,
    pub(super) fork_context: Option<Duration>,
    pub(super) child_create: Duration,
    pub(super) durability_wait: Duration,
    pub(super) input_admission: Duration,
    pub(super) total: Duration,
}

pub(super) fn record_spawn_success(
    telemetry: &SessionTelemetry,
    fork_mode: Option<&SpawnAgentForkMode>,
    multi_agent_version: MultiAgentVersion,
    measurements: SpawnMeasurements,
) {
    let fork_mode = match fork_mode {
        None => "none",
        Some(SpawnAgentForkMode::FullHistory) => "all",
        Some(SpawnAgentForkMode::LastNTurns(_)) => "last_n",
    };
    let history_mode = match measurements.history_mode {
        ThreadHistoryMode::Legacy => "legacy",
        ThreadHistoryMode::Paginated => "paginated",
    };
    let multi_agent_version = match multi_agent_version {
        MultiAgentVersion::Disabled => "disabled",
        MultiAgentVersion::V1 => "v1",
        MultiAgentVersion::V2 => "v2",
    };
    let record_phase = |phase, duration| {
        telemetry.record_multi_agent_spawn_phase(
            phase,
            duration,
            fork_mode,
            history_mode,
            multi_agent_version,
        );
    };
    if let Some(duration) = measurements.residency_reservation {
        record_phase("residency_reservation", duration);
    }
    if let Some(duration) = measurements.fork_context {
        record_phase("fork_context", duration);
    }
    record_phase("child_create", measurements.child_create);
    record_phase("durability_wait", measurements.durability_wait);
    record_phase("input_admission", measurements.input_admission);
    record_phase("total", measurements.total);
}

#[cfg(test)]
#[path = "spawn_telemetry_tests.rs"]
mod tests;
