//! Optional process observations, independent of sessions. Process-owned
//! providers observe existing children; remote transports own connection policy.

/// A cumulative observation of one host process, shared across its sessions.
#[derive(Clone, Debug, PartialEq)]
pub struct CodeModeProcessMetrics {
    pub process_id: u32,
    /// Unix timestamp, stable for this process across all metrics transports.
    pub process_start_time_seconds: f64,
    pub cpu_seconds_total: f64,
    pub resident_memory_bytes: u64,
}

/// Unsupported capability and an absent process are distinct from a failed sample.
#[derive(Clone, Debug, PartialEq)]
pub enum CodeModeProcessMetricsSample {
    Unsupported,
    /// The process owner knows no host is running; never inferred from an RPC status.
    NotRunning,
    Snapshot(CodeModeProcessMetrics),
}
