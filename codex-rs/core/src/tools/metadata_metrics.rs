//! Measures bytes shed by existing metadata budgets without changing their policy.
//! Samples describe budget applications, not unique tool calls or full request sizes.

use codex_otel::MetricsClient;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::executed_tool_call_metadata_bytes;

const SHED_BYTES_METRIC: &str = "codex.tool_metadata.shed_bytes";
const BYTE_BUCKETS: &[f64] = &[
    1024.0, 4096.0, 16384.0, 32768.0, 65536.0, 131072.0, 262144.0, 524288.0, 1048576.0, 2097152.0,
    4194304.0, 8388608.0, 16777216.0,
];

pub(super) fn bound_prompt_metadata(
    items: &mut [ResponseItem],
    bound: fn(&mut [ResponseItem]),
    stage: &'static str,
    metrics: Option<&MetricsClient>,
) {
    let before = metrics.map(|_| metadata_bytes(items));
    bound(items);
    if let Some(before) = before {
        // Measure only the budget operation: attaching observations can itself add bytes.
        record_shedding(stage, before, metadata_bytes(items), metrics);
    }
}

pub(crate) fn metadata_bytes(items: &[ResponseItem]) -> usize {
    items.iter().fold(0_usize, |bytes, item| {
        bytes.saturating_add(executed_tool_call_metadata_bytes(item))
    })
}

pub(crate) fn record_shedding(
    stage: &'static str,
    before: usize,
    after: usize,
    metrics: Option<&MetricsClient>,
) {
    let Some(metrics) = metrics else {
        return;
    };
    let shed = before.saturating_sub(after);
    if shed == 0 || before == usize::MAX || after == usize::MAX {
        return;
    }
    // Count the complete attempted-tool metadata envelope, including argument and
    // inventory truncation performed by retention/request budgets.
    if let Err(error) = metrics.histogram_with_boundaries(
        SHED_BYTES_METRIC,
        i64::try_from(shed).unwrap_or(i64::MAX),
        BYTE_BUCKETS,
        &[("stage", stage)],
    ) {
        tracing::warn!("tool metadata shedding metric failed: {error}");
    }
}

#[cfg(test)]
#[path = "metadata_metrics_tests.rs"]
mod tests;
