use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::tools::sandboxing::ToolCtx;
use codex_analytics::PluginMeasurementsInput;
use codex_core_plugins::PluginMetricsSidecar;
use codex_exec_server::Environment;
use codex_utils_path_uri::PathUri;

/// Creates a metrics sidecar for one plugin command.
pub(crate) async fn sidecar_for_command(
    ctx: &ToolCtx,
    command: &[String],
    cwd: &PathUri,
    environment: &Environment,
) -> Option<PluginMetricsSidecar> {
    if !ctx.session.services.analytics_events_client.is_enabled() {
        return None;
    }
    let resolved = ctx
        .step_context
        .turn
        .plugin_metrics_operation_for_command(command, cwd, environment)
        .await?;
    if environment.is_remote() {
        PluginMetricsSidecar::create_remote(environment, resolved).await
    } else {
        PluginMetricsSidecar::create(resolved)
    }
}

/// Finishes a metrics sidecar and publishes any valid rows.
pub(crate) async fn finish_and_track_measurements(
    metrics_sidecar: Option<PluginMetricsSidecar>,
    exit_code: i32,
    session: &Session,
    turn: &TurnContext,
    item_id: &str,
) {
    let Some(metrics_sidecar) = metrics_sidecar else {
        return;
    };
    let Some(batch) = metrics_sidecar.finish(exit_code).await else {
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
