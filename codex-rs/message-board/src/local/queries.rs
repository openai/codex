//! Bounded board queries and channel-description updates that preserve existing evidence.

use super::LocalAgentMessageBoard;
use super::MAX_READ_CHARS;
use super::StoredPost;
use super::catalog;
use super::connection::Access;
use super::invalid;
use super::json_search;
use super::paging::Window;
use super::paging::decode_posts;
use super::paging::direction;
use super::paging::preview;
use super::storage_error;
use super::validate_channel;
use crate::AgentMessageBoard;
use crate::AppendRequest;
use crate::BoardPermissions;
use crate::BoardReadView;
use crate::ChannelPage;
use crate::ChannelQuery;
use crate::ChannelSummary;
use crate::CreateChannelRequest;
use crate::Page;
use crate::PageRequest;
use crate::PostContent;
use crate::PostMetadata;
use crate::PostPreview;
use crate::PostQuery;
use crate::PostRequest;
use crate::ReadPostRequest;
use crate::ReadThreadRequest;
use crate::SubscriptionChange;
use crate::SubscriptionRequest;
use crate::SubscriptionState;
use crate::ThreadPage;
use crate::ThreadQuery;
use crate::ThreadSort;
use crate::ThreadSummary;
use crate::UpsertChannelRequest;
use caseless::default_case_fold_str;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::error::Result;
use futures::future::BoxFuture;
use sqlx::QueryBuilder;
use sqlx::SqliteConnection;
use std::collections::BTreeMap;
use std::sync::Arc;
use uuid::Uuid;

impl LocalAgentMessageBoard {
    pub(super) async fn load_post(
        &self,
        conn: &mut SqliteConnection,
        id: Uuid,
    ) -> Result<StoredPost> {
        let query = if let Some(view) = self.mode.view() {
            sqlx::query_scalar("SELECT payload FROM posts WHERE seq<=? AND board=? AND id=?")
                .bind(view.post_cutoff)
        } else {
            sqlx::query_scalar("SELECT payload FROM posts WHERE board=? AND id=?")
        };
        let payload: Option<String> = query
            .bind(self.identity.to_string())
            .bind(id.to_string())
            .fetch_optional(conn)
            .await
            .map_err(storage_error)?;
        serde_json::from_str(&payload.ok_or_else(|| invalid("post not found in this board"))?)
            .map_err(storage_error)
    }

    pub async fn read_post(
        &self,
        caller: ThreadId,
        request: ReadPostRequest,
    ) -> Result<PostContent> {
        self.host.agent_path(caller).await?;
        let mut conn = self.pool.acquire().await.map_err(storage_error)?;
        let post = self.load_post(&mut conn, request.message_id).await?;
        let n_chars = post.text.chars().count();
        let offset = (request.offset_chars as usize).min(n_chars);
        let text: String = post
            .text
            .chars()
            .skip(offset)
            .take((request.limit_chars.get() as usize).min(MAX_READ_CHARS))
            .collect();
        let next_offset_chars = offset + text.chars().count();
        Ok(PostContent {
            metadata: post.metadata,
            text,
            n_chars,
            next_offset_chars,
        })
    }

    /// Creates a channel or updates its navigation description without replacing posts,
    /// its original author, or its creation time. Only writable connections may call this.
    pub async fn upsert_channel(
        &self,
        caller: ThreadId,
        request: UpsertChannelRequest,
    ) -> Result<()> {
        if let Some(access) = self.mode.access() {
            access.require_write(self.identity, "channel.edit_metadata")?;
        }
        let UpsertChannelRequest {
            channel_name,
            description,
        } = request;
        validate_channel(&channel_name)?;
        let author = self.host.agent_path(caller).await?;
        let mut tx = self.begin_write().await?;
        let updated = sqlx::query("UPDATE channels SET description=? WHERE board=? AND name=?")
            .bind(description.as_str())
            .bind(self.identity.to_string())
            .bind(&channel_name)
            .execute(&mut *tx)
            .await
            .map_err(storage_error)?
            .rows_affected();
        if updated != 0 {
            return tx.commit().await.map_err(storage_error);
        }
        tx.rollback().await.map_err(storage_error)?;
        // Only creation needs a timestamp. Do not hold a write transaction across
        // the host clock call; a concurrent creator is handled by the upsert below.
        let now = self.host.current_time(caller).await?;
        let mut tx = self.begin_write().await?;
        sqlx::query("INSERT INTO channels(board,name,name_search,created_at,timestamp,author,description) VALUES(?,?,?,?,?,?,?) ON CONFLICT(board,name) DO UPDATE SET description=excluded.description")
            .bind(self.identity.to_string()).bind(&channel_name).bind(default_case_fold_str(&channel_name))
            .bind(now.to_rfc3339()).bind(now.timestamp_micros()).bind(author.to_string()).bind(description.as_str())
            .execute(&mut *tx).await.map_err(storage_error)?;
        tx.commit().await.map_err(storage_error)
    }

    /// Lists the caller's channel subscriptions within the existing page budget.
    pub async fn subscribed_channels(
        &self,
        caller: ThreadId,
        page: PageRequest,
    ) -> Result<Page<String>> {
        self.host.agent_path(caller).await?;
        let window = Window::new(&page)?;
        // Probe the existing subscriptions primary key for each channel.
        let channels = sqlx::query_scalar::<_, String>(
            "SELECT c.name FROM channels AS c WHERE c.board=? AND EXISTS (
                SELECT 1 FROM subscriptions AS s
                WHERE s.board=c.board AND s.target=json_object('Channel',c.name) AND s.agent=?
             ) ORDER BY c.name LIMIT ? OFFSET ?",
        )
        .bind(self.identity.to_string())
        .bind(caller.to_string())
        .bind((window.limit + 1) as i64)
        .bind(window.offset())
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(storage_error)?;
        window.finish(channels)
    }
}

impl AgentMessageBoard for LocalAgentMessageBoard {
    fn identity(&self) -> SessionId {
        self.identity
    }

    fn storage_namespace(&self) -> Option<&str> {
        Some(&self.storage_namespace)
    }

    fn read_view(&self, caller: ThreadId) -> BoxFuture<'_, Result<Arc<dyn BoardReadView>>> {
        Box::pin(async move {
            self.host.agent_path(caller).await?;
            catalog::capture(self).await
        })
    }

    fn upsert_channel(
        &self,
        caller: ThreadId,
        request: UpsertChannelRequest,
    ) -> BoxFuture<'_, Result<()>> {
        Box::pin(LocalAgentMessageBoard::upsert_channel(
            self, caller, request,
        ))
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

    fn append(
        &self,
        caller: ThreadId,
        request: AppendRequest,
    ) -> BoxFuture<'_, Result<PostMetadata>> {
        Box::pin(LocalAgentMessageBoard::append(self, caller, request))
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

    fn subscribed_channels(
        &self,
        caller: ThreadId,
        page: PageRequest,
    ) -> BoxFuture<'_, Result<Page<String>>> {
        Box::pin(LocalAgentMessageBoard::subscribed_channels(
            self, caller, page,
        ))
    }

    fn set_subscriptions<'a>(
        &'a self,
        caller: ThreadId,
        channels: &'a BTreeMap<&str, SubscriptionChange>,
    ) -> BoxFuture<'a, Result<()>> {
        Box::pin(self.apply_subscriptions(caller, channels))
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
    ) -> BoxFuture<'_, Result<ChannelPage>> {
        Box::pin(async move {
            self.host.agent_path(caller).await?;
            if let Some(view) = self.mode.view() {
                return view.list_channels(query);
            }
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
                channels.push(catalog::channel_summary(self, &mut tx, &name).await?);
            }
            Ok(ChannelPage {
                channels: window.finish(channels)?,
                board_permissions: self.mode.access().map(|access| BoardPermissions {
                    create_channel: matches!(access, Access::ReadWrite),
                    edit_metadata: false,
                    manage_permissions: false,
                }),
            })
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
            if !exists
                || self.mode.view().is_some_and(|view| {
                    !view
                        .channels
                        .iter()
                        .any(|channel| channel.channel_name == query.channel_name)
                })
            {
                return Err(invalid("channel not found in this board"));
            }
            let window = Window::new(&query.page)?;
            let order = direction(query.direction);
            // Select the page before looking up reply summaries, especially when
            // activity sorting examines more roots than the page will return.
            let mut sql = QueryBuilder::new("WITH ");
            let (roots, by_time, for_count) = if let Some(view) = self.mode.view() {
                // A sequence cutoff can make SQLite scan and sort a whole channel or
                // thread. Keep the ordered indexes for roots and last activity, and
                // allow the reply count to use the sequence range.
                sql.push("visible_roots AS NOT MATERIALIZED (SELECT * FROM posts INDEXED BY posts_roots_created WHERE id=root AND seq<=")
                    .push_bind(view.post_cutoff)
                    .push("), visible_by_time AS NOT MATERIALIZED (SELECT * FROM posts INDEXED BY posts_board_root_timestamp WHERE seq<=")
                    .push_bind(view.post_cutoff)
                    .push("), visible_posts AS NOT MATERIALIZED (SELECT * FROM posts WHERE seq<=")
                    .push_bind(view.post_cutoff)
                    .push("), ");
                ("visible_roots", "visible_by_time", "visible_posts")
            } else {
                ("posts", "posts", "posts")
            };
            sql.push("page AS MATERIALIZED (SELECT p.board,p.id,p.payload,p.seq,");
            match query.sort {
                ThreadSort::Created => {
                    sql.push("p.timestamp");
                }
                ThreadSort::Activity => {
                    sql.push("(SELECT MAX(r.timestamp) FROM ")
                        .push(by_time)
                        .push(" r WHERE r.board=p.board AND r.root=p.id)");
                }
            }
            sql.push(" AS sort_timestamp FROM ")
                .push(roots)
                .push(" p WHERE p.board=")
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
                     (SELECT COUNT(*) FROM ",
                )
                .push(for_count)
                .push(
                    " r WHERE r.board=p.board AND r.root=p.id AND r.id<>r.root),
                     (SELECT r.payload FROM ",
                )
                .push(by_time)
                .push(
                    " r WHERE r.board=p.board AND r.root=p.id AND r.id<>r.root
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
                let lookup = if let Some(view) = self.mode.view() {
                    sqlx::query_as::<_, (i64, i64)>(
                        "SELECT timestamp,seq FROM posts WHERE seq<=? AND board=? AND id=?",
                    )
                    .bind(view.post_cutoff)
                } else {
                    sqlx::query_as("SELECT timestamp,seq FROM posts WHERE board=? AND id=?")
                };
                Some(
                    lookup
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
            // For a frozen channel page, scan in result order. Otherwise SQLite can
            // choose the sequence index for the cutoff and sort the entire channel.
            let select = if self.mode.view().is_some() && query.channel_name.is_some() {
                "SELECT payload FROM posts INDEXED BY posts_board_channel_timestamp WHERE board="
            } else {
                "SELECT payload FROM posts WHERE board="
            };
            let mut sql = QueryBuilder::new(select);
            sql.push_bind(self.identity.to_string());
            if let Some(view) = self.mode.view() {
                sql.push(" AND seq<=").push_bind(view.post_cutoff);
            }
            if let Some(channel) = query.channel_name {
                sql.push(" AND channel=").push_bind(channel);
            }
            if let Some(author) = query.author {
                sql.push(" AND author=").push_bind(author.to_string());
            }
            if let Some((timestamp, seq)) = after {
                sql.push(" AND (timestamp,seq)>(")
                    .push_bind(timestamp)
                    .push(",")
                    .push_bind(seq)
                    .push(")");
            }
            let needle = query.query.as_deref().map(default_case_fold_str);
            let posts = if let (Some(pointer), Some(needle)) = (
                self.nested_json_pointer,
                needle.as_deref().filter(|text| !text.is_empty()),
            ) {
                json_search::page(sql, &mut tx, pointer, needle, &window).await?
            } else {
                if let Some(needle) = needle {
                    sql.push(" AND instr(body_search,")
                        .push_bind(needle)
                        .push(")>0");
                }
                sql.push(" ORDER BY timestamp DESC,seq DESC LIMIT ")
                    .push_bind((window.limit + 1) as i64)
                    .push(" OFFSET ")
                    .push_bind(window.offset());
                decode_posts(
                    sql.build_query_scalar::<String>()
                        .fetch_all(&mut *tx)
                        .await
                        .map_err(storage_error)?,
                )?
            };
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
            let replies = if let Some(view) = self.mode.view() {
                sqlx::query_scalar(
                    "SELECT payload FROM posts INDEXED BY posts_board_root_timestamp
                     WHERE seq<=? AND board=? AND root=? AND id<>root
                     ORDER BY timestamp DESC,seq DESC LIMIT ? OFFSET ?",
                )
                .bind(view.post_cutoff)
            } else {
                sqlx::query_scalar(
                    "SELECT payload FROM posts WHERE board=? AND root=? AND id<>root
                     ORDER BY timestamp DESC,seq DESC LIMIT ? OFFSET ?",
                )
            };
            let posts = decode_posts(
                replies
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
