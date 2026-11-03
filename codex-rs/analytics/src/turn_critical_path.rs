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

impl TurnCriticalPath {
    pub fn status(&self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Incomplete => "incomplete",
            Self::Complete(_) => "complete",
        }
    }

    pub fn durations(&self) -> Option<&TurnCriticalPathDurations> {
        match self {
            Self::Complete(durations) => Some(durations),
            Self::Disabled | Self::Incomplete => None,
        }
    }
}

/// Category totals over the server turn interval, truncated to milliseconds.
/// Inference is the compatibility total of sampling and compaction, not another leaf.
/// Independent truncation can leave the seven leaf categories up to six milliseconds
/// below duration_ms, and sampling_ms + compaction_ms up to one below inference_ms.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TurnCriticalPathDurations {
    pub duration_ms: u64,
    pub inference_ms: u64,
    pub sampling_ms: u64,
    pub compaction_ms: u64,
    pub auto_review_ms: u64,
    /// Portion of auto_review_ms outside observed Guardian turn execution; not additive.
    pub auto_review_overhead_ms: Option<u64>,
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
    critical_path_sampling_ms: Option<u64>,
    critical_path_compaction_ms: Option<u64>,
    critical_path_auto_review_ms: Option<u64>,
    critical_path_auto_review_overhead_ms: Option<u64>,
    critical_path_user_wait_ms: Option<u64>,
    critical_path_tools_ms: Option<u64>,
    critical_path_overhead_ms: Option<u64>,
    critical_path_unknown_ms: Option<u64>,
}

impl From<TurnCriticalPath> for CriticalPathEventParams {
    fn from(path: TurnCriticalPath) -> Self {
        let durations = path.durations();
        Self {
            critical_path_status: path.status(),
            critical_path_duration_ms: durations.map(|value| value.duration_ms),
            critical_path_inference_ms: durations.map(|value| value.inference_ms),
            critical_path_sampling_ms: durations.map(|value| value.sampling_ms),
            critical_path_compaction_ms: durations.map(|value| value.compaction_ms),
            critical_path_auto_review_ms: durations.map(|value| value.auto_review_ms),
            critical_path_auto_review_overhead_ms: durations
                .and_then(|value| value.auto_review_overhead_ms),
            critical_path_user_wait_ms: durations.map(|value| value.user_wait_ms),
            critical_path_tools_ms: durations.map(|value| value.tools_ms),
            critical_path_overhead_ms: durations.map(|value| value.overhead_ms),
            critical_path_unknown_ms: durations.map(|value| value.unknown_ms),
        }
    }
}
