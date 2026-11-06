//! Channel summaries and fixed read views. Capture only catalog metadata and a
//! monotonically increasing post boundary; payload reads continue to use SQLite.

use super::BoardMode;
use super::LocalAgentMessageBoard;
use super::connection::Access;
use super::invalid;
use super::paging::Window;
use super::storage_error;
use crate::AgentMessageBoard;
use crate::BoardPermissions;
use crate::BoardReadView;
use crate::ChannelPage;
use crate::ChannelPermissions;
use crate::ChannelQuery;
use crate::ChannelSummary;
use crate::Page;
use crate::PostContent;
use crate::PostPreview;
use crate::PostQuery;
use crate::ReadPostRequest;
use crate::ReadThreadRequest;
use crate::SortDirection;
use crate::ThreadPage;
use crate::ThreadQuery;
use crate::ThreadSummary;
use caseless::default_case_fold_str;
use chrono::DateTime;
use chrono::Utc;
use codex_protocol::AgentPath;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::error::Result;
use futures::future::BoxFuture;
use sqlx::Row;
use sqlx::SqliteConnection;
use sqlx::sqlite::SqliteRow;
use std::sync::Arc;
use uuid::Uuid;

/// The captured catalog and the read-only SQLite handle that enforces its post boundary.
struct LocalBoardReadView {
    board: LocalAgentMessageBoard,
    view: Arc<ReadView>,
}

impl BoardReadView for LocalBoardReadView {
    fn identity(&self) -> SessionId {
        self.board.identity
    }

    fn with_json_search(&self, pointer: &'static str) -> Arc<dyn BoardReadView> {
        let mut board = self.board.clone();
        board.nested_json_pointer = Some(pointer);
        Arc::new(Self {
            board,
            view: Arc::clone(&self.view),
        })
    }

    fn storage_namespace(&self) -> &str {
        &self.board.storage_namespace
    }

    fn channels(&self) -> &[ChannelSummary] {
        &self.view.channels
    }

    fn list_channels(
        &self,
        caller: ThreadId,
        query: ChannelQuery,
    ) -> BoxFuture<'_, Result<ChannelPage>> {
        self.board.list_channels(caller, query)
    }

    fn list_threads(
        &self,
        caller: ThreadId,
        query: ThreadQuery,
    ) -> BoxFuture<'_, Result<Page<ThreadSummary>>> {
        self.board.list_threads(caller, query)
    }

    fn search_posts(
        &self,
        caller: ThreadId,
        query: PostQuery,
    ) -> BoxFuture<'_, Result<Page<PostPreview>>> {
        self.board.search_posts(caller, query)
    }

    fn read_thread(
        &self,
        caller: ThreadId,
        request: ReadThreadRequest,
    ) -> BoxFuture<'_, Result<ThreadPage>> {
        self.board.read_thread(caller, request)
    }

    fn read_post(
        &self,
        caller: ThreadId,
        request: ReadPostRequest,
    ) -> BoxFuture<'_, Result<PostContent>> {
        AgentMessageBoard::read_post(&self.board, caller, request)
    }

    fn appended_request_ids<'a>(
        &'a self,
        caller: ThreadId,
        channel: &'a str,
        after: i64,
    ) -> BoxFuture<'a, Result<Vec<(i64, String)>>> {
        Box::pin(async move {
            self.board.host.agent_path(caller).await?;
            if after < 0 || after > self.view.post_cutoff {
                return Err(invalid("post cursor is outside the captured board view"));
            }
            // The persisted retry key prefixes the caller-supplied request ID with "{caller}:".
            sqlx::query_as(
                "SELECT seq, substr(request_id, instr(request_id, ':') + 1) FROM posts
                 WHERE board=? AND channel=? AND seq>? AND seq<=? ORDER BY seq LIMIT ?",
            )
            .bind(self.board.identity.to_string())
            .bind(channel)
            .bind(after)
            .bind(self.view.post_cutoff)
            .bind(i64::from(crate::PageRequest::default().limit.get()))
            .fetch_all(self.board.pool.as_ref())
            .await
            .map_err(storage_error)
        })
    }

    fn read_post_texts<'a>(
        &'a self,
        caller: ThreadId,
        positions: &'a [i64],
    ) -> BoxFuture<'a, Result<Vec<String>>> {
        Box::pin(async move {
            self.board.host.agent_path(caller).await?;
            if positions.len() > crate::PageRequest::default().limit.get() as usize {
                return Err(invalid("requested posts exceed one board page"));
            }
            // Drive primary-key lookups from the requested page, not from all posts on the board.
            let texts = sqlx::query_scalar::<_, String>(
                "SELECT json_extract(p.payload, '$.text') FROM json_each(?) AS requested
                 CROSS JOIN posts AS p
                 WHERE p.seq=requested.value AND p.board=? AND p.seq<=? ORDER BY requested.key",
            )
            .bind(serde_json::to_string(positions).map_err(storage_error)?)
            .bind(self.board.identity.to_string())
            .bind(self.view.post_cutoff)
            .fetch_all(self.board.pool.as_ref())
            .await
            .map_err(storage_error)?;
            if texts.len() != positions.len() {
                return Err(invalid("post not found in this board view"));
            }
            Ok(texts)
        })
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

pub(super) async fn capture(board: &LocalAgentMessageBoard) -> Result<Arc<dyn BoardReadView>> {
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
    Ok(Arc::new(LocalBoardReadView { board, view }))
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
