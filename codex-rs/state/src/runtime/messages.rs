//! SQLite-backed state operations for queued thread messages.
//!
//! This module extends [`StateRuntime`] with the storage APIs used by message
//! producers and active threads. Claiming a message deletes the row inside the
//! same transaction, so competing runtimes deliver each queued message at most
//! once.

use super::*;
use crate::model::ThreadMessageRow;

const DELIVERY_AFTER_TURN: &str = "after-turn";
const DELIVERY_STEER_CURRENT_TURN: &str = "steer-current-turn";

impl StateRuntime {
    pub async fn create_thread_message(
        &self,
        params: &ThreadMessageCreateParams,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
INSERT INTO thread_messages (
    id,
    thread_id,
    source,
    content,
    instructions,
    meta_json,
    delivery,
    queued_at
) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(params.id.as_str())
        .bind(params.thread_id.as_str())
        .bind(params.source.as_str())
        .bind(params.content.as_str())
        .bind(params.instructions.as_deref())
        .bind(params.meta_json.as_str())
        .bind(params.delivery.as_str())
        .bind(params.queued_at)
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }

    pub async fn list_thread_messages(
        &self,
        thread_id: &str,
    ) -> anyhow::Result<Vec<ThreadMessage>> {
        let rows = sqlx::query_as::<_, ThreadMessageRow>(
            r#"
SELECT
    seq,
    id,
    thread_id,
    source,
    content,
    instructions,
    meta_json,
    delivery,
    queued_at
FROM thread_messages
WHERE thread_id = ?
ORDER BY queued_at ASC, seq ASC
            "#,
        )
        .bind(thread_id)
        .fetch_all(self.pool.as_ref())
        .await?;
        Ok(rows.into_iter().map(ThreadMessage::from).collect())
    }

    pub async fn claim_next_thread_message(
        &self,
        thread_id: &str,
        can_after_turn: bool,
        can_steer_current_turn: bool,
    ) -> anyhow::Result<Option<ThreadMessageClaim>> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query_as::<_, ThreadMessageRow>(
            r#"
SELECT
    seq,
    id,
    thread_id,
    source,
    content,
    instructions,
    meta_json,
    delivery,
    queued_at
FROM thread_messages
WHERE thread_id = ?
ORDER BY queued_at ASC, seq ASC
LIMIT 1
            "#,
        )
        .bind(thread_id)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(row) = row else {
            tx.commit().await?;
            return Ok(None);
        };

        let can_claim = match row.delivery.as_str() {
            DELIVERY_AFTER_TURN => can_after_turn,
            DELIVERY_STEER_CURRENT_TURN => can_steer_current_turn || can_after_turn,
            delivery => {
                sqlx::query("DELETE FROM thread_messages WHERE seq = ? AND id = ?")
                    .bind(row.seq)
                    .bind(row.id.as_str())
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
                return Ok(Some(ThreadMessageClaim::Invalid {
                    id: row.id,
                    reason: format!("invalid delivery `{delivery}`"),
                }));
            }
        };
        if !can_claim {
            tx.commit().await?;
            return Ok(Some(ThreadMessageClaim::NotReady));
        }

        let result = sqlx::query("DELETE FROM thread_messages WHERE seq = ? AND id = ?")
            .bind(row.seq)
            .bind(row.id.as_str())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        Ok(Some(ThreadMessageClaim::Claimed(ThreadMessage::from(row))))
    }
}

#[cfg(test)]
mod tests {
    use super::StateRuntime;
    use super::test_support::unique_temp_dir;
    use crate::ThreadMessageClaim;
    use crate::ThreadMessageCreateParams;
    use pretty_assertions::assert_eq;

    fn message_params(id: &str, thread_id: &str, queued_at: i64) -> ThreadMessageCreateParams {
        ThreadMessageCreateParams {
            id: id.to_string(),
            thread_id: thread_id.to_string(),
            source: "external".to_string(),
            content: "do something".to_string(),
            instructions: Some("be concise".to_string()),
            meta_json: r#"{"ticket":"ABC_123"}"#.to_string(),
            delivery: "after-turn".to_string(),
            queued_at,
        }
    }

    async fn test_runtime() -> std::sync::Arc<StateRuntime> {
        StateRuntime::init(unique_temp_dir(), "test-provider".to_string())
            .await
            .expect("initialize runtime")
    }

    #[tokio::test]
    async fn thread_messages_table_and_indexes_exist() {
        let runtime = test_runtime().await;
        let names = sqlx::query_scalar::<_, String>(
            r#"
SELECT name
FROM sqlite_master
WHERE tbl_name = 'thread_messages'
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
                "thread_messages",
                "thread_messages_thread_delivery_order_idx",
                "thread_messages_thread_order_idx",
            ]
        );
    }

    #[tokio::test]
    async fn thread_message_rows_round_trip() {
        let runtime = test_runtime().await;
        let params = message_params("message-1", "thread-1", 100);

        runtime
            .create_thread_message(&params)
            .await
            .expect("create message");
        let messages = runtime
            .list_thread_messages("thread-1")
            .await
            .expect("list messages");

        assert_eq!(messages.len(), 1);
        let message = &messages[0];
        assert_eq!(message.id, params.id);
        assert_eq!(message.thread_id, params.thread_id);
        assert_eq!(message.source, params.source);
        assert_eq!(message.content, params.content);
        assert_eq!(message.instructions, params.instructions);
        assert_eq!(message.meta_json, params.meta_json);
        assert_eq!(message.delivery, params.delivery);
        assert_eq!(message.queued_at, params.queued_at);
    }

    #[tokio::test]
    async fn claim_is_scoped_to_thread_id_and_ordered() {
        let runtime = test_runtime().await;
        runtime
            .create_thread_message(&message_params("newer", "thread-1", 200))
            .await
            .expect("create newer message");
        runtime
            .create_thread_message(&message_params("other-thread", "thread-2", 50))
            .await
            .expect("create other thread message");
        runtime
            .create_thread_message(&message_params("older", "thread-1", 100))
            .await
            .expect("create older message");

        let claim = runtime
            .claim_next_thread_message(
                "thread-1", /*can_after_turn*/ true, /*can_steer_current_turn*/ true,
            )
            .await
            .expect("claim message");

        let Some(ThreadMessageClaim::Claimed(claimed)) = claim else {
            panic!("expected claimed message");
        };
        assert_eq!(claimed.id, "older");
        assert_eq!(claimed.thread_id, "thread-1");
        assert_eq!(claimed.queued_at, 100);
        assert_eq!(
            runtime
                .list_thread_messages("thread-1")
                .await
                .expect("list remaining thread-1 messages")
                .into_iter()
                .map(|message| message.id)
                .collect::<Vec<_>>(),
            vec!["newer".to_string()]
        );
        assert_eq!(
            runtime
                .list_thread_messages("thread-2")
                .await
                .expect("list thread-2 messages")
                .into_iter()
                .map(|message| message.id)
                .collect::<Vec<_>>(),
            vec!["other-thread".to_string()]
        );
    }

    #[tokio::test]
    async fn claim_consumes_message_once() {
        let runtime = test_runtime().await;
        runtime
            .create_thread_message(&message_params("message-1", "thread-1", 100))
            .await
            .expect("create message");

        assert!(matches!(
            runtime
                .claim_next_thread_message(
                    "thread-1", /*can_after_turn*/ true, /*can_steer_current_turn*/ true,
                )
                .await
                .expect("claim message"),
            Some(ThreadMessageClaim::Claimed(_))
        ));
        assert_eq!(
            runtime
                .claim_next_thread_message(
                    "thread-1", /*can_after_turn*/ true, /*can_steer_current_turn*/ true,
                )
                .await
                .expect("claim message again"),
            None
        );
    }

    #[tokio::test]
    async fn oldest_unclaimable_message_blocks_later_messages() {
        let runtime = test_runtime().await;
        let mut steer = message_params("steer", "thread-1", 100);
        steer.delivery = "steer-current-turn".to_string();
        runtime
            .create_thread_message(&steer)
            .await
            .expect("create steer message");
        runtime
            .create_thread_message(&message_params("after", "thread-1", 200))
            .await
            .expect("create after-turn message");

        assert_eq!(
            runtime
                .claim_next_thread_message(
                    "thread-1", /*can_after_turn*/ false,
                    /*can_steer_current_turn*/ false,
                )
                .await
                .expect("claim message"),
            Some(ThreadMessageClaim::NotReady)
        );
        assert_eq!(
            runtime
                .list_thread_messages("thread-1")
                .await
                .expect("list messages")
                .into_iter()
                .map(|message| message.id)
                .collect::<Vec<_>>(),
            vec!["steer".to_string(), "after".to_string()]
        );
    }

    #[tokio::test]
    async fn invalid_delivery_is_deleted_without_claiming() {
        let runtime = test_runtime().await;
        let mut params = message_params("bad", "thread-1", 100);
        params.delivery = "bad-delivery".to_string();
        runtime
            .create_thread_message(&params)
            .await
            .expect("create message");

        assert_eq!(
            runtime
                .claim_next_thread_message(
                    "thread-1", /*can_after_turn*/ true, /*can_steer_current_turn*/ true,
                )
                .await
                .expect("claim message"),
            Some(ThreadMessageClaim::Invalid {
                id: "bad".to_string(),
                reason: "invalid delivery `bad-delivery`".to_string(),
            })
        );
        assert!(
            runtime
                .list_thread_messages("thread-1")
                .await
                .expect("list messages")
                .is_empty()
        );
    }
}
