//! Read channel metadata and permissions for a local board.

use super::LocalAgentMessageBoard;
use super::connection::Access;
use super::invalid;
use super::storage_error;
use crate::ChannelPermissions;
use crate::ChannelSummary;
use chrono::DateTime;
use chrono::Utc;
use codex_protocol::AgentPath;
use codex_protocol::error::Result;
use sqlx::Row;
use sqlx::SqliteConnection;
use uuid::Uuid;

pub(super) async fn channel_summary(
    board: &LocalAgentMessageBoard,
    conn: &mut SqliteConnection,
    name: &str,
) -> Result<ChannelSummary> {
    let row = sqlx::query(
            "SELECT c.name,c.created_at,c.author,c.description,
             (SELECT COUNT(*) FROM posts p WHERE p.board=c.board AND p.channel=c.name) AS message_count,
             (SELECT p.id FROM posts p WHERE p.board=c.board AND p.channel=c.name ORDER BY p.timestamp DESC,p.seq DESC LIMIT 1) AS last_message_id
             FROM channels c WHERE c.board=? AND c.name=?",
        )
        .bind(board.identity.to_string())
        .bind(name)
        .fetch_optional(conn)
        .await
        .map_err(storage_error)?
        .ok_or_else(|| invalid("channel not found in this board"))?;
    Ok(ChannelSummary {
        permissions: board.access.map(|access| {
            let write = matches!(access, Access::ReadWrite);
            ChannelPermissions {
                read: true,
                post: write,
                reply: write,
                edit_metadata: false,
            }
        }),
        channel_name: row.get("name"),
        description: row.get("description"),
        created_at: DateTime::parse_from_rfc3339(row.get("created_at"))
            .map_err(storage_error)?
            .with_timezone(&Utc),
        created_by: AgentPath::try_from(row.get::<String, _>("author")).map_err(invalid)?,
        message_count: row.get::<i64, _>("message_count") as usize,
        last_message_id: row
            .get::<Option<String>, _>("last_message_id")
            .map(|id| Uuid::parse_str(&id))
            .transpose()
            .map_err(storage_error)?,
    })
}
