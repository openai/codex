use super::*;
use uuid::Uuid;

pub struct ThreadGoalUpdate {
    pub status: Option<crate::ThreadGoalStatus>,
    pub token_budget: Option<Option<i64>>,
}

pub enum ThreadGoalAccountingOutcome {
    Unchanged(Option<crate::ThreadGoal>),
    Updated(crate::ThreadGoal),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThreadGoalAccountingMode {
    ActiveStatusOnly,
    ActiveOnly,
    ActiveOrComplete,
    ActiveOrStopped,
}

impl StateRuntime {
    pub async fn get_thread_goal(
        &self,
        thread_id: ThreadId,
    ) -> anyhow::Result<Option<crate::ThreadGoal>> {
        let row = sqlx::query(
            r#"
SELECT
    thread_id,
    goal_id,
    objective,
    status,
    token_budget,
    tokens_used,
    time_used_seconds,
    created_at_ms,
    updated_at_ms
FROM thread_goals
WHERE thread_id = ?
            "#,
        )
        .bind(thread_id.to_string())
        .fetch_optional(self.pool.as_ref())
        .await?;

        row.map(|row| thread_goal_from_row(&row)).transpose()
    }

    pub async fn replace_thread_goal(
        &self,
        thread_id: ThreadId,
        objective: &str,
        status: crate::ThreadGoalStatus,
        token_budget: Option<i64>,
    ) -> anyhow::Result<crate::ThreadGoal> {
        let goal_id = Uuid::new_v4().to_string();
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let status = status_after_budget_limit(status, /*tokens_used*/ 0, token_budget);
        let row = sqlx::query(
            r#"
INSERT INTO thread_goals (
    thread_id,
    goal_id,
    objective,
    status,
    token_budget,
    tokens_used,
    time_used_seconds,
    created_at_ms,
    updated_at_ms
) VALUES (?, ?, ?, ?, ?, 0, 0, ?, ?)
ON CONFLICT(thread_id) DO UPDATE SET
    goal_id = excluded.goal_id,
    objective = excluded.objective,
    status = excluded.status,
    token_budget = excluded.token_budget,
    tokens_used = 0,
    time_used_seconds = 0,
    created_at_ms = excluded.created_at_ms,
    updated_at_ms = excluded.updated_at_ms
RETURNING
    thread_id,
    goal_id,
    objective,
    status,
    token_budget,
    tokens_used,
    time_used_seconds,
    created_at_ms,
    updated_at_ms
            "#,
        )
        .bind(thread_id.to_string())
        .bind(goal_id)
        .bind(objective)
        .bind(status.as_str())
        .bind(token_budget)
        .bind(now_ms)
        .bind(now_ms)
        .fetch_one(self.pool.as_ref())
        .await?;

        thread_goal_from_row(&row)
    }

    pub async fn update_thread_goal(
        &self,
        thread_id: ThreadId,
        update: ThreadGoalUpdate,
    ) -> anyhow::Result<Option<crate::ThreadGoal>> {
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let result = match (update.status, update.token_budget) {
            (Some(status), Some(token_budget)) => {
                sqlx::query(
                    r#"
UPDATE thread_goals
SET
    status = CASE
        WHEN status = ? AND ? = ? THEN status
        WHEN ? = 'active' AND ? IS NOT NULL AND tokens_used >= ? THEN ?
        ELSE ?
    END,
    token_budget = ?,
    updated_at_ms = ?
WHERE thread_id = ?
            "#,
                )
                .bind(crate::ThreadGoalStatus::BudgetLimited.as_str())
                .bind(status.as_str())
                .bind(crate::ThreadGoalStatus::Paused.as_str())
                .bind(status.as_str())
                .bind(token_budget)
                .bind(token_budget)
                .bind(crate::ThreadGoalStatus::BudgetLimited.as_str())
                .bind(status.as_str())
                .bind(token_budget)
                .bind(now_ms)
                .bind(thread_id.to_string())
                .execute(self.pool.as_ref())
                .await?
            }
            (Some(status), None) => {
                sqlx::query(
                    r#"
UPDATE thread_goals
SET
    status = CASE
        WHEN status = ? AND ? = ? THEN status
        WHEN ? = 'active' AND token_budget IS NOT NULL AND tokens_used >= token_budget THEN ?
        ELSE ?
    END,
    updated_at_ms = ?
WHERE thread_id = ?
            "#,
                )
                .bind(crate::ThreadGoalStatus::BudgetLimited.as_str())
                .bind(status.as_str())
                .bind(crate::ThreadGoalStatus::Paused.as_str())
                .bind(status.as_str())
                .bind(crate::ThreadGoalStatus::BudgetLimited.as_str())
                .bind(status.as_str())
                .bind(now_ms)
                .bind(thread_id.to_string())
                .execute(self.pool.as_ref())
                .await?
            }
            (None, Some(token_budget)) => {
                sqlx::query(
                    r#"
UPDATE thread_goals
SET
    token_budget = ?,
    status = CASE
        WHEN status = 'active' AND ? IS NOT NULL AND tokens_used >= ? THEN ?
        ELSE status
    END,
    updated_at_ms = ?
WHERE thread_id = ?
            "#,
                )
                .bind(token_budget)
                .bind(token_budget)
                .bind(token_budget)
                .bind(crate::ThreadGoalStatus::BudgetLimited.as_str())
                .bind(now_ms)
                .bind(thread_id.to_string())
                .execute(self.pool.as_ref())
                .await?
            }
            (None, None) => return self.get_thread_goal(thread_id).await,
        };

        if result.rows_affected() == 0 {
            return Ok(None);
        }

        self.get_thread_goal(thread_id).await
    }

    pub async fn pause_active_thread_goal(
        &self,
        thread_id: ThreadId,
    ) -> anyhow::Result<Option<crate::ThreadGoal>> {
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let result = sqlx::query(
            r#"
UPDATE thread_goals
SET
    status = ?,
    updated_at_ms = ?
WHERE thread_id = ?
  AND status = 'active'
            "#,
        )
        .bind(crate::ThreadGoalStatus::Paused.as_str())
        .bind(now_ms)
        .bind(thread_id.to_string())
        .execute(self.pool.as_ref())
        .await?;

        if result.rows_affected() == 0 {
            return Ok(None);
        }

        self.get_thread_goal(thread_id).await
    }

    pub async fn delete_thread_goal(&self, thread_id: ThreadId) -> anyhow::Result<bool> {
        let result = sqlx::query(
            r#"
DELETE FROM thread_goals
WHERE thread_id = ?
            "#,
        )
        .bind(thread_id.to_string())
        .execute(self.pool.as_ref())
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn account_thread_goal_usage(
        &self,
        thread_id: ThreadId,
        time_delta_seconds: i64,
        token_delta: i64,
        mode: ThreadGoalAccountingMode,
        expected_goal_id: Option<&str>,
    ) -> anyhow::Result<ThreadGoalAccountingOutcome> {
        let time_delta_seconds = time_delta_seconds.max(0);
        let token_delta = token_delta.max(0);
        if time_delta_seconds == 0 && token_delta == 0 {
            return Ok(ThreadGoalAccountingOutcome::Unchanged(
                self.get_thread_goal(thread_id).await?,
            ));
        }

        let now_ms = datetime_to_epoch_millis(Utc::now());
        let status_filter = match mode {
            ThreadGoalAccountingMode::ActiveStatusOnly => "status = 'active'",
            ThreadGoalAccountingMode::ActiveOnly => "status IN ('active', 'budget_limited')",
            ThreadGoalAccountingMode::ActiveOrComplete => {
                "status IN ('active', 'budget_limited', 'complete')"
            }
            ThreadGoalAccountingMode::ActiveOrStopped => {
                "status IN ('active', 'paused', 'budget_limited')"
            }
        };
        let budget_limit_status_filter = match mode {
            ThreadGoalAccountingMode::ActiveStatusOnly
            | ThreadGoalAccountingMode::ActiveOnly
            | ThreadGoalAccountingMode::ActiveOrComplete => "status = 'active'",
            ThreadGoalAccountingMode::ActiveOrStopped => {
                "status IN ('active', 'paused', 'budget_limited')"
            }
        };
        let goal_id_filter = if expected_goal_id.is_some() {
            "goal_id = ?"
        } else {
            "1 = 1"
        };
        let query = format!(
            r#"
UPDATE thread_goals
SET
    time_used_seconds = time_used_seconds + ?,
    tokens_used = tokens_used + ?,
    status = CASE
        WHEN {budget_limit_status_filter} AND token_budget IS NOT NULL AND tokens_used + ? >= token_budget
            THEN ?
        ELSE status
    END,
    updated_at_ms = ?
WHERE thread_id = ?
  AND {status_filter}
  AND {goal_id_filter}
RETURNING
    thread_id,
    goal_id,
    objective,
    status,
    token_budget,
    tokens_used,
    time_used_seconds,
    created_at_ms,
    updated_at_ms
            "#,
        );

        let mut query = sqlx::query(&query)
            .bind(time_delta_seconds)
            .bind(token_delta)
            .bind(token_delta)
            .bind(crate::ThreadGoalStatus::BudgetLimited.as_str())
            .bind(now_ms)
            .bind(thread_id.to_string());
        if let Some(expected_goal_id) = expected_goal_id {
            query = query.bind(expected_goal_id);
        }

        let row = query.fetch_optional(self.pool.as_ref()).await?;

        let Some(row) = row else {
            return Ok(ThreadGoalAccountingOutcome::Unchanged(
                self.get_thread_goal(thread_id).await?,
            ));
        };

        let updated = thread_goal_from_row(&row)?;
        Ok(ThreadGoalAccountingOutcome::Updated(updated))
    }
}

fn thread_goal_from_row(row: &sqlx::sqlite::SqliteRow) -> anyhow::Result<crate::ThreadGoal> {
    ThreadGoalRow::try_from_row(row).and_then(crate::ThreadGoal::try_from)
}

fn status_after_budget_limit(
    status: crate::ThreadGoalStatus,
    tokens_used: i64,
    token_budget: Option<i64>,
) -> crate::ThreadGoalStatus {
    if status == crate::ThreadGoalStatus::Active
        && token_budget.is_some_and(|budget| tokens_used >= budget)
    {
        crate::ThreadGoalStatus::BudgetLimited
    } else {
        status
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::test_support::test_thread_metadata;
    use crate::runtime::test_support::unique_temp_dir;
    use pretty_assertions::assert_eq;

    async fn test_runtime() -> std::sync::Arc<StateRuntime> {
        StateRuntime::init(unique_temp_dir(), "test-provider".to_string())
            .await
            .expect("state db should initialize")
    }

    fn test_thread_id() -> ThreadId {
        ThreadId::from_string("00000000-0000-0000-0000-000000000123").expect("valid thread id")
    }

    async fn upsert_test_thread(runtime: &StateRuntime, thread_id: ThreadId) {
        let metadata = test_thread_metadata(
            runtime.codex_home(),
            thread_id,
            runtime.codex_home().join("workspace"),
        );
        runtime
            .upsert_thread(&metadata)
            .await
            .expect("test thread should be upserted");
    }

    #[tokio::test]
    async fn replace_update_and_get_thread_goal() {
        let runtime = test_runtime().await;
        let thread_id = test_thread_id();
        upsert_test_thread(&runtime, thread_id).await;

        let goal = runtime
            .replace_thread_goal(
                thread_id,
                "optimize the benchmark",
                crate::ThreadGoalStatus::Active,
                /*token_budget*/ Some(100_000),
            )
            .await
            .expect("goal replacement should succeed");
        assert_eq!(
            Some(goal.clone()),
            runtime.get_thread_goal(thread_id).await.unwrap()
        );

        let updated = runtime
            .update_thread_goal(
                thread_id,
                ThreadGoalUpdate {
                    status: Some(crate::ThreadGoalStatus::Paused),
                    token_budget: Some(Some(200_000)),
                },
            )
            .await
            .expect("goal update should succeed")
            .expect("goal should exist");
        let expected = crate::ThreadGoal {
            status: crate::ThreadGoalStatus::Paused,
            token_budget: Some(200_000),
            updated_at: updated.updated_at,
            ..goal.clone()
        };
        assert_eq!(expected, updated);

        let replaced = runtime
            .replace_thread_goal(
                thread_id,
                "ship the new result",
                crate::ThreadGoalStatus::Active,
                /*token_budget*/ None,
            )
            .await
            .expect("goal replacement should succeed");
        assert_eq!("ship the new result", replaced.objective);
        assert_eq!(crate::ThreadGoalStatus::Active, replaced.status);
        assert_eq!(None, replaced.token_budget);
        assert_eq!(0, replaced.tokens_used);
        assert_eq!(0, replaced.time_used_seconds);

        assert!(runtime.delete_thread_goal(thread_id).await.unwrap());
        assert_eq!(None, runtime.get_thread_goal(thread_id).await.unwrap());
        assert!(!runtime.delete_thread_goal(thread_id).await.unwrap());
    }

    #[tokio::test]
    async fn replace_thread_goal_applies_budget_limit_immediately() {
        let runtime = test_runtime().await;
        let thread_id = test_thread_id();
        upsert_test_thread(&runtime, thread_id).await;

        let replaced = runtime
            .replace_thread_goal(
                thread_id,
                "stay within budget",
                crate::ThreadGoalStatus::Active,
                /*token_budget*/ Some(0),
            )
            .await
            .expect("goal replacement should succeed");

        assert_eq!(crate::ThreadGoalStatus::BudgetLimited, replaced.status);
        assert_eq!(Some(0), replaced.token_budget);
        assert_eq!(0, replaced.tokens_used);
        assert_eq!(0, replaced.time_used_seconds);
    }

    #[tokio::test]
    async fn usage_accounting_ignores_replaced_goal_version() {
        let runtime = test_runtime().await;
        let thread_id = test_thread_id();
        upsert_test_thread(&runtime, thread_id).await;

        let original = runtime
            .replace_thread_goal(
                thread_id,
                "old objective",
                crate::ThreadGoalStatus::Active,
                /*token_budget*/ Some(100),
            )
            .await
            .expect("goal replacement should succeed");
        let replacement = runtime
            .replace_thread_goal(
                thread_id,
                "new objective",
                crate::ThreadGoalStatus::Active,
                /*token_budget*/ Some(10),
            )
            .await
            .expect("goal replacement should succeed");

        let outcome = runtime
            .account_thread_goal_usage(
                thread_id,
                /*time_delta_seconds*/ 5,
                /*token_delta*/ 5,
                ThreadGoalAccountingMode::ActiveOnly,
                Some(original.goal_id.as_str()),
            )
            .await
            .expect("usage accounting should succeed");

        let ThreadGoalAccountingOutcome::Unchanged(Some(goal)) = outcome else {
            panic!("stale goal version should not be updated");
        };
        assert_ne!(replacement.goal_id, original.goal_id);
        assert_eq!(replacement.created_at, goal.created_at);
        assert_eq!("new objective", goal.objective);
        assert_eq!(0, goal.tokens_used);
        assert_eq!(0, goal.time_used_seconds);
    }

    #[tokio::test]
    async fn concurrent_partial_updates_preserve_independent_fields() {
        let runtime = test_runtime().await;
        let thread_id = test_thread_id();
        upsert_test_thread(&runtime, thread_id).await;
        runtime
            .replace_thread_goal(
                thread_id,
                "optimize the benchmark",
                crate::ThreadGoalStatus::Active,
                /*token_budget*/ Some(100_000),
            )
            .await
            .expect("goal replacement should succeed");

        let status_update = runtime.update_thread_goal(
            thread_id,
            ThreadGoalUpdate {
                status: Some(crate::ThreadGoalStatus::Paused),
                token_budget: None,
            },
        );
        let budget_update = runtime.update_thread_goal(
            thread_id,
            ThreadGoalUpdate {
                status: None,
                token_budget: Some(Some(200_000)),
            },
        );
        let (status_update, budget_update) = tokio::join!(status_update, budget_update);
        status_update.expect("status update should succeed");
        budget_update.expect("budget update should succeed");

        let goal = runtime
            .get_thread_goal(thread_id)
            .await
            .expect("goal read should succeed")
            .expect("goal should exist");
        assert_eq!(crate::ThreadGoalStatus::Paused, goal.status);
        assert_eq!(Some(200_000), goal.token_budget);
    }

    #[tokio::test]
    async fn pause_active_thread_goal_does_not_clobber_terminal_status() {
        let runtime = test_runtime().await;
        let thread_id = test_thread_id();
        upsert_test_thread(&runtime, thread_id).await;
        let goal = runtime
            .replace_thread_goal(
                thread_id,
                "optimize the benchmark",
                crate::ThreadGoalStatus::Active,
                /*token_budget*/ Some(100_000),
            )
            .await
            .expect("goal replacement should succeed");

        let paused = runtime
            .pause_active_thread_goal(thread_id)
            .await
            .expect("active pause should succeed")
            .expect("active goal should be paused");
        let expected = crate::ThreadGoal {
            status: crate::ThreadGoalStatus::Paused,
            updated_at: paused.updated_at,
            ..goal
        };
        assert_eq!(expected, paused);

        let complete = runtime
            .update_thread_goal(
                thread_id,
                ThreadGoalUpdate {
                    status: Some(crate::ThreadGoalStatus::Complete),
                    token_budget: None,
                },
            )
            .await
            .expect("goal update should succeed")
            .expect("goal should exist");
        let pause_result = runtime
            .pause_active_thread_goal(thread_id)
            .await
            .expect("terminal pause attempt should succeed");
        assert_eq!(None, pause_result);
        assert_eq!(
            Some(complete),
            runtime
                .get_thread_goal(thread_id)
                .await
                .expect("goal read should succeed")
        );
    }

    #[tokio::test]
    async fn usage_accounting_updates_active_goals_and_accounts_budget_limited_in_flight_usage() {
        let runtime = test_runtime().await;
        let thread_id = test_thread_id();
        upsert_test_thread(&runtime, thread_id).await;
        runtime
            .replace_thread_goal(
                thread_id,
                "stay within budget",
                crate::ThreadGoalStatus::Active,
                /*token_budget*/ Some(20),
            )
            .await
            .expect("goal replacement should succeed");

        let outcome = runtime
            .account_thread_goal_usage(
                thread_id,
                /*time_delta_seconds*/ 7,
                /*token_delta*/ 5,
                ThreadGoalAccountingMode::ActiveOnly,
                /*expected_goal_id*/ None,
            )
            .await
            .expect("usage accounting should succeed");
        let ThreadGoalAccountingOutcome::Updated(goal) = outcome else {
            panic!("active goal should be updated");
        };
        assert_eq!(crate::ThreadGoalStatus::Active, goal.status);
        assert_eq!(5, goal.tokens_used);
        assert_eq!(7, goal.time_used_seconds);

        let outcome = runtime
            .account_thread_goal_usage(
                thread_id,
                /*time_delta_seconds*/ 3,
                /*token_delta*/ 15,
                ThreadGoalAccountingMode::ActiveOnly,
                /*expected_goal_id*/ None,
            )
            .await
            .expect("usage accounting should succeed");
        let ThreadGoalAccountingOutcome::Updated(goal) = outcome else {
            panic!("budget crossing should update the goal");
        };
        assert_eq!(crate::ThreadGoalStatus::BudgetLimited, goal.status);
        assert_eq!(20, goal.tokens_used);
        assert_eq!(10, goal.time_used_seconds);

        let outcome = runtime
            .account_thread_goal_usage(
                thread_id,
                /*time_delta_seconds*/ 5,
                /*token_delta*/ 5,
                ThreadGoalAccountingMode::ActiveOnly,
                /*expected_goal_id*/ None,
            )
            .await
            .expect("usage accounting should succeed");
        let ThreadGoalAccountingOutcome::Updated(goal) = outcome else {
            panic!("budget-limited goal should still account in-flight active usage");
        };
        assert_eq!(crate::ThreadGoalStatus::BudgetLimited, goal.status);
        assert_eq!(25, goal.tokens_used);
        assert_eq!(15, goal.time_used_seconds);
    }

    #[tokio::test]
    async fn active_status_only_usage_accounting_does_not_update_budget_limited_goals() {
        let runtime = test_runtime().await;
        let thread_id = test_thread_id();
        upsert_test_thread(&runtime, thread_id).await;
        runtime
            .replace_thread_goal(
                thread_id,
                "stay stopped",
                crate::ThreadGoalStatus::BudgetLimited,
                /*token_budget*/ Some(20),
            )
            .await
            .expect("goal replacement should succeed");

        let outcome = runtime
            .account_thread_goal_usage(
                thread_id,
                /*time_delta_seconds*/ 5,
                /*token_delta*/ 5,
                ThreadGoalAccountingMode::ActiveStatusOnly,
                /*expected_goal_id*/ None,
            )
            .await
            .expect("usage accounting should succeed");
        let ThreadGoalAccountingOutcome::Unchanged(Some(goal)) = outcome else {
            panic!("budget-limited goal should not be updated");
        };
        assert_eq!(crate::ThreadGoalStatus::BudgetLimited, goal.status);
        assert_eq!(0, goal.tokens_used);
        assert_eq!(0, goal.time_used_seconds);
    }

    #[tokio::test]
    async fn stopped_usage_accounting_promotes_paused_goal_over_budget() {
        let runtime = test_runtime().await;
        let thread_id = test_thread_id();
        upsert_test_thread(&runtime, thread_id).await;
        runtime
            .replace_thread_goal(
                thread_id,
                "stop before overrun",
                crate::ThreadGoalStatus::Active,
                /*token_budget*/ Some(20),
            )
            .await
            .expect("goal replacement should succeed");
        runtime
            .update_thread_goal(
                thread_id,
                crate::ThreadGoalUpdate {
                    status: Some(crate::ThreadGoalStatus::Paused),
                    token_budget: None,
                },
            )
            .await
            .expect("goal update should succeed");

        let outcome = runtime
            .account_thread_goal_usage(
                thread_id,
                /*time_delta_seconds*/ 3,
                /*token_delta*/ 25,
                ThreadGoalAccountingMode::ActiveOrStopped,
                /*expected_goal_id*/ None,
            )
            .await
            .expect("usage accounting should succeed");
        let ThreadGoalAccountingOutcome::Updated(goal) = outcome else {
            panic!("stopped goal should account final usage");
        };
        assert_eq!(crate::ThreadGoalStatus::BudgetLimited, goal.status);
        assert_eq!(25, goal.tokens_used);
        assert_eq!(3, goal.time_used_seconds);
    }

    #[tokio::test]
    async fn budget_updates_immediately_stop_active_goals_already_over_budget() {
        let runtime = test_runtime().await;
        let thread_id = test_thread_id();
        upsert_test_thread(&runtime, thread_id).await;
        runtime
            .replace_thread_goal(
                thread_id,
                "stay within budget",
                crate::ThreadGoalStatus::Active,
                /*token_budget*/ Some(100),
            )
            .await
            .expect("goal replacement should succeed");
        runtime
            .account_thread_goal_usage(
                thread_id,
                /*time_delta_seconds*/ 1,
                /*token_delta*/ 50,
                ThreadGoalAccountingMode::ActiveOnly,
                /*expected_goal_id*/ None,
            )
            .await
            .expect("usage accounting should succeed");

        let lowered = runtime
            .update_thread_goal(
                thread_id,
                ThreadGoalUpdate {
                    status: None,
                    token_budget: Some(Some(40)),
                },
            )
            .await
            .expect("goal update should succeed")
            .expect("goal should exist");

        assert_eq!(crate::ThreadGoalStatus::BudgetLimited, lowered.status);
        assert_eq!(Some(40), lowered.token_budget);
        assert_eq!(50, lowered.tokens_used);
    }

    #[tokio::test]
    async fn activating_goal_already_over_budget_keeps_it_budget_limited() {
        let runtime = test_runtime().await;
        let thread_id = test_thread_id();
        upsert_test_thread(&runtime, thread_id).await;
        runtime
            .replace_thread_goal(
                thread_id,
                "stay within budget",
                crate::ThreadGoalStatus::Active,
                /*token_budget*/ Some(40),
            )
            .await
            .expect("goal replacement should succeed");
        runtime
            .account_thread_goal_usage(
                thread_id,
                /*time_delta_seconds*/ 1,
                /*token_delta*/ 50,
                ThreadGoalAccountingMode::ActiveOnly,
                /*expected_goal_id*/ None,
            )
            .await
            .expect("usage accounting should succeed");

        let reactivated = runtime
            .update_thread_goal(
                thread_id,
                ThreadGoalUpdate {
                    status: Some(crate::ThreadGoalStatus::Active),
                    token_budget: None,
                },
            )
            .await
            .expect("goal update should succeed")
            .expect("goal should exist");

        assert_eq!(crate::ThreadGoalStatus::BudgetLimited, reactivated.status);
        assert_eq!(Some(40), reactivated.token_budget);
        assert_eq!(50, reactivated.tokens_used);
    }

    #[tokio::test]
    async fn pausing_budget_limited_goal_preserves_terminal_status() {
        let runtime = test_runtime().await;
        let thread_id = test_thread_id();
        upsert_test_thread(&runtime, thread_id).await;
        runtime
            .replace_thread_goal(
                thread_id,
                "stay within budget",
                crate::ThreadGoalStatus::Active,
                /*token_budget*/ Some(40),
            )
            .await
            .expect("goal replacement should succeed");
        runtime
            .account_thread_goal_usage(
                thread_id,
                /*time_delta_seconds*/ 1,
                /*token_delta*/ 50,
                ThreadGoalAccountingMode::ActiveOnly,
                /*expected_goal_id*/ None,
            )
            .await
            .expect("usage accounting should succeed");

        let paused = runtime
            .update_thread_goal(
                thread_id,
                ThreadGoalUpdate {
                    status: Some(crate::ThreadGoalStatus::Paused),
                    token_budget: None,
                },
            )
            .await
            .expect("goal update should succeed")
            .expect("goal should exist");

        assert_eq!(crate::ThreadGoalStatus::BudgetLimited, paused.status);
        assert_eq!(Some(40), paused.token_budget);
        assert_eq!(50, paused.tokens_used);
    }

    #[tokio::test]
    async fn usage_accounting_can_finalize_completed_goal_for_completing_turn() {
        let runtime = test_runtime().await;
        let thread_id = test_thread_id();
        upsert_test_thread(&runtime, thread_id).await;
        runtime
            .replace_thread_goal(
                thread_id,
                "finish the report",
                crate::ThreadGoalStatus::Complete,
                /*token_budget*/ Some(1_000),
            )
            .await
            .expect("goal replacement should succeed");

        let active_only = runtime
            .account_thread_goal_usage(
                thread_id,
                /*time_delta_seconds*/ 30,
                /*token_delta*/ 200,
                ThreadGoalAccountingMode::ActiveOnly,
                /*expected_goal_id*/ None,
            )
            .await
            .expect("usage accounting should succeed");
        let ThreadGoalAccountingOutcome::Unchanged(Some(goal)) = active_only else {
            panic!("completed goal should not be updated by active-only accounting");
        };
        assert_eq!(crate::ThreadGoalStatus::Complete, goal.status);
        assert_eq!(0, goal.tokens_used);
        assert_eq!(0, goal.time_used_seconds);

        let completing_turn = runtime
            .account_thread_goal_usage(
                thread_id,
                /*time_delta_seconds*/ 30,
                /*token_delta*/ 200,
                ThreadGoalAccountingMode::ActiveOrComplete,
                /*expected_goal_id*/ None,
            )
            .await
            .expect("usage accounting should succeed");
        let ThreadGoalAccountingOutcome::Updated(goal) = completing_turn else {
            panic!("completed goal should be updated for final accounting");
        };
        assert_eq!(crate::ThreadGoalStatus::Complete, goal.status);
        assert_eq!(200, goal.tokens_used);
        assert_eq!(30, goal.time_used_seconds);
    }

    #[tokio::test]
    async fn usage_accounting_can_finalize_stopped_goal_for_in_flight_turn() {
        let runtime = test_runtime().await;
        let thread_id = test_thread_id();
        upsert_test_thread(&runtime, thread_id).await;
        runtime
            .replace_thread_goal(
                thread_id,
                "finish the report",
                crate::ThreadGoalStatus::Active,
                /*token_budget*/ Some(1_000),
            )
            .await
            .expect("goal replacement should succeed");
        runtime
            .update_thread_goal(
                thread_id,
                ThreadGoalUpdate {
                    status: Some(crate::ThreadGoalStatus::Paused),
                    token_budget: None,
                },
            )
            .await
            .expect("goal update should succeed")
            .expect("goal should exist");

        let active_only = runtime
            .account_thread_goal_usage(
                thread_id,
                /*time_delta_seconds*/ 30,
                /*token_delta*/ 200,
                ThreadGoalAccountingMode::ActiveOnly,
                /*expected_goal_id*/ None,
            )
            .await
            .expect("usage accounting should succeed");
        let ThreadGoalAccountingOutcome::Unchanged(Some(goal)) = active_only else {
            panic!("paused goal should not be updated by active-only accounting");
        };
        assert_eq!(crate::ThreadGoalStatus::Paused, goal.status);
        assert_eq!(0, goal.tokens_used);
        assert_eq!(0, goal.time_used_seconds);

        let in_flight_turn = runtime
            .account_thread_goal_usage(
                thread_id,
                /*time_delta_seconds*/ 30,
                /*token_delta*/ 200,
                ThreadGoalAccountingMode::ActiveOrStopped,
                /*expected_goal_id*/ None,
            )
            .await
            .expect("usage accounting should succeed");
        let ThreadGoalAccountingOutcome::Updated(goal) = in_flight_turn else {
            panic!("stopped goal should be updated for in-flight accounting");
        };
        assert_eq!(crate::ThreadGoalStatus::Paused, goal.status);
        assert_eq!(200, goal.tokens_used);
        assert_eq!(30, goal.time_used_seconds);
    }

    #[tokio::test]
    async fn usage_accounting_adds_concurrent_token_deltas() {
        let runtime = test_runtime().await;
        let thread_id = test_thread_id();
        upsert_test_thread(&runtime, thread_id).await;
        runtime
            .replace_thread_goal(
                thread_id,
                "count every token",
                crate::ThreadGoalStatus::Active,
                /*token_budget*/ Some(1_000),
            )
            .await
            .expect("goal replacement should succeed");

        let first = runtime.account_thread_goal_usage(
            thread_id,
            /*time_delta_seconds*/ 4,
            /*token_delta*/ 40,
            ThreadGoalAccountingMode::ActiveOnly,
            /*expected_goal_id*/ None,
        );
        let second = runtime.account_thread_goal_usage(
            thread_id,
            /*time_delta_seconds*/ 6,
            /*token_delta*/ 60,
            ThreadGoalAccountingMode::ActiveOnly,
            /*expected_goal_id*/ None,
        );
        let (first, second) = tokio::join!(first, second);
        first.expect("first usage accounting should succeed");
        second.expect("second usage accounting should succeed");

        let goal = runtime
            .get_thread_goal(thread_id)
            .await
            .expect("goal read should succeed")
            .expect("goal should exist");
        assert_eq!(100, goal.tokens_used);
        assert_eq!(10, goal.time_used_seconds);
    }

    #[tokio::test]
    async fn deleting_thread_deletes_goal() {
        let runtime = test_runtime().await;
        let thread_id = test_thread_id();
        upsert_test_thread(&runtime, thread_id).await;
        runtime
            .replace_thread_goal(
                thread_id,
                "clean up with the thread",
                crate::ThreadGoalStatus::Active,
                /*token_budget*/ None,
            )
            .await
            .expect("goal replacement should succeed");

        runtime
            .delete_thread(thread_id)
            .await
            .expect("thread deletion should succeed");

        assert_eq!(
            None,
            runtime
                .get_thread_goal(thread_id)
                .await
                .expect("goal read should succeed")
        );
    }
}
