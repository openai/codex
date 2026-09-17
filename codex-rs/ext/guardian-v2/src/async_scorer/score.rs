//! Owns cached-score publication, observation coverage, and reuse snapshots.
//! The cached score, authorization, and coverage are published under one lock;
//! failures never replace a newer sample.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::SystemTime;

use codex_extension_api::ExtensionMetrics;
use codex_extension_api::ToolStartInput;
use codex_protocol::security_risk::SecurityRiskScore;

use super::authorization::ScoreAuthorization;
use super::wrapper_lag::WrapperLag;

#[derive(Default)]
pub(super) struct GuardianV2ScoreProgress {
    state: Mutex<ScoreState>,
    pub(super) metrics: Option<Arc<dyn ExtensionMetrics>>,
}

#[derive(Default)]
struct ScoreState {
    score: Option<SecurityRiskScore>,
    wrapper_lag: WrapperLag,
    latest_tool_call: usize,
    js_executions: usize,
    latest_scored_tool_call: usize,
    latest_failed_tool_call: usize,
    oversized_tool_calls: BTreeSet<String>,
    authorization: Option<ScoreAuthorization>,
}

/// A consistent view of the published score and the observations it covers.
#[derive(Debug, PartialEq)]
pub(super) struct CachedScore {
    pub(super) lag: usize,
    pub(super) has_unscored_failure: bool,
    pub(super) js_executions: usize,
    pub(super) oversized: bool,
    pub(super) action_risk: Option<f64>,
    pub(super) authorization: Option<ScoreAuthorization>,
}

impl GuardianV2ScoreProgress {
    pub(super) fn new(metrics: Option<Arc<dyn ExtensionMetrics>>) -> Self {
        Self {
            metrics,
            ..Default::default()
        }
    }

    pub(super) fn observe(&self, input: &ToolStartInput<'_>) -> usize {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.latest_tool_call = state.latest_tool_call.saturating_add(/*rhs*/ 1);
        let index = state.latest_tool_call;
        state.wrapper_lag.record(input, index);
        index
    }

    pub(super) fn observe_js_execution(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Setup/reset calls must not consume the first JS execution allowance.
        state.js_executions = state.js_executions.saturating_add(/*rhs*/ 1);
    }

    pub(super) fn invalidate(&self, index: usize) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.latest_failed_tool_call = state.latest_failed_tool_call.max(index);
    }

    pub(super) fn mark_oversized(&self, call_id: &str, index: usize) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.oversized_tool_calls.insert(call_id.to_owned());
        state.latest_failed_tool_call = state.latest_failed_tool_call.max(index);
    }

    pub(super) fn finish(&self, call_id: &str) {
        // Overflow belongs to each active call even after a newer score succeeds.
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .oversized_tool_calls
            .remove(call_id);
    }

    pub(super) fn publish(
        &self,
        score: SecurityRiskScore,
        authorization: ScoreAuthorization,
        index: usize,
    ) -> bool {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let accepted = state
            .score
            .as_ref()
            .is_none_or(|previous| previous.sampled_at < score.sampled_at);
        if accepted {
            state.score = Some(score);
            state.authorization = Some(authorization);
            state.latest_scored_tool_call = state.latest_scored_tool_call.max(index);
        }
        accepted
    }

    pub(super) fn fail_closed(&self, sampled_at: SystemTime) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let score = SecurityRiskScore {
            scores: BTreeMap::from([("action_risk".to_owned(), 1.0)]),
            call_id: None,
            action: None,
            sampled_at: Some(sampled_at.into()),
        };
        // Failures win timestamp ties; successful samples must be strictly newer.
        if state
            .score
            .as_ref()
            .is_none_or(|previous| previous.sampled_at <= score.sampled_at)
        {
            state.score = Some(score);
        }
    }

    pub(super) fn clear_score(&self) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .score = None;
    }

    pub(super) fn inspect(&self, call_id: Option<&str>) -> CachedScore {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        CachedScore {
            lag: state
                .latest_tool_call
                .saturating_sub(state.latest_scored_tool_call)
                .saturating_sub(
                    state
                        .wrapper_lag
                        .discount(call_id, state.latest_scored_tool_call),
                ),
            has_unscored_failure: state.latest_failed_tool_call > state.latest_scored_tool_call,
            js_executions: state.js_executions,
            oversized: call_id.is_some_and(|id| state.oversized_tool_calls.contains(id)),
            action_risk: state
                .score
                .as_ref()
                .and_then(|score| score.scores.get("action_risk").copied()),
            authorization: state.authorization.clone(),
        }
    }
}

#[cfg(test)]
#[path = "score_tests.rs"]
pub(super) mod tests;
