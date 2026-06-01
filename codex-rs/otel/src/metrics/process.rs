use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use super::client::MetricsClient;
use super::error::Result;
use super::names::PROCESS_START_METRIC;
use super::tags::ORIGINATOR_TAG;
use super::tags::bounded_originator_tag_value;

static PROCESS_START_RECORDED: AtomicBool = AtomicBool::new(false);

/// Record the process start counter at most once for this process.
pub fn record_process_start_once(metrics: &MetricsClient, originator: &str) -> Result<bool> {
    if PROCESS_START_RECORDED
        .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
        .is_err()
    {
        return Ok(false);
    }

    metrics.counter(
        PROCESS_START_METRIC,
        /*inc*/ 1,
        &[(ORIGINATOR_TAG, bounded_originator_tag_value(originator))],
    )?;
    Ok(true)
}
