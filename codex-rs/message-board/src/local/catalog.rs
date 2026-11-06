//! Channel summaries and fixed read views. Capture only catalog metadata and a
//! monotonically increasing post boundary; payload reads continue to use SQLite.

use super::BoardMode;
use super::LocalAgentMessageBoard;
use super::connection::Access;
use super::invalid;
use super::options::LocalBoardOptions;
use super::paging::Window;
use super::storage_error;
use crate::AgentMessageBoard;
use crate::BoardPermissions;
use crate::ChannelPage;
use crate::ChannelPermissions;
use crate::ChannelQuery;
use crate::ChannelSummary;
use crate::SortDirection;
use caseless::default_case_fold_str;
use chrono::DateTime;
use chrono::Utc;
use codex_protocol::AgentPath;
use codex_protocol::error::Result;
use sqlx::Row;
use sqlx::SqliteConnection;
use sqlx::sqlite::SqliteRow;
use std::sync::Arc;
use uuid::Uuid;

/// An owned, fixed read boundary. It does not retain deleted evidence or hold a
/// transaction open. Clones share the catalog, cutoff and underlying connection pool.
#[derive(Clone)]
pub struct LocalBoardReadView {
    board: Arc<LocalAgentMessageBoard>,
    view: Arc<ReadView>,
}

impl LocalBoardReadView {
    /// Reads use the captured boundary; every mutation, including subscriptions, is denied.
    pub fn board(&self) -> Arc<dyn AgentMessageBoard> {
        self.board.clone()
    }

    /// Reads this capture with literal search over JSON posts and a string-encoded JSON field.
    /// The JSON Pointer identifies the field to decode.
    pub fn board_with_json_search(&self, pointer: &'static str) -> Arc<dyn AgentMessageBoard> {
        let mut board = self.board.as_ref().clone();
        board.nested_json_pointer = Some(pointer);
        Arc::new(board)
    }

    /// Whether the captured database belongs to the caller's requested storage namespace.
    pub fn uses_storage_namespace(&self, options: &LocalBoardOptions) -> Result<bool> {
        self.board.uses_storage_namespace(options)
    }

    /// Complete catalog captured with the post boundary, ordered by newest activity first.
    pub fn channels(&self) -> &[ChannelSummary] {
        &self.view.channels
    }
}

pub(super) struct ReadView {
    pub(super) post_cutoff: i64,
    pub(super) channels: Vec<ChannelSummary>,
}

impl ReadView {
    pub(super) fn list_channels(&self, query: ChannelQuery) -> Result<ChannelPage> {
        let window = Window::new(&query.page)?;
        let needle = default_case_fold_str(&query.query.unwrap_or_default());
        let ordered: Box<dyn Iterator<Item = &ChannelSummary>> = match query.direction {
            SortDirection::NewestFirst => Box::new(self.channels.iter()),
            SortDirection::OldestFirst => Box::new(self.channels.iter().rev()),
        };
        let channels = ordered
            .filter(|channel| default_case_fold_str(&channel.channel_name).contains(&needle))
            .skip(window.offset() as usize)
            .take(window.limit + 1)
            .cloned()
            .collect();
        Ok(ChannelPage {
            channels: window.finish(channels)?,
            board_permissions: Some(BoardPermissions {
                create_channel: false,
                edit_metadata: false,
                manage_permissions: false,
            }),
        })
    }
}

pub(super) async fn capture(board: &LocalAgentMessageBoard) -> Result<LocalBoardReadView> {
    let mut tx = board.pool.begin().await.map_err(storage_error)?;
    // seq is the database-wide AUTOINCREMENT key. Every future board append is above it.
    let post_cutoff = sqlx::query_scalar::<_, i64>("SELECT COALESCE(MAX(seq),0) FROM posts")
        .fetch_one(&mut *tx)
        .await
        .map_err(storage_error)?;
    let rows = sqlx::query(
        "SELECT c.name,c.created_at,c.author,c.description,last.id AS last_message_id,
                (SELECT COUNT(*) FROM posts p WHERE p.board=c.board AND p.channel=c.name) AS message_count
         FROM channels c LEFT JOIN posts last ON last.seq=(
             SELECT p.seq FROM posts p WHERE p.board=c.board AND p.channel=c.name
             ORDER BY p.timestamp DESC,p.seq DESC LIMIT 1
         )
         WHERE c.board=? ORDER BY COALESCE(last.timestamp,c.timestamp) DESC,c.name DESC",
    )
    .bind(board.identity.to_string())
    .fetch_all(&mut *tx)
    .await
    .map_err(storage_error)?;
    tx.commit().await.map_err(storage_error)?;
    let channels = rows
        .into_iter()
        .map(|row| decode_channel_summary(Some(BoardMode::FIXED_ACCESS), row))
        .collect::<Result<Vec<_>>>()?;
    let view = Arc::new(ReadView {
        post_cutoff,
        channels,
    });
    let board = LocalAgentMessageBoard {
        mode: BoardMode::Fixed {
            view: Arc::clone(&view),
        },
        ..board.clone()
    };
    Ok(LocalBoardReadView {
        board: Arc::new(board),
        view,
    })
}

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
    decode_channel_summary(board.mode.access(), row)
}

fn decode_channel_summary(access: Option<Access>, row: SqliteRow) -> Result<ChannelSummary> {
    Ok(ChannelSummary {
        permissions: access.map(|access| {
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
