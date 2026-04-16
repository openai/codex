use super::*;

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
    ActiveOnly,
    ActiveOrComplete,
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
    objective,
    status,
    token_budget,
    tokens_used,
    created_at_ms,
    updated_at_ms
FROM thread_goals
WHERE thread_id = ?
            "#,
        )
        .bind(thread_id.to_string())
        .fetch_optional(self.pool.as_ref())
        .await?;

        row.map(|row| ThreadGoalRow::try_from_row(&row).and_then(crate::ThreadGoal::try_from))
            .transpose()
    }

    pub async fn replace_thread_goal(
        &self,
        thread_id: ThreadId,
        objective: &str,
        status: crate::ThreadGoalStatus,
        token_budget: Option<i64>,
    ) -> anyhow::Result<crate::ThreadGoal> {
        let now_ms = datetime_to_epoch_millis(Utc::now());
        sqlx::query(
            r#"
INSERT INTO thread_goals (
    thread_id,
    objective,
    status,
    token_budget,
    tokens_used,
    created_at_ms,
    updated_at_ms
) VALUES (?, ?, ?, ?, 0, ?, ?)
ON CONFLICT(thread_id) DO UPDATE SET
    objective = excluded.objective,
    status = excluded.status,
    token_budget = excluded.token_budget,
    tokens_used = 0,
    created_at_ms = excluded.created_at_ms,
    updated_at_ms = excluded.updated_at_ms
            "#,
        )
        .bind(thread_id.to_string())
        .bind(objective)
        .bind(status.as_str())
        .bind(token_budget)
        .bind(now_ms)
        .bind(now_ms)
        .execute(self.pool.as_ref())
        .await?;

        self.get_thread_goal(thread_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("thread goal disappeared after replacement"))
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
    status = ?,
    token_budget = ?,
    updated_at_ms = ?
WHERE thread_id = ?
            "#,
                )
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
    status = ?,
    updated_at_ms = ?
WHERE thread_id = ?
            "#,
                )
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
    updated_at_ms = ?
WHERE thread_id = ?
            "#,
                )
                .bind(token_budget)
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

    pub async fn account_thread_goal_usage(
        &self,
        thread_id: ThreadId,
        token_delta: i64,
        mode: ThreadGoalAccountingMode,
    ) -> anyhow::Result<ThreadGoalAccountingOutcome> {
        if token_delta <= 0 {
            return Ok(ThreadGoalAccountingOutcome::Unchanged(
                self.get_thread_goal(thread_id).await?,
            ));
        }

        let now_ms = datetime_to_epoch_millis(Utc::now());
        let status_filter = match mode {
            ThreadGoalAccountingMode::ActiveOnly => "status = 'active'",
            ThreadGoalAccountingMode::ActiveOrComplete => "status IN ('active', 'complete')",
        };
        let query = format!(
            r#"
UPDATE thread_goals
SET
    tokens_used = tokens_used + ?,
    status = CASE
        WHEN status = 'active' AND token_budget IS NOT NULL AND tokens_used + ? >= token_budget
            THEN ?
        ELSE status
    END,
    updated_at_ms = ?
WHERE thread_id = ?
  AND {status_filter}
            "#,
        );

        let result = sqlx::query(&query)
            .bind(token_delta)
            .bind(token_delta)
            .bind(crate::ThreadGoalStatus::BudgetLimited.as_str())
            .bind(now_ms)
            .bind(thread_id.to_string())
            .execute(self.pool.as_ref())
            .await?;

        if result.rows_affected() == 0 {
            return Ok(ThreadGoalAccountingOutcome::Unchanged(
                self.get_thread_goal(thread_id).await?,
            ));
        }

        let updated = self
            .get_thread_goal(thread_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("thread goal disappeared after usage accounting"))?;
        Ok(ThreadGoalAccountingOutcome::Updated(updated))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[tokio::test]
    async fn replace_update_and_get_thread_goal() {
        let runtime = test_runtime().await;
        let thread_id = test_thread_id();

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
    }

    #[tokio::test]
    async fn concurrent_partial_updates_preserve_independent_fields() {
        let runtime = test_runtime().await;
        let thread_id = test_thread_id();
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
    async fn usage_accounting_updates_active_goals_and_stops_on_budget() {
        let runtime = test_runtime().await;
        let thread_id = test_thread_id();
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
                /*token_delta*/ 5,
                ThreadGoalAccountingMode::ActiveOnly,
            )
            .await
            .expect("usage accounting should succeed");
        let ThreadGoalAccountingOutcome::Updated(goal) = outcome else {
            panic!("active goal should be updated");
        };
        assert_eq!(crate::ThreadGoalStatus::Active, goal.status);
        assert_eq!(5, goal.tokens_used);

        let outcome = runtime
            .account_thread_goal_usage(
                thread_id,
                /*token_delta*/ 15,
                ThreadGoalAccountingMode::ActiveOnly,
            )
            .await
            .expect("usage accounting should succeed");
        let ThreadGoalAccountingOutcome::Updated(goal) = outcome else {
            panic!("budget crossing should update the goal");
        };
        assert_eq!(crate::ThreadGoalStatus::BudgetLimited, goal.status);
        assert_eq!(20, goal.tokens_used);

        let outcome = runtime
            .account_thread_goal_usage(
                thread_id,
                /*token_delta*/ 5,
                ThreadGoalAccountingMode::ActiveOnly,
            )
            .await
            .expect("usage accounting should succeed");
        let ThreadGoalAccountingOutcome::Unchanged(Some(goal)) = outcome else {
            panic!("terminal goal should not continue accounting");
        };
        assert_eq!(crate::ThreadGoalStatus::BudgetLimited, goal.status);
        assert_eq!(20, goal.tokens_used);
    }

    #[tokio::test]
    async fn usage_accounting_can_finalize_completed_goal_for_completing_turn() {
        let runtime = test_runtime().await;
        let thread_id = test_thread_id();
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
                /*token_delta*/ 200,
                ThreadGoalAccountingMode::ActiveOnly,
            )
            .await
            .expect("usage accounting should succeed");
        let ThreadGoalAccountingOutcome::Unchanged(Some(goal)) = active_only else {
            panic!("completed goal should not be updated by active-only accounting");
        };
        assert_eq!(crate::ThreadGoalStatus::Complete, goal.status);
        assert_eq!(0, goal.tokens_used);

        let completing_turn = runtime
            .account_thread_goal_usage(
                thread_id,
                /*token_delta*/ 200,
                ThreadGoalAccountingMode::ActiveOrComplete,
            )
            .await
            .expect("usage accounting should succeed");
        let ThreadGoalAccountingOutcome::Updated(goal) = completing_turn else {
            panic!("completed goal should be updated for final accounting");
        };
        assert_eq!(crate::ThreadGoalStatus::Complete, goal.status);
        assert_eq!(200, goal.tokens_used);
    }

    #[tokio::test]
    async fn usage_accounting_adds_concurrent_token_deltas() {
        let runtime = test_runtime().await;
        let thread_id = test_thread_id();
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
            /*token_delta*/ 40,
            ThreadGoalAccountingMode::ActiveOnly,
        );
        let second = runtime.account_thread_goal_usage(
            thread_id,
            /*token_delta*/ 60,
            ThreadGoalAccountingMode::ActiveOnly,
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
    }
}
