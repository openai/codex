use std::sync::Weak;

use codex_core::ThreadManager;
use codex_otel::GOAL_BLOCKED_METRIC;
use codex_otel::GOAL_BUDGET_LIMITED_METRIC;
use codex_otel::GOAL_COMPLETED_METRIC;
use codex_otel::GOAL_CREATED_METRIC;
use codex_otel::GOAL_DURATION_SECONDS_METRIC;
use codex_otel::GOAL_RESUMED_METRIC;
use codex_otel::GOAL_TOKEN_COUNT_METRIC;
use codex_otel::GOAL_USAGE_LIMITED_METRIC;
use codex_otel::MetricsClient;
use codex_protocol::ThreadId;
use codex_protocol::openai_models::ReasoningEffort;
use serde::Deserialize;

#[derive(Deserialize)]
struct ToolMetadata {
    reasoning_effort: Option<ReasoningEffort>,
}

#[derive(Clone, Default)]
pub(crate) struct GoalMetrics {
    metrics_client: Option<MetricsClient>,
}

impl GoalMetrics {
    pub(crate) fn new(metrics_client: Option<MetricsClient>) -> Self {
        Self { metrics_client }
    }

    pub(crate) fn record_tool_created(&self, metadata: Option<&str>) {
        let Some(client) = &self.metrics_client else {
            return;
        };
        // The invoking step's settings can differ from the thread's current selection.
        let effort = metadata
            .and_then(|metadata| serde_json::from_str::<ToolMetadata>(metadata).ok())
            .and_then(|metadata| metadata.reasoning_effort);
        Self::record_created(client, effort);
    }

    pub(crate) async fn record_external_created(
        &self,
        manager: &Weak<ThreadManager>,
        thread_id: ThreadId,
    ) {
        let Some(client) = &self.metrics_client else {
            return;
        };
        // A goal can be the first action on a thread, before any turn metadata exists.
        let effort = if let Some(manager) = manager.upgrade()
            && let Ok(thread) = manager.get_thread(thread_id).await
        {
            let settings = thread.thread_settings_snapshot().await;
            match settings.reasoning_effort {
                Some(effort) => Some(effort),
                None => thread
                    .model_info(&settings.model, &Default::default())
                    .await
                    .default_reasoning_level
                    .clone(),
            }
        } else {
            None
        };
        Self::record_created(client, effort);
    }

    fn record_created(client: &MetricsClient, effort: Option<ReasoningEffort>) {
        let label = match &effort {
            Some(ReasoningEffort::Custom(_)) => "custom",
            Some(effort) => effort.as_str(),
            None => "unknown",
        };
        let _ = client.counter(
            GOAL_CREATED_METRIC,
            /*inc*/ 1,
            &[("reasoning_effort", label)],
        );
    }

    pub(crate) fn record_resumed(&self) {
        let Some(metrics_client) = self.metrics_client.as_ref() else {
            return;
        };
        let _ = metrics_client.counter(GOAL_RESUMED_METRIC, /*inc*/ 1, &[]);
    }

    pub(crate) fn record_resumed_if_status_changed(
        &self,
        previous_status: Option<codex_state::ThreadGoalStatus>,
        goal_status: codex_state::ThreadGoalStatus,
    ) {
        if goal_status == codex_state::ThreadGoalStatus::Active
            && matches!(
                previous_status,
                Some(
                    codex_state::ThreadGoalStatus::Paused
                        | codex_state::ThreadGoalStatus::Blocked
                        | codex_state::ThreadGoalStatus::UsageLimited
                )
            )
        {
            self.record_resumed();
        }
    }

    pub(crate) fn record_terminal_if_status_changed(
        &self,
        previous_status: Option<codex_state::ThreadGoalStatus>,
        goal: &codex_state::ThreadGoal,
    ) {
        if previous_status == Some(goal.status) {
            return;
        }

        let counter = match goal.status {
            codex_state::ThreadGoalStatus::Blocked => GOAL_BLOCKED_METRIC,
            codex_state::ThreadGoalStatus::UsageLimited => GOAL_USAGE_LIMITED_METRIC,
            codex_state::ThreadGoalStatus::BudgetLimited => GOAL_BUDGET_LIMITED_METRIC,
            codex_state::ThreadGoalStatus::Complete => GOAL_COMPLETED_METRIC,
            codex_state::ThreadGoalStatus::Active | codex_state::ThreadGoalStatus::Paused => {
                return;
            }
        };
        let Some(metrics_client) = self.metrics_client.as_ref() else {
            return;
        };
        let status_tag = [("status", goal.status.as_str())];
        let _ = metrics_client.counter(counter, /*inc*/ 1, &[]);
        let _ = metrics_client.histogram(GOAL_TOKEN_COUNT_METRIC, goal.tokens_used, &status_tag);
        let _ = metrics_client.histogram(
            GOAL_DURATION_SECONDS_METRIC,
            goal.time_used_seconds,
            &status_tag,
        );
    }
}
