//! Owns cached-score publication, observation coverage, and reuse snapshots.
//! Scores, authorization, and coverage stay policy-specific. Action freshness, wrapper
//! tracking, and JS allowances span generations; late results cannot refresh another policy.
//! Pending classifications expire only when no active action can reuse their score.

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
    pub(super) actions: Arc<Mutex<ActionState>>,
    initial_failure_index: usize,
    pub(super) updates: tokio::sync::watch::Sender<()>,
    pub(super) metrics: Option<Arc<dyn ExtensionMetrics>>,
}

#[derive(Clone)]
pub(super) struct PendingScore {
    pub(super) progress: Arc<GuardianV2ScoreProgress>,
    pub(super) index: usize,
    pub(super) max_lag: usize,
}

impl PendingScore {
    pub(super) fn is_expired(&self) -> bool {
        let state = self
            .progress
            .actions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state
            .latest_tool_call
            .saturating_sub(self.index)
            .saturating_sub(state.wrapper_lag.pending_discount(self.index))
            > self.max_lag
    }

    pub(super) async fn expired(&self) {
        // Subscribe before checking so admission cannot race past the expiry check.
        let mut observations = self
            .progress
            .actions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .updates
            .subscribe();
        while !self.is_expired() {
            if observations.changed().await.is_err() {
                return;
            }
        }
    }
}

#[derive(Default)]
struct ScoreState {
    score: Option<SecurityRiskScore>,
    score_index: usize,
    latest_scored_tool_call: usize,
    authorization: Option<ScoreAuthorization>,
}

/// Observations of the shared thread, independent of the policy scoring them.
#[derive(Default)]
pub(super) struct ActionState {
    wrapper_lag: WrapperLag,
    latest_tool_call: usize,
    latest_failed_tool_call: usize,
    oversized_tool_calls: BTreeSet<String>,
    js_executions: usize,
    updates: tokio::sync::watch::Sender<()>,
}

/// A consistent view of the published score and the observations it covers.
#[derive(Debug, PartialEq)]
pub(super) struct CachedScore {
    pub(super) lag: usize,
    pub(super) sampled_at: Option<SystemTime>,
    pub(super) score_at_or_before_action: bool,
    pub(super) has_unscored_failure: bool,
    pub(super) js_executions: usize,
    pub(super) oversized: bool,
    pub(super) action_risk: Option<f64>,
    pub(super) authorization: Option<ScoreAuthorization>,
}

impl GuardianV2ScoreProgress {
    pub(super) fn new(
        metrics: Option<Arc<dyn ExtensionMetrics>>,
        actions: Arc<Mutex<ActionState>>,
    ) -> Self {
        let initial_failure_index = actions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .latest_failed_tool_call;
        Self {
            metrics,
            actions,
            initial_failure_index,
            ..Default::default()
        }
    }

    pub(super) fn observe(&self, input: &ToolStartInput<'_>) -> usize {
        let mut state = self
            .actions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.latest_tool_call = state.latest_tool_call.saturating_add(/*rhs*/ 1);
        let index = state.latest_tool_call;
        state.wrapper_lag.record(input, index);
        state.updates.send_replace(());
        index
    }

    pub(super) fn observe_js_execution(&self) {
        // Setup/reset calls must not consume the first JS execution allowance.
        let mut state = self
            .actions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.js_executions = state.js_executions.saturating_add(/*rhs*/ 1);
    }

    pub(super) fn invalidate(&self, index: usize) {
        let mut state = self
            .actions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.latest_failed_tool_call = state.latest_failed_tool_call.max(index);
    }

    pub(super) fn mark_oversized(&self, call_id: &str, index: usize) {
        let mut state = self
            .actions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.oversized_tool_calls.insert(call_id.to_owned());
        state.latest_failed_tool_call = state.latest_failed_tool_call.max(index);
    }

    pub(super) fn finish(&self, call_id: &str) {
        // Overflow belongs to each active call even after a newer score succeeds.
        let mut state = self
            .actions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.oversized_tool_calls.remove(call_id);
        state.wrapper_lag.finish(call_id);
        state.updates.send_replace(());
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
            state.score_index = index;
            state.authorization = Some(authorization);
            state.latest_scored_tool_call = state.latest_scored_tool_call.max(index);
            self.updates.send_replace(());
        }
        accepted
    }

    /// Invalidates this action across policies before publishing a local fail-closed score.
    pub(super) fn fail_closed(&self, sampled_at: SystemTime, index: usize) {
        self.invalidate(index);
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
        self.updates.send_replace(());
    }

    pub(super) fn clear_score(&self) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .score = None;
    }

    pub(super) fn inspect(&self, call_id: Option<&str>) -> CachedScore {
        let actions = self
            .actions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        CachedScore {
            sampled_at: state
                .score
                .as_ref()
                .and_then(|score| score.sampled_at)
                .map(SystemTime::from),
            // Use the accepted score's source, not the coverage high-water mark.
            score_at_or_before_action: actions
                .wrapper_lag
                .get(call_id)
                .is_some_and(|start| state.score_index <= start.index),
            lag: actions
                .latest_tool_call
                .saturating_sub(state.latest_scored_tool_call)
                .saturating_sub(
                    actions
                        .wrapper_lag
                        .discount(call_id, state.latest_scored_tool_call),
                ),
            // Only failures newly recorded since cache creation can invalidate it.
            // A published score covers those failures through its own action index.
            has_unscored_failure: actions.latest_failed_tool_call
                > self.initial_failure_index.max(state.score_index),
            js_executions: actions.js_executions,
            oversized: call_id.is_some_and(|id| actions.oversized_tool_calls.contains(id)),
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
