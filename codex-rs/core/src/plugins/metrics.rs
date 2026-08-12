use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use codex_analytics::PluginMeasurementsInput;
use codex_core_plugins::PluginMetricsSidecar;

/// Finishes a metrics sidecar and publishes any valid rows.
pub(crate) fn finish_and_track_measurements(
    metrics_sidecar: Option<PluginMetricsSidecar>,
    exit_code: i32,
    session: &Session,
    turn: &TurnContext,
    item_id: &str,
) {
    let Some(metrics_sidecar) = metrics_sidecar else {
        return;
    };
    let Some(batch) = metrics_sidecar.finish(exit_code) else {
        return;
    };
    session
        .services
        .analytics_events_client
        .track_plugin_measurements(PluginMeasurementsInput {
            thread_id: session.thread_id().to_string(),
            turn_id: turn.sub_id.clone(),
            item_id: item_id.to_string(),
            plugin_id: batch.plugin_id,
            execution_id: batch.execution_id,
            operation: batch.operation,
            rows: batch.rows,
        });
}
