//! Turn critical-path facts and their nullable analytics fields.
//! Missing capture never becomes a zero-duration measurement.

use serde::Serialize;

/// Capture coverage for one finalized turn, independent of its completion status.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub enum TurnCriticalPath {
    #[default]
    Disabled,
    Incomplete,
    Complete(TurnCriticalPathDurations),
}

/// Category totals over the server turn interval, truncated to milliseconds.
/// Sampling and compaction both contribute to inference. Independent truncation
/// can leave the category sum up to five milliseconds below duration_ms.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TurnCriticalPathDurations {
    pub duration_ms: u64,
    pub inference_ms: u64,
    pub auto_review_ms: u64,
    pub user_wait_ms: u64,
    pub tools_ms: u64,
    pub overhead_ms: u64,
    pub unknown_ms: u64,
}

#[derive(Serialize)]
pub(crate) struct CriticalPathEventParams {
    critical_path_status: &'static str,
    critical_path_duration_ms: Option<u64>,
    critical_path_inference_ms: Option<u64>,
    critical_path_auto_review_ms: Option<u64>,
    critical_path_user_wait_ms: Option<u64>,
    critical_path_tools_ms: Option<u64>,
    critical_path_overhead_ms: Option<u64>,
    critical_path_unknown_ms: Option<u64>,
}

impl From<TurnCriticalPath> for CriticalPathEventParams {
    fn from(path: TurnCriticalPath) -> Self {
        let (status, durations) = match path {
            TurnCriticalPath::Disabled => ("disabled", None),
            TurnCriticalPath::Incomplete => ("incomplete", None),
            TurnCriticalPath::Complete(durations) => ("complete", Some(durations)),
        };
        Self {
            critical_path_status: status,
            critical_path_duration_ms: durations.as_ref().map(|value| value.duration_ms),
            critical_path_inference_ms: durations.as_ref().map(|value| value.inference_ms),
            critical_path_auto_review_ms: durations.as_ref().map(|value| value.auto_review_ms),
            critical_path_user_wait_ms: durations.as_ref().map(|value| value.user_wait_ms),
            critical_path_tools_ms: durations.as_ref().map(|value| value.tools_ms),
            critical_path_overhead_ms: durations.as_ref().map(|value| value.overhead_ms),
            critical_path_unknown_ms: durations.as_ref().map(|value| value.unknown_ms),
        }
    }
}
