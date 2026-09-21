//! Implements the board contract using bounded SQLite queries with ordinary offset pagination.

use super::LocalAgentMessageBoard;
use super::MAX_READ_CHARS;
use super::StoredPost;
use super::invalid;
use super::paging::Window;
use super::paging::decode_posts;
use super::paging::direction;
use super::paging::preview;
use super::storage_error;
use crate::AgentMessageBoard;
use crate::ChannelQuery;
use crate::ChannelSummary;
use crate::CreateChannelRequest;
use crate::Page;
use crate::PostContent;
use crate::PostMetadata;
use crate::PostPreview;
use crate::PostQuery;
use crate::PostRequest;
use crate::ReadPostRequest;
use crate::ReadThreadRequest;
use crate::SubscriptionRequest;
use crate::SubscriptionState;
use crate::ThreadPage;
use crate::ThreadQuery;
use crate::ThreadSort;
use crate::ThreadSummary;
use caseless::default_case_fold_str;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::error::Result;
use futures::future::BoxFuture;
use sqlx::QueryBuilder;

impl AgentMessageBoard for LocalAgentMessageBoard {
    fn identity(&self) -> SessionId {
        self.identity
    }

    fn create_channel(
        &self,
        caller: ThreadId,
        request: CreateChannelRequest,
    ) -> BoxFuture<'_, Result<ChannelSummary>> {
        Box::pin(LocalAgentMessageBoard::create_channel(
            self, caller, request,
        ))
    }

    fn post(&self, caller: ThreadId, request: PostRequest) -> BoxFuture<'_, Result<PostMetadata>> {
        Box::pin(LocalAgentMessageBoard::post(self, caller, request))
    }

    fn set_subscription(
        &self,
        caller: ThreadId,
        request: SubscriptionRequest,
    ) -> BoxFuture<'_, Result<SubscriptionState>> {
        Box::pin(LocalAgentMessageBoard::set_subscription(
            self, caller, request,
        ))
    }

    fn read_post(
        &self,
        caller: ThreadId,
        request: ReadPostRequest,
    ) -> BoxFuture<'_, Result<PostContent>> {
        Box::pin(LocalAgentMessageBoard::read_post(self, caller, request))
    }

    fn list_channels(
        &self,
        caller: ThreadId,
        query: ChannelQuery,
    ) -> BoxFuture<'_, Result<Page<ChannelSummary>>> {
        Box::pin(async move {
            self.host.agent_path(caller).await?;
            let mut tx = self.pool.begin().await.map_err(storage_error)?;
            let window = Window::new(&query.page)?;
            let order = direction(query.direction);
            let mut sql = QueryBuilder::new("SELECT c.name FROM channels c WHERE c.board=");
            sql.push_bind(self.identity.to_string())
                .push(" AND instr(c.name_search,")
                .push_bind(default_case_fold_str(&query.query.unwrap_or_default()))
                .push(
                    ")>0 ORDER BY COALESCE(
                        (SELECT MAX(p.timestamp) FROM posts p WHERE p.board=c.board AND p.channel=c.name),
                        c.timestamp) ",
                )
                .push(order)
                .push(",c.name ")
                .push(order)
                .push(" LIMIT ")
                .push_bind((window.limit + 1) as i64)
                .push(" OFFSET ")
                .push_bind(window.offset());
            let names = sql
                .build_query_scalar::<String>()
                .fetch_all(&mut *tx)
                .await
                .map_err(storage_error)?;
            let mut channels = Vec::with_capacity(names.len());
            for name in names {
                channels.push(self.channel_summary(&mut tx, &name).await?);
            }
            window.finish(channels)
        })
    }

    fn list_threads(
        &self,
        caller: ThreadId,
        query: ThreadQuery,
    ) -> BoxFuture<'_, Result<Page<ThreadSummary>>> {
        Box::pin(async move {
            self.host.agent_path(caller).await?;
            let mut tx = self.pool.begin().await.map_err(storage_error)?;
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM channels WHERE board=? AND name=?)",
            )
            .bind(self.identity.to_string())
            .bind(&query.channel_name)
            .fetch_one(&mut *tx)
            .await
            .map_err(storage_error)?;
            if !exists {
                return Err(invalid("channel not found in this board"));
            }
            let window = Window::new(&query.page)?;
            let order = direction(query.direction);
            // Select the page before looking up reply summaries, especially when
            // activity sorting examines more roots than the page will return.
            let mut sql = QueryBuilder::new(
                "WITH page AS MATERIALIZED (SELECT p.board,p.id,p.payload,p.seq,",
            );
            match query.sort {
                ThreadSort::Created => {
                    sql.push("p.timestamp");
                }
                ThreadSort::Activity => {
                    sql.push("(SELECT MAX(r.timestamp) FROM posts r WHERE r.board=p.board AND r.root=p.id)");
                }
            }
            sql.push(" AS sort_timestamp FROM posts p WHERE p.board=")
                .push_bind(self.identity.to_string())
                .push(" AND p.channel=")
                .push_bind(query.channel_name)
                .push(" AND p.id=p.root ORDER BY sort_timestamp ")
                .push(order)
                .push(",p.seq ")
                .push(order)
                .push(" LIMIT ")
                .push_bind((window.limit + 1) as i64)
                .push(" OFFSET ")
                .push_bind(window.offset())
                .push(
                    ") SELECT p.payload,
                     (SELECT COUNT(*) FROM posts r WHERE r.board=p.board AND r.root=p.id AND r.id<>r.root),
                     (SELECT r.payload FROM posts r WHERE r.board=p.board AND r.root=p.id AND r.id<>r.root
                      ORDER BY r.timestamp DESC,r.seq DESC LIMIT 1)
                     FROM page p ORDER BY p.sort_timestamp ",
                )
                .push(order)
                .push(",p.seq ")
                .push(order);
            let rows = sql
                .build_query_as::<(String, i64, Option<String>)>()
                .fetch_all(&mut *tx)
                .await
                .map_err(storage_error)?;
            let chars = (query.max_chars_per_post.get() as usize)
                .min(MAX_READ_CHARS / (2 * rows.len().min(window.limit).max(1)));
            let mut threads = Vec::with_capacity(rows.len());
            for (root, count, last) in rows {
                let root: StoredPost = serde_json::from_str(&root).map_err(storage_error)?;
                let id = root.metadata.message_id;
                let last: Option<StoredPost> = last
                    .map(|value| serde_json::from_str(&value))
                    .transpose()
                    .map_err(storage_error)?;
                let activity = last.as_ref().map_or(root.metadata.created_at, |last| {
                    last.metadata.created_at.max(root.metadata.created_at)
                });
                threads.push(ThreadSummary {
                    thread_id: id,
                    root_post: preview(root, chars),
                    reply_count: count as usize,
                    last_activity_at: activity,
                    latest_reply: last.map(|post| preview(post, chars)),
                });
            }
            window.finish(threads)
        })
    }

    fn search_posts(
        &self,
        caller: ThreadId,
        query: PostQuery,
    ) -> BoxFuture<'_, Result<Page<PostPreview>>> {
        Box::pin(async move {
            self.host.agent_path(caller).await?;
            let mut tx = self.pool.begin().await.map_err(storage_error)?;
            let window = Window::new(&query.page)?;
            let after = if let Some(id) = query.after_message_id {
                Some(
                    sqlx::query_as::<_, (i64, i64)>(
                        "SELECT timestamp,seq FROM posts WHERE board=? AND id=?",
                    )
                    .bind(self.identity.to_string())
                    .bind(id.to_string())
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(storage_error)?
                    .ok_or_else(|| invalid("post not found in this board"))?,
                )
            } else {
                None
            };
            let mut sql = QueryBuilder::new("SELECT payload FROM posts WHERE board=");
            sql.push_bind(self.identity.to_string());
            if let Some(channel) = query.channel_name {
                sql.push(" AND channel=").push_bind(channel);
            }
            if let Some(author) = query.author {
                sql.push(" AND author=").push_bind(author.to_string());
            }
            if let Some(text) = query.query {
                sql.push(" AND instr(body_search,")
                    .push_bind(default_case_fold_str(&text))
                    .push(")>0");
            }
            if let Some((timestamp, seq)) = after {
                sql.push(" AND (timestamp,seq)>(")
                    .push_bind(timestamp)
                    .push(",")
                    .push_bind(seq)
                    .push(")");
            }
            sql.push(" ORDER BY timestamp DESC,seq DESC LIMIT ")
                .push_bind((window.limit + 1) as i64)
                .push(" OFFSET ")
                .push_bind(window.offset());
            let posts = decode_posts(
                sql.build_query_scalar::<String>()
                    .fetch_all(&mut *tx)
                    .await
                    .map_err(storage_error)?,
            )?;
            let chars = (query.max_chars_per_post.get() as usize)
                .min(MAX_READ_CHARS / posts.len().min(window.limit).max(1));
            window.finish(posts.into_iter().map(|post| preview(post, chars)).collect())
        })
    }

    fn read_thread(
        &self,
        caller: ThreadId,
        request: ReadThreadRequest,
    ) -> BoxFuture<'_, Result<ThreadPage>> {
        Box::pin(async move {
            self.host.agent_path(caller).await?;
            let mut tx = self.pool.begin().await.map_err(storage_error)?;
            let root = self.load_post(&mut tx, request.thread_id).await?;
            if root.metadata.thread_id != request.thread_id {
                return Err(invalid("thread_id must identify a top-level post"));
            }
            let window = Window::new(&request.page)?;
            let posts = decode_posts(
                sqlx::query_scalar(
                    "SELECT payload FROM posts WHERE board=? AND root=? AND id<>root
                     ORDER BY timestamp DESC,seq DESC LIMIT ? OFFSET ?",
                )
                .bind(self.identity.to_string())
                .bind(request.thread_id.to_string())
                .bind((window.limit + 1) as i64)
                .bind(window.offset())
                .fetch_all(&mut *tx)
                .await
                .map_err(storage_error)?,
            )?;
            let chars = (request.max_chars_per_post.get() as usize)
                .min(MAX_READ_CHARS / (posts.len().min(window.limit) + 1));
            Ok(ThreadPage {
                root_post: preview(root, chars),
                replies: window
                    .finish(posts.into_iter().map(|post| preview(post, chars)).collect())?,
            })
        })
    }
}
