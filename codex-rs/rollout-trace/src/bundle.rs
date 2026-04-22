//! Trace bundle manifest and local layout constants.

use serde::Deserialize;
use serde::Serialize;

use crate::model::AgentThreadId;

pub(crate) const MANIFEST_FILE_NAME: &str = "manifest.json";
pub(crate) const RAW_EVENT_LOG_FILE_NAME: &str = "trace.jsonl";
pub(crate) const PAYLOADS_DIR_NAME: &str = "payloads";
/// Conventional file name for a reducer-written `RolloutTrace` cache.
pub const REDUCED_STATE_FILE_NAME: &str = "state.json";
pub(crate) const TRACE_MANIFEST_SCHEMA_VERSION: u32 = 1;
pub(crate) const REDUCED_TRACE_SCHEMA_VERSION: u32 = 1;

/// Manifest stored at the root of a trace bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TraceBundleManifest {
    pub(crate) schema_version: u32,
    pub(crate) trace_id: String,
    pub(crate) rollout_id: String,
    /// Root thread for the recorded rollout. Replay should fail rather than
    /// inventing a placeholder, because every reduced object is scoped back to
    /// this thread tree.
    pub(crate) root_thread_id: AgentThreadId,
    pub(crate) started_at_unix_ms: i64,
    pub(crate) raw_event_log: String,
    pub(crate) payloads_dir: String,
}

impl TraceBundleManifest {
    /// Builds a manifest that uses the standard local bundle layout.
    pub(crate) fn new(
        trace_id: String,
        rollout_id: String,
        root_thread_id: AgentThreadId,
        started_at_unix_ms: i64,
    ) -> Self {
        Self {
            schema_version: TRACE_MANIFEST_SCHEMA_VERSION,
            trace_id,
            rollout_id,
            root_thread_id,
            started_at_unix_ms,
            raw_event_log: RAW_EVENT_LOG_FILE_NAME.to_string(),
            payloads_dir: PAYLOADS_DIR_NAME.to_string(),
        }
    }
}
