//! Caller-side RPC attempt counts using the client's protocol method names, independent of tracing.

use codex_otel::EXEC_SERVER_CLIENT_REQUEST_COUNT_METRIC;
use codex_otel::MetricsClient;

pub(crate) fn record_client_request(metrics: Option<&MetricsClient>, method: &str) {
    let Some(metrics) = metrics else {
        return;
    };
    // Record before local admission so failures and cancelled calls still count
    // as attempts. Notifications and responses never enter these call paths.
    if metrics
        .counter_with_description(
            EXEC_SERVER_CLIENT_REQUEST_COUNT_METRIC,
            "Total number of client-side exec-server RPC attempts, including local failures.",
            /*inc*/ 1,
            &[("method", method)],
        )
        .is_err()
    {
        tracing::warn!("failed to emit exec-server client request counter");
    }
}
