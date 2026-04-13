//! SQLite-backed state operations for queued external messages.
//!
//! This module extends [`StateRuntime`] with the storage APIs used by message
//! producers and active threads. Claiming a message deletes the row inside the
//! same transaction, so competing runtimes deliver each queued message at most
//! once.

use super::*;
use crate::model::ExternalMessageRow;

const DELIVERY_AFTER_TURN: &str = "after-turn";
const DELIVERY_STEER_CURRENT_TURN: &str = "steer-current-turn";

impl StateRuntime {
    pub async fn create_external_message(
        &self,
        params: &ExternalMessageCreateParams,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
INSERT INTO external_messages (
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

    pub async fn list_external_messages(
        &self,
        thread_id: &str,
    ) -> anyhow::Result<Vec<ExternalMessage>> {
        let rows = sqlx::query_as::<_, ExternalMessageRow>(
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
FROM external_messages
WHERE thread_id = ?
ORDER BY queued_at ASC, seq ASC
            "#,
        )
        .bind(thread_id)
        .fetch_all(self.pool.as_ref())
        .await?;
        Ok(rows.into_iter().map(ExternalMessage::from).collect())
    }

    pub async fn delete_external_message(&self, thread_id: &str, id: &str) -> anyhow::Result<bool> {
        let result = sqlx::query("DELETE FROM external_messages WHERE thread_id = ? AND id = ?")
            .bind(thread_id)
            .bind(id)
            .execute(self.pool.as_ref())
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn claim_next_external_message(
        &self,
        thread_id: &str,
        can_after_turn: bool,
        can_steer_current_turn: bool,
    ) -> anyhow::Result<Option<ExternalMessageClaim>> {
        let row = sqlx::query_as::<_, ExternalMessageRow>(
            r#"
DELETE FROM external_messages
WHERE seq = (
    SELECT seq
    FROM external_messages
    WHERE thread_id = ?
    ORDER BY queued_at ASC, seq ASC
    LIMIT 1
)
AND (
    delivery NOT IN (?, ?)
    OR (delivery = ? AND ?)
    OR (delivery = ? AND ?)
)
RETURNING
    seq,
    id,
    thread_id,
    source,
    content,
    instructions,
    meta_json,
    delivery,
    queued_at
            "#,
        )
        .bind(thread_id)
        .bind(DELIVERY_AFTER_TURN)
        .bind(DELIVERY_STEER_CURRENT_TURN)
        .bind(DELIVERY_AFTER_TURN)
        .bind(can_after_turn)
        .bind(DELIVERY_STEER_CURRENT_TURN)
        .bind(can_steer_current_turn || can_after_turn)
        .fetch_optional(self.pool.as_ref())
        .await?;

        if let Some(row) = row {
            return match row.delivery.as_str() {
                DELIVERY_AFTER_TURN | DELIVERY_STEER_CURRENT_TURN => Ok(Some(
                    ExternalMessageClaim::Claimed(ExternalMessage::from(row)),
                )),
                delivery => Ok(Some(ExternalMessageClaim::Invalid {
                    id: row.id,
                    reason: format!("invalid delivery `{delivery}`"),
                })),
            };
        }

        let oldest_delivery = sqlx::query_scalar::<_, String>(
            r#"
SELECT delivery
FROM external_messages
WHERE thread_id = ?
ORDER BY queued_at ASC, seq ASC
LIMIT 1
            "#,
        )
        .bind(thread_id)
        .fetch_optional(self.pool.as_ref())
        .await?;

        match oldest_delivery.as_deref() {
            Some(DELIVERY_AFTER_TURN) if !can_after_turn => {
                Ok(Some(ExternalMessageClaim::NotReady))
            }
            Some(DELIVERY_STEER_CURRENT_TURN) if !(can_steer_current_turn || can_after_turn) => {
                Ok(Some(ExternalMessageClaim::NotReady))
            }
            None | Some(_) => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::StateRuntime;
    use super::test_support::unique_temp_dir;
    use crate::ExternalMessageClaim;
    use crate::ExternalMessageCreateParams;
    use pretty_assertions::assert_eq;

    fn message_params(id: &str, thread_id: &str, queued_at: i64) -> ExternalMessageCreateParams {
        ExternalMessageCreateParams {
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
    async fn external_messages_table_and_indexes_exist() {
        let runtime = test_runtime().await;
        let names = sqlx::query_scalar::<_, String>(
            r#"
SELECT name
FROM sqlite_master
WHERE tbl_name = 'external_messages'
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
                "external_messages",
                "external_messages_thread_delivery_order_idx",
                "external_messages_thread_order_idx",
            ]
        );
    }

    #[tokio::test]
    async fn external_message_rows_round_trip() {
        let runtime = test_runtime().await;
        let params = message_params("message-1", "thread-1", /*queued_at*/ 100);

        runtime
            .create_external_message(&params)
            .await
            .expect("create message");
        let messages = runtime
            .list_external_messages("thread-1")
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
    async fn delete_external_message_is_scoped_to_thread_id() {
        let runtime = test_runtime().await;
        runtime
            .create_external_message(&message_params(
                "message-1",
                "thread-1",
                /*queued_at*/ 100,
            ))
            .await
            .expect("create thread-1 message");
        runtime
            .create_external_message(&message_params(
                "message-2",
                "thread-2",
                /*queued_at*/ 100,
            ))
            .await
            .expect("create thread-2 message");

        let deleted_wrong_thread = runtime
            .delete_external_message("thread-2", "message-1")
            .await
            .expect("delete wrong-external message");
        assert!(!deleted_wrong_thread);
        let deleted = runtime
            .delete_external_message("thread-1", "message-1")
            .await
            .expect("delete thread-1 message");
        assert!(deleted);
        assert_eq!(
            runtime
                .list_external_messages("thread-1")
                .await
                .expect("list thread-1 messages"),
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
    }

    #[tokio::test]
    async fn claim_is_scoped_to_thread_id_and_ordered() {
        let runtime = test_runtime().await;
        runtime
            .create_external_message(&message_params("newer", "thread-1", /*queued_at*/ 200))
            .await
            .expect("create newer message");
        runtime
            .create_external_message(&message_params(
                "other-thread",
                "thread-2",
                /*queued_at*/ 50,
            ))
            .await
            .expect("create other external message");
        runtime
            .create_external_message(&message_params("older", "thread-1", /*queued_at*/ 100))
            .await
            .expect("create older message");

        let claim = runtime
            .claim_next_external_message(
                "thread-1", /*can_after_turn*/ true, /*can_steer_current_turn*/ true,
            )
            .await
            .expect("claim message");

        let Some(ExternalMessageClaim::Claimed(claimed)) = claim else {
            panic!("expected claimed message");
        };
        assert_eq!(claimed.id, "older");
        assert_eq!(claimed.thread_id, "thread-1");
        assert_eq!(claimed.queued_at, 100);
        assert_eq!(
            runtime
                .list_external_messages("thread-1")
                .await
                .expect("list remaining thread-1 messages")
                .into_iter()
                .map(|message| message.id)
                .collect::<Vec<_>>(),
            vec!["newer".to_string()]
        );
        assert_eq!(
            runtime
                .list_external_messages("thread-2")
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
            .create_external_message(&message_params(
                "message-1",
                "thread-1",
                /*queued_at*/ 100,
            ))
            .await
            .expect("create message");

        assert!(matches!(
            runtime
                .claim_next_external_message(
                    "thread-1", /*can_after_turn*/ true, /*can_steer_current_turn*/ true,
                )
                .await
                .expect("claim message"),
            Some(ExternalMessageClaim::Claimed(_))
        ));
        assert_eq!(
            runtime
                .claim_next_external_message(
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
        let mut steer = message_params("steer", "thread-1", /*queued_at*/ 100);
        steer.delivery = "steer-current-turn".to_string();
        runtime
            .create_external_message(&steer)
            .await
            .expect("create steer message");
        runtime
            .create_external_message(&message_params("after", "thread-1", /*queued_at*/ 200))
            .await
            .expect("create after-turn message");

        assert_eq!(
            runtime
                .claim_next_external_message(
                    "thread-1", /*can_after_turn*/ false,
                    /*can_steer_current_turn*/ false,
                )
                .await
                .expect("claim message"),
            Some(ExternalMessageClaim::NotReady)
        );
        assert_eq!(
            runtime
                .list_external_messages("thread-1")
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
        let mut params = message_params("bad", "thread-1", /*queued_at*/ 100);
        params.delivery = "bad-delivery".to_string();
        runtime
            .create_external_message(&params)
            .await
            .expect("create message");

        assert_eq!(
            runtime
                .claim_next_external_message(
                    "thread-1", /*can_after_turn*/ true, /*can_steer_current_turn*/ true,
                )
                .await
                .expect("claim message"),
            Some(ExternalMessageClaim::Invalid {
                id: "bad".to_string(),
                reason: "invalid delivery `bad-delivery`".to_string(),
            })
        );
        assert!(
            runtime
                .list_external_messages("thread-1")
                .await
                .expect("list messages")
                .is_empty()
        );
    }
}
