//! Cleanup operations for per-thread delivery state.
//!
//! Timers and queued external messages are stored independently because they have
//! different runtime behavior, but thread lifecycle operations need to treat
//! them as one unit. This module owns that cross-table cleanup.

use super::*;

impl StateRuntime {
    /// Delete all queued external messages and timers associated with `thread_id`.
    pub async fn delete_thread_delivery_state(&self, thread_id: &str) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM external_messages WHERE thread_id = ?")
            .bind(thread_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM thread_timers WHERE thread_id = ?")
            .bind(thread_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::StateRuntime;
    use super::test_support::unique_temp_dir;
    use crate::ExternalMessageCreateParams;
    use crate::ThreadTimerCreateParams;
    use pretty_assertions::assert_eq;

    fn message_params(id: &str, thread_id: &str) -> ExternalMessageCreateParams {
        ExternalMessageCreateParams {
            id: id.to_string(),
            thread_id: thread_id.to_string(),
            source: "external".to_string(),
            content: "do something".to_string(),
            instructions: None,
            meta_json: "{}".to_string(),
            delivery: "after-turn".to_string(),
            queued_at: 100,
        }
    }

    fn timer_params(id: &str, thread_id: &str) -> ThreadTimerCreateParams {
        ThreadTimerCreateParams {
            id: id.to_string(),
            thread_id: thread_id.to_string(),
            source: "agent".to_string(),
            client_id: "codex-tui".to_string(),
            trigger_json: r#"{"kind":"delay","seconds":10,"repeat":false}"#.to_string(),
            content: "run tests".to_string(),
            instructions: None,
            meta_json: "{}".to_string(),
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
    async fn delete_thread_delivery_state_removes_messages_and_timers_for_thread() {
        let runtime = test_runtime().await;
        runtime
            .create_external_message(&message_params("message-1", "thread-1"))
            .await
            .expect("create thread-1 message");
        runtime
            .create_external_message(&message_params("message-2", "thread-2"))
            .await
            .expect("create thread-2 message");
        runtime
            .create_thread_timer(&timer_params("timer-1", "thread-1"))
            .await
            .expect("create thread-1 timer");
        runtime
            .create_thread_timer(&timer_params("timer-2", "thread-2"))
            .await
            .expect("create thread-2 timer");

        runtime
            .delete_thread_delivery_state("thread-1")
            .await
            .expect("delete delivery state");

        assert_eq!(
            runtime
                .list_external_messages("thread-1")
                .await
                .expect("list thread-1 messages"),
            Vec::new()
        );
        assert_eq!(
            runtime
                .list_thread_timers("thread-1")
                .await
                .expect("list thread-1 timers"),
            Vec::new()
        );
        assert_eq!(
            runtime
                .list_external_messages("thread-2")
                .await
                .expect("list thread-2 messages")
                .into_iter()
                .map(|message| message.id)
                .collect::<Vec<_>>(),
            vec!["message-2".to_string()]
        );
        assert_eq!(
            runtime
                .list_thread_timers("thread-2")
                .await
                .expect("list thread-2 timers")
                .into_iter()
                .map(|timer| timer.id)
                .collect::<Vec<_>>(),
            vec!["timer-2".to_string()]
        );
    }
}
