//! SQLite-backed boards. The tree ID scopes every read and write.
//!
//! Immediate transactions serialize mutations across independently opened handles.
//! Live handles in this process share one connection pool per database path.
//! Accepted posts survive runtime unload and process restart, but cannot recreate
//! a board after its root has been permanently deleted.

use crate::ChannelSummary;
use crate::CreateChannelRequest;
use crate::MessageBoardHost;
use crate::PostContent;
use crate::PostDestination;
use crate::PostMetadata;
use crate::PostRequest;
use crate::ReadPostRequest;
use crate::SubscriptionChange;
use crate::SubscriptionRequest;
use crate::SubscriptionState;
use crate::SubscriptionTarget;
use caseless::default_case_fold_str;
use chrono::DateTime;
use chrono::Utc;
use codex_protocol::AgentPath;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;
use codex_state::SqliteConfig;
use futures::StreamExt;
use serde::Deserialize;
use serde::Serialize;
use sqlx::Row;
use sqlx::SqliteConnection;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Weak;
use tokio::sync::Mutex;
use uuid::Uuid;

mod lifecycle;
mod paging;
mod queries;

#[cfg(test)]
#[path = "local/pools_tests.rs"]
mod pools_tests;

const MAX_POST_BYTES: usize = 64 * 1024;
const MAX_CHANNEL_BYTES: usize = 128;
const MAX_READ_CHARS: usize = 20_000;
const DATABASE_FILE: &str = "agent_message_board_1.sqlite";

// Weak entries let the last board handle release its pool. Initialization and
// recovery share one lock so concurrent starts cannot open duplicate or stale pools.
static POOLS: LazyLock<Mutex<HashMap<PathBuf, Weak<SqlitePool>>>> = LazyLock::new(Mutex::default);

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS deleted_boards (board TEXT PRIMARY KEY NOT NULL);
CREATE TABLE IF NOT EXISTS channels (
 board TEXT NOT NULL, name TEXT NOT NULL, name_search TEXT NOT NULL, created_at TEXT NOT NULL, timestamp INTEGER NOT NULL, author TEXT NOT NULL,
 PRIMARY KEY(board,name)
);
CREATE TABLE IF NOT EXISTS posts (
 seq INTEGER PRIMARY KEY AUTOINCREMENT,
 board TEXT NOT NULL, id TEXT NOT NULL, channel TEXT NOT NULL, root TEXT NOT NULL,
 author TEXT NOT NULL, timestamp INTEGER NOT NULL, body_search TEXT NOT NULL,
 payload TEXT NOT NULL, request_id TEXT NOT NULL, request TEXT NOT NULL,
 UNIQUE(board,id), UNIQUE(board,request_id)
);
CREATE INDEX IF NOT EXISTS posts_board_channel ON posts(board,channel,seq);
CREATE INDEX IF NOT EXISTS posts_board_channel_timestamp ON posts(board,channel,timestamp,seq);
CREATE INDEX IF NOT EXISTS posts_roots_created ON posts(board,channel,timestamp,seq) WHERE id=root;
CREATE INDEX IF NOT EXISTS posts_board_root ON posts(board,root,seq);
CREATE INDEX IF NOT EXISTS posts_board_root_timestamp ON posts(board,root,timestamp,seq);
CREATE INDEX IF NOT EXISTS posts_board_timestamp ON posts(board,timestamp,seq);
CREATE TABLE IF NOT EXISTS subscriptions (
 board TEXT NOT NULL, target TEXT NOT NULL, agent TEXT NOT NULL,
 PRIMARY KEY(board,target,agent)
);";

#[derive(Clone)]
pub struct LocalAgentMessageBoard {
    identity: SessionId,
    pool: Arc<SqlitePool>,
    host: Arc<dyn MessageBoardHost>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StoredPost {
    metadata: PostMetadata,
    text: String,
}

impl LocalAgentMessageBoard {
    /// Reopens the same board for a root, child or resumed runtime. The shared
    /// SQLite configuration preserves the host's connection and journal policy.
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "pool creation and schema initialization stay serialized to avoid duplicate pools"
    )]
    pub async fn open(
        sqlite: &SqliteConfig,
        identity: SessionId,
        host: Arc<dyn MessageBoardHost>,
    ) -> Result<Self> {
        tokio::fs::create_dir_all(sqlite.home()).await?;
        let path = tokio::fs::canonicalize(sqlite.home())
            .await?
            .join(DATABASE_FILE);
        let mut pools = POOLS.lock().await;
        pools.retain(|_, pool| pool.strong_count() > 0);
        let pool = if let Some(pool) = pools
            .get(&path)
            .and_then(Weak::upgrade)
            .filter(|pool| !pool.is_closed())
        {
            pool
        } else {
            let pool = sqlite
                .open_read_write_pool(&path)
                .await
                .map_err(storage_error)?;
            sqlx::raw_sql(SCHEMA)
                .execute(&pool)
                .await
                .map_err(storage_error)?;
            let pool = Arc::new(pool);
            pools.insert(path, Arc::downgrade(&pool));
            pool
        };
        Ok(Self {
            identity,
            pool,
            host,
        })
    }

    pub async fn create_channel(
        &self,
        caller: ThreadId,
        request: CreateChannelRequest,
    ) -> Result<ChannelSummary> {
        validate_channel(&request.channel_name)?;
        let author = self.host.agent_path(caller).await?;
        let now = self.host.current_time(caller).await?;
        let mut tx = self.begin_write().await?;
        self.insert_channel(&mut tx, &request.channel_name, &author, now)
            .await?;
        if request.subscription == SubscriptionChange::Subscribe {
            self.subscribe(
                &mut tx,
                &SubscriptionTarget::Channel(request.channel_name.clone()),
                caller,
            )
            .await?;
        }
        let summary = self.channel_summary(&mut tx, &request.channel_name).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(summary)
    }

    /// Once started, finish the accepted write and fanout even if the tool caller
    /// disconnects. Delivery is attempted once; retries never replay old notices
    /// into a recipient's later turn. Once committed, notification failures are
    /// logged without failing the post; the persisted content remains readable.
    pub async fn post(&self, caller: ThreadId, request: PostRequest) -> Result<PostMetadata> {
        let board = self.clone();
        tokio::spawn(async move { board.post_inner(caller, request).await })
            .await
            .map_err(storage_error)?
    }

    async fn post_inner(&self, caller: ThreadId, request: PostRequest) -> Result<PostMetadata> {
        if request.text.len() > MAX_POST_BYTES
            || request.text.is_empty()
            || request.request_id.is_empty()
            || request.request_id.len() > 512
            || request.agents_to_notify.len() > 256
        {
            return Err(invalid(
                "post text, request ID or recipient count exceeds the board limits",
            ));
        }
        let author = self.host.agent_path(caller).await?;
        let request_id = format!("{caller}:{}", request.request_id);
        let request_json = serde_json::to_string(&request).map_err(storage_error)?;
        let mut conn = self.pool.acquire().await.map_err(storage_error)?;
        if let Some(post) = self
            .existing_post(&mut conn, &request_id, &request_json)
            .await?
        {
            return Ok(post.metadata);
        }
        drop(conn);
        let mut recipients = HashSet::new();
        for path in &request.agents_to_notify {
            recipients.insert(self.host.resolve_agent(path.clone()).await?);
        }
        let now = self.host.current_time(caller).await?;
        let mut tx = self.begin_write().await?;
        if let Some(post) = self
            .existing_post(&mut tx, &request_id, &request_json)
            .await?
        {
            return Ok(post.metadata);
        }
        let id = Uuid::now_v7();
        let (channel, root, target) = match &request.destination {
            PostDestination::Channel(channel) => {
                let exists: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM channels WHERE board=? AND name=?)",
                )
                .bind(self.identity.to_string())
                .bind(channel)
                .fetch_one(&mut *tx)
                .await
                .map_err(storage_error)?;
                if !exists {
                    return Err(invalid("channel not found in this board"));
                }
                (
                    channel.clone(),
                    id,
                    SubscriptionTarget::Channel(channel.clone()),
                )
            }
            PostDestination::NewChannel(channel) => {
                validate_channel(channel)?;
                self.insert_channel(&mut tx, channel, &author, now).await?;
                (
                    channel.clone(),
                    id,
                    SubscriptionTarget::Channel(channel.clone()),
                )
            }
            PostDestination::Thread(root) => {
                let post = self.load_post(&mut tx, *root).await?;
                if post.metadata.thread_id != *root {
                    return Err(invalid("thread_id must identify a top-level post"));
                }
                (
                    post.metadata.channel_name,
                    *root,
                    SubscriptionTarget::Thread(*root),
                )
            }
        };
        // Return one JSON array to avoid handing each subscriber row across
        // SQLite's worker thread while the write transaction is held.
        let subscribed = sqlx::query_scalar::<_, String>(
            "SELECT json_group_array(agent) FROM subscriptions WHERE board=? AND target=?",
        )
        .bind(self.identity.to_string())
        .bind(target_key(&target)?)
        .fetch_one(&mut *tx)
        .await
        .map_err(storage_error)?;
        let subscribed: Vec<String> = serde_json::from_str(&subscribed).map_err(storage_error)?;
        for recipient in subscribed {
            recipients.insert(ThreadId::from_string(&recipient).map_err(storage_error)?);
        }
        let post = StoredPost {
            metadata: PostMetadata {
                message_id: id,
                channel_name: channel.clone(),
                author: author.clone(),
                thread_id: root,
                created_at: now,
            },
            text: request.text.clone(),
        };
        sqlx::query("INSERT INTO posts(board,id,channel,root,author,timestamp,body_search,payload,request_id,request) VALUES(?,?,?,?,?,?,?,?,?,?)")
            .bind(self.identity.to_string()).bind(id.to_string()).bind(channel).bind(root.to_string())
            .bind(author.to_string()).bind(now.timestamp_micros()).bind(default_case_fold_str(&request.text))
            .bind(serde_json::to_string(&post).map_err(storage_error)?).bind(request_id).bind(request_json)
            .execute(&mut *tx).await.map_err(storage_error)?;
        self.subscribe(&mut tx, &SubscriptionTarget::Thread(root), caller)
            .await?;
        tx.commit().await.map_err(storage_error)?;

        // A committed post succeeds even if a best-effort notice cannot be delivered.
        let notice = paging::preview(post.clone(), /*max_chars*/ 150);
        futures::stream::iter(recipients)
            .for_each_concurrent(/*limit*/ 16, |recipient| {
                let notice = notice.clone();
                async move {
                    if let Err(error) = self.host.notify(recipient, notice).await {
                        tracing::warn!(%recipient, %error, "Failed to deliver message-board notification");
                    }
                }
            })
            .await;
        Ok(post.metadata)
    }

    pub async fn set_subscription(
        &self,
        caller: ThreadId,
        request: SubscriptionRequest,
    ) -> Result<SubscriptionState> {
        let caller_path = self.host.agent_path(caller).await?;
        let target_path = request.target_agent.unwrap_or(caller_path);
        let target_agent = self.host.resolve_agent(target_path.clone()).await?;
        let mut tx = self.begin_write().await?;
        let (channel, root, last) = match &request.target {
            SubscriptionTarget::Channel(name) => {
                let summary = self.channel_summary(&mut tx, name).await?;
                (name.clone(), None, summary.last_message_id)
            }
            SubscriptionTarget::Thread(root) => {
                let post = self.load_post(&mut tx, *root).await?;
                if post.metadata.thread_id != *root {
                    return Err(invalid("thread_id must identify a top-level post"));
                }
                let last: String = sqlx::query_scalar("SELECT id FROM posts WHERE board=? AND root=? ORDER BY timestamp DESC, seq DESC LIMIT 1")
                    .bind(self.identity.to_string()).bind(root.to_string()).fetch_one(&mut *tx).await.map_err(storage_error)?;
                (
                    post.metadata.channel_name,
                    Some(*root),
                    Some(Uuid::parse_str(&last).map_err(storage_error)?),
                )
            }
        };
        let enabled = request.change == SubscriptionChange::Subscribe;
        match request.change {
            SubscriptionChange::Subscribe => {
                self.subscribe(&mut tx, &request.target, target_agent)
                    .await?
            }
            SubscriptionChange::Unsubscribe => {
                sqlx::query("DELETE FROM subscriptions WHERE board=? AND target=? AND agent=?")
                    .bind(self.identity.to_string())
                    .bind(target_key(&request.target)?)
                    .bind(target_agent.to_string())
                    .execute(&mut *tx)
                    .await
                    .map_err(storage_error)?;
            }
        }
        tx.commit().await.map_err(storage_error)?;
        Ok(SubscriptionState {
            channel_name: channel,
            thread_id: root,
            target_agent: target_path,
            enabled,
            last_message_id: last,
        })
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

    async fn insert_channel(
        &self,
        conn: &mut SqliteConnection,
        name: &str,
        author: &AgentPath,
        now: DateTime<Utc>,
    ) -> Result<()> {
        let inserted = sqlx::query("INSERT OR IGNORE INTO channels(board,name,name_search,created_at,timestamp,author) VALUES(?,?,?,?,?,?)")
            .bind(self.identity.to_string()).bind(name).bind(default_case_fold_str(name)).bind(now.to_rfc3339()).bind(now.timestamp_micros()).bind(author.to_string())
            .execute(conn).await.map_err(storage_error)?.rows_affected();
        if inserted == 0 {
            return Err(invalid("channel already exists"));
        }
        Ok(())
    }

    async fn subscribe(
        &self,
        conn: &mut SqliteConnection,
        target: &SubscriptionTarget,
        agent: ThreadId,
    ) -> Result<()> {
        sqlx::query("INSERT OR IGNORE INTO subscriptions(board,target,agent) VALUES(?,?,?)")
            .bind(self.identity.to_string())
            .bind(target_key(target)?)
            .bind(agent.to_string())
            .execute(conn)
            .await
            .map_err(storage_error)?;
        Ok(())
    }

    async fn load_post(&self, conn: &mut SqliteConnection, id: Uuid) -> Result<StoredPost> {
        let payload: Option<String> =
            sqlx::query_scalar("SELECT payload FROM posts WHERE board=? AND id=?")
                .bind(self.identity.to_string())
                .bind(id.to_string())
                .fetch_optional(conn)
                .await
                .map_err(storage_error)?;
        serde_json::from_str(&payload.ok_or_else(|| invalid("post not found in this board"))?)
            .map_err(storage_error)
    }

    async fn existing_post(
        &self,
        conn: &mut SqliteConnection,
        request_id: &str,
        request: &str,
    ) -> Result<Option<StoredPost>> {
        let row = sqlx::query("SELECT payload,request FROM posts WHERE board=? AND request_id=?")
            .bind(self.identity.to_string())
            .bind(request_id)
            .fetch_optional(conn)
            .await
            .map_err(storage_error)?;
        row.map(|row| {
            if row.get::<String, _>("request") != request {
                return Err(invalid("request ID was already used for a different post"));
            }
            serde_json::from_str(row.get("payload")).map_err(storage_error)
        })
        .transpose()
    }

    async fn channel_summary(
        &self,
        conn: &mut SqliteConnection,
        name: &str,
    ) -> Result<ChannelSummary> {
        let row = sqlx::query(
            "SELECT c.created_at,c.author,
             (SELECT COUNT(*) FROM posts p WHERE p.board=c.board AND p.channel=c.name) AS message_count,
             (SELECT p.id FROM posts p WHERE p.board=c.board AND p.channel=c.name ORDER BY p.timestamp DESC,p.seq DESC LIMIT 1) AS last_message_id
             FROM channels c WHERE c.board=? AND c.name=?",
        )
        .bind(self.identity.to_string())
        .bind(name)
        .fetch_optional(conn)
        .await
        .map_err(storage_error)?
        .ok_or_else(|| invalid("channel not found in this board"))?;
        Ok(ChannelSummary {
            channel_name: name.to_string(),
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
}

fn validate_channel(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > MAX_CHANNEL_BYTES
        || name.trim() != name
        || name.chars().any(char::is_control)
    {
        return Err(invalid(
            "channel names must contain 1–128 bytes without edge whitespace or control characters",
        ));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> CodexErr {
    CodexErr::InvalidRequest(message.into())
}

fn storage_error(error: impl std::fmt::Display) -> CodexErr {
    CodexErr::Io(std::io::Error::other(error.to_string()))
}

fn target_key(target: &SubscriptionTarget) -> Result<String> {
    serde_json::to_string(target).map_err(storage_error)
}
