//! Tracks observation coverage and the authorization attached to the latest score.
//! Failed samples may replace an equally old score, but never a newer one.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::time::SystemTime;

use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionMetrics;
use codex_protocol::security_risk::SecurityRiskScore;

use super::authorization::ScoreAuthorization;
use super::wrapper_lag::WrapperLag;

#[derive(Default)]
pub(super) struct GuardianV2ScoreProgress {
    pub(super) wrapper_lag: WrapperLag,
    pub(super) latest_tool_call: AtomicUsize,
    // Setup and reset calls must not consume the first JS execution allowance.
    pub(super) js_executions: AtomicUsize,
    pub(super) latest_scored_tool_call: AtomicUsize,
    pub(super) latest_failed_tool_call: AtomicUsize,
    // Keep overflow attached to each active call even after a newer score succeeds.
    // The host's finish callback removes entries on completion, failure, or cancellation.
    pub(super) oversized_tool_calls: Mutex<BTreeSet<String>>,
    // Serialize successful score publication with its authorization metadata.
    pub(super) authorization: Mutex<Option<ScoreAuthorization>>,
    pub(super) metrics: Option<Arc<dyn ExtensionMetrics>>,
}

pub(super) fn record_fail_closed_score(thread_store: &ExtensionData, sampled_at: SystemTime) {
    let score = SecurityRiskScore {
        scores: BTreeMap::from([("action_risk".to_owned(), 1.0)]),
        call_id: None,
        action: None,
        sampled_at: Some(sampled_at.into()),
    };
    thread_store.insert_if(score.clone(), |previous| {
        previous.is_none_or(|previous| previous.sampled_at <= score.sampled_at)
    });
}

#[cfg(test)]
#[path = "score_tests.rs"]
mod tests;
