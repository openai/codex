//! One durable unread position per local thread, independent of rebuildable metadata.
//! Reads do not materialize state. Live terminal events are delivered serially per thread;
//! keep their last key across acknowledgements and reverts to suppress redelivery.

use std::collections::HashMap;

use codex_protocol::ThreadId;
use sqlx::QueryBuilder;
use sqlx::Row;
use sqlx::Sqlite;
use sqlx::SqliteConnection;
use uuid::Uuid;

use super::StateRuntime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadReadState {
    /// None is read; Some("") marks the whole thread unread, even if it has no turns.
    pub first_unread_turn: Option<String>,
    pub revision: String,
}

pub enum ReadStateOperation {
    Read,
    Unread,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ReadStateUpdate {
    Applied(ThreadReadState),
    Conflict(ThreadReadState),
    Unavailable,
}

impl StateRuntime {
    /// Read just the requested page. Only durable metadata rows are eligible.
    pub async fn thread_read_states(
        &self,
        ids: &[ThreadId],
    ) -> anyhow::Result<HashMap<ThreadId, ThreadReadState>> {
        let mut conn = self.pool.acquire().await?;
        read_states(&mut conn, ids).await
    }

    /// Publish a newly delivered terminal event only once, never on history replay.
    /// Every newer result guards against an acknowledgement based on an older snapshot,
    /// even if the first unread position stays the same.
    pub async fn publish_thread_attention(
        &self,
        id: ThreadId,
        turn_id: &str,
    ) -> anyhow::Result<bool> {
        let changed = sqlx::query(
            "INSERT INTO thread_read_receipts(thread_id,first_unread_turn,revision,last_published_turn)
             SELECT id, ?, ?, ? FROM threads WHERE id=?
             ON CONFLICT(thread_id) DO UPDATE SET
               first_unread_turn=COALESCE(thread_read_receipts.first_unread_turn,excluded.first_unread_turn),
               revision=excluded.revision,
               last_published_turn=excluded.last_published_turn
             WHERE thread_read_receipts.last_published_turn IS NOT excluded.last_published_turn",
        )
        .bind(turn_id)
        .bind(Uuid::new_v4().to_string())
        .bind(turn_id)
        .bind(id.to_string())
        .execute(self.pool.as_ref())
        .await?
        .rows_affected() != 0;
        Ok(changed)
    }

    /// Both read and unread edits compare the snapshot the client actually saw. Never retry a
    /// stale edit automatically: its author may not have seen a newer result or another mark.
    pub async fn update_thread_read_state(
        &self,
        id: ThreadId,
        expected_revision: &str,
        operation: ReadStateOperation,
    ) -> anyhow::Result<ReadStateUpdate> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let Some(before) = read_states(&mut tx, &[id]).await?.remove(&id) else {
            return Ok(ReadStateUpdate::Unavailable);
        };
        if before.revision != expected_revision {
            return Ok(ReadStateUpdate::Conflict(before));
        }
        let first_unread = match operation {
            ReadStateOperation::Read => None,
            ReadStateOperation::Unread => Some(""),
        };
        let revision = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO thread_read_receipts(thread_id,first_unread_turn,revision)
             VALUES(?,?,?) ON CONFLICT(thread_id) DO UPDATE
             SET first_unread_turn=excluded.first_unread_turn, revision=excluded.revision",
        )
        .bind(id.to_string())
        .bind(first_unread)
        .bind(&revision)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(ReadStateUpdate::Applied(ThreadReadState {
            first_unread_turn: first_unread.map(str::to_owned),
            revision,
        }))
    }
}

async fn read_states(
    conn: &mut SqliteConnection,
    ids: &[ThreadId],
) -> anyhow::Result<HashMap<ThreadId, ThreadReadState>> {
    let mut result = HashMap::new();
    for ids in ids.chunks(998) {
        let mut query = QueryBuilder::<Sqlite>::new(
            "SELECT t.id, r.first_unread_turn, r.revision FROM threads t
             LEFT JOIN thread_read_receipts r ON r.thread_id=t.id WHERE t.id IN (",
        );
        let mut separated = query.separated(", ");
        for id in ids {
            separated.push_bind(id.to_string());
        }
        separated.push_unseparated(")");
        for row in query.build().fetch_all(&mut *conn).await? {
            let id: String = row.try_get("id")?;
            let revision: Option<String> = row.try_get("revision")?;
            result.insert(
                ThreadId::from_string(&id)?,
                ThreadReadState {
                    first_unread_turn: row.try_get("first_unread_turn")?,
                    // No baseline write. Bind the initial revision to the thread so it cannot
                    // accidentally be used to edit another thread with no receipt yet.
                    revision: revision.unwrap_or_else(|| format!("initial:{id}")),
                },
            );
        }
    }
    Ok(result)
}

#[cfg(test)]
#[path = "thread_read_state_tests.rs"]
mod tests;
