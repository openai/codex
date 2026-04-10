use super::*;
use crate::model::ThreadTimerRow;
use tokio::sync::Mutex;

pub struct TimerDataVersionChecker {
    conn: Mutex<SqliteConnection>,
}

impl TimerDataVersionChecker {
    pub async fn data_version(&self) -> anyhow::Result<i64> {
        let mut conn = self.conn.lock().await;
        let version = sqlx::query_scalar::<_, i64>("PRAGMA data_version")
            .fetch_one(&mut *conn)
            .await?;
        Ok(version)
    }
}

impl StateRuntime {
    pub async fn timer_data_version_checker(&self) -> anyhow::Result<TimerDataVersionChecker> {
        let state_path = state_db_path(self.codex_home());
        let options = base_sqlite_options(state_path.as_path());
        let conn = options.connect().await?;
        Ok(TimerDataVersionChecker {
            conn: Mutex::new(conn),
        })
    }

    pub async fn create_thread_timer(
        &self,
        params: &ThreadTimerCreateParams,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
INSERT INTO thread_timers (
    id,
    thread_id,
    source,
    client_id,
    trigger_json,
    prompt,
    delivery,
    created_at,
    next_run_at,
    last_run_at,
    pending_run
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(params.id.as_str())
        .bind(params.thread_id.as_str())
        .bind(params.source.as_str())
        .bind(params.client_id.as_str())
        .bind(params.trigger_json.as_str())
        .bind(params.prompt.as_str())
        .bind(params.delivery.as_str())
        .bind(params.created_at)
        .bind(params.next_run_at)
        .bind(params.last_run_at)
        .bind(i64::from(params.pending_run))
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }

    pub async fn list_thread_timers(&self, thread_id: &str) -> anyhow::Result<Vec<ThreadTimer>> {
        let rows = sqlx::query_as::<_, ThreadTimerRow>(
            r#"
SELECT
    id,
    thread_id,
    source,
    client_id,
    trigger_json,
    prompt,
    delivery,
    created_at,
    next_run_at,
    last_run_at,
    pending_run
FROM thread_timers
WHERE thread_id = ?
ORDER BY created_at ASC, id ASC
            "#,
        )
        .bind(thread_id)
        .fetch_all(self.pool.as_ref())
        .await?;
        Ok(rows.into_iter().map(ThreadTimer::from).collect())
    }

    pub async fn delete_thread_timer(&self, thread_id: &str, id: &str) -> anyhow::Result<bool> {
        let result = sqlx::query("DELETE FROM thread_timers WHERE thread_id = ? AND id = ?")
            .bind(thread_id)
            .bind(id)
            .execute(self.pool.as_ref())
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn update_thread_timer_due(
        &self,
        thread_id: &str,
        id: &str,
        next_run_at: Option<i64>,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            r#"
UPDATE thread_timers
SET pending_run = 1,
    next_run_at = ?
WHERE thread_id = ?
  AND id = ?
            "#,
        )
        .bind(next_run_at)
        .bind(thread_id)
        .bind(id)
        .execute(self.pool.as_ref())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn claim_one_shot_thread_timer(
        &self,
        thread_id: &str,
        id: &str,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            r#"
DELETE FROM thread_timers
WHERE thread_id = ?
  AND id = ?
  AND pending_run = 1
            "#,
        )
        .bind(thread_id)
        .bind(id)
        .execute(self.pool.as_ref())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn claim_recurring_thread_timer(
        &self,
        thread_id: &str,
        id: &str,
        params: &ThreadTimerUpdateParams,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            r#"
UPDATE thread_timers
SET trigger_json = ?,
    delivery = ?,
    next_run_at = ?,
    last_run_at = ?,
    pending_run = ?
WHERE thread_id = ?
  AND id = ?
  AND pending_run = 1
            "#,
        )
        .bind(params.trigger_json.as_str())
        .bind(params.delivery.as_str())
        .bind(params.next_run_at)
        .bind(params.last_run_at)
        .bind(i64::from(params.pending_run))
        .bind(thread_id)
        .bind(id)
        .execute(self.pool.as_ref())
        .await?;
        Ok(result.rows_affected() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::StateRuntime;
    use super::test_support::unique_temp_dir;
    use crate::ThreadTimerCreateParams;
    use crate::ThreadTimerUpdateParams;
    use pretty_assertions::assert_eq;

    fn timer_params(id: &str, thread_id: &str) -> ThreadTimerCreateParams {
        ThreadTimerCreateParams {
            id: id.to_string(),
            thread_id: thread_id.to_string(),
            source: "agent".to_string(),
            client_id: "codex-tui".to_string(),
            trigger_json: r#"{"kind":"delay","seconds":10,"repeat":true}"#.to_string(),
            prompt: "run tests".to_string(),
            delivery: "after-turn".to_string(),
            created_at: 100,
            next_run_at: Some(110),
            last_run_at: None,
            pending_run: false,
        }
    }

    async fn test_runtime() -> std::sync::Arc<StateRuntime> {
        StateRuntime::init(unique_temp_dir(), "test-provider".to_string())
            .await
            .expect("initialize runtime")
    }

    #[tokio::test]
    async fn thread_timers_table_and_indexes_exist() {
        let runtime = test_runtime().await;
        let names = sqlx::query_scalar::<_, String>(
            r#"
SELECT name
FROM sqlite_master
WHERE tbl_name = 'thread_timers'
  AND name NOT LIKE 'sqlite_autoindex_%'
ORDER BY name
            "#,
        )
        .fetch_all(runtime.pool.as_ref())
        .await
        .expect("query schema objects");

        assert_eq!(
            names,
            vec![
                "idx_thread_timers_thread_created",
                "idx_thread_timers_thread_next_run",
                "idx_thread_timers_thread_pending",
                "thread_timers",
            ]
        );
    }

    #[tokio::test]
    async fn thread_timer_rows_round_trip_source_and_client_metadata() {
        let runtime = test_runtime().await;
        let mut params = timer_params("timer-1", "thread-1");
        params.pending_run = true;
        params.last_run_at = Some(105);

        runtime
            .create_thread_timer(&params)
            .await
            .expect("create timer");
        let timers = runtime
            .list_thread_timers("thread-1")
            .await
            .expect("list timers");

        assert_eq!(timers.len(), 1);
        let timer = &timers[0];
        assert_eq!(timer.id, params.id);
        assert_eq!(timer.thread_id, params.thread_id);
        assert_eq!(timer.source, params.source);
        assert_eq!(timer.client_id, params.client_id);
        assert_eq!(timer.trigger_json, params.trigger_json);
        assert_eq!(timer.prompt, params.prompt);
        assert_eq!(timer.delivery, params.delivery);
        assert_eq!(timer.created_at, params.created_at);
        assert_eq!(timer.next_run_at, params.next_run_at);
        assert_eq!(timer.last_run_at, params.last_run_at);
        assert_eq!(timer.pending_run, params.pending_run);
    }

    #[tokio::test]
    async fn thread_timer_crud_is_scoped_to_thread_id() {
        let runtime = test_runtime().await;
        runtime
            .create_thread_timer(&timer_params("timer-1", "thread-1"))
            .await
            .expect("create thread-1 timer");
        runtime
            .create_thread_timer(&timer_params("timer-2", "thread-2"))
            .await
            .expect("create thread-2 timer");

        assert_eq!(
            runtime
                .list_thread_timers("thread-1")
                .await
                .expect("list thread-1 timers")
                .into_iter()
                .map(|timer| timer.id)
                .collect::<Vec<_>>(),
            vec!["timer-1".to_string()]
        );
        assert!(
            !runtime
                .delete_thread_timer("thread-1", "timer-2")
                .await
                .expect("delete wrong thread timer")
        );
        assert!(
            runtime
                .delete_thread_timer("thread-2", "timer-2")
                .await
                .expect("delete correct thread timer")
        );
    }

    #[tokio::test]
    async fn one_shot_claim_consumes_pending_timer_once() {
        let runtime = test_runtime().await;
        let mut params = timer_params("timer-1", "thread-1");
        params.pending_run = true;
        params.next_run_at = None;
        runtime
            .create_thread_timer(&params)
            .await
            .expect("create pending timer");

        assert!(
            runtime
                .claim_one_shot_thread_timer("thread-1", "timer-1")
                .await
                .expect("claim timer")
        );
        assert!(
            !runtime
                .claim_one_shot_thread_timer("thread-1", "timer-1")
                .await
                .expect("claim timer again")
        );
        assert!(
            runtime
                .list_thread_timers("thread-1")
                .await
                .expect("list timers")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn recurring_claim_updates_pending_timer_once() {
        let runtime = test_runtime().await;
        let mut params = timer_params("timer-1", "thread-1");
        params.pending_run = true;
        runtime
            .create_thread_timer(&params)
            .await
            .expect("create pending timer");
        let update = ThreadTimerUpdateParams {
            trigger_json: params.trigger_json.clone(),
            delivery: "steer-current-turn".to_string(),
            next_run_at: Some(120),
            last_run_at: Some(110),
            pending_run: false,
        };

        assert!(
            runtime
                .claim_recurring_thread_timer("thread-1", "timer-1", &update)
                .await
                .expect("claim recurring timer")
        );
        assert!(
            !runtime
                .claim_recurring_thread_timer("thread-1", "timer-1", &update)
                .await
                .expect("claim recurring timer again")
        );
        let timers = runtime
            .list_thread_timers("thread-1")
            .await
            .expect("list timers");
        assert_eq!(timers.len(), 1);
        assert_eq!(timers[0].delivery, "steer-current-turn");
        assert_eq!(timers[0].next_run_at, Some(120));
        assert_eq!(timers[0].last_run_at, Some(110));
        assert!(!timers[0].pending_run);
    }
}
