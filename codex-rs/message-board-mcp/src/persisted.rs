//! Reads persisted membership without starting runtimes or modifying Codex's registry.
//! Posts use system time and optionally deliver notices through the existing app-server RPC.

use crate::app_server::AppServer;
use chrono::DateTime;
use chrono::Utc;
use codex_message_board::MessageBoardHost;
use codex_message_board::NotificationDelivery;
use codex_message_board::PostPreview;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;
use codex_protocol::protocol::InterAgentCommunication;
use futures::future::BoxFuture;
use sqlx::SqlitePool;
use std::sync::Arc;

pub(super) struct PersistedHost {
    pub registry: SqlitePool,
    pub root: ThreadId,
    pub app_server: Option<Arc<AppServer>>,
}

impl MessageBoardHost for PersistedHost {
    fn agent_path(&self, caller: ThreadId) -> BoxFuture<'_, Result<AgentPath>> {
        Box::pin(async move {
            let stored: Option<Option<String>> =
                sqlx::query_scalar("SELECT agent_path FROM threads WHERE id = ?")
                    .bind(caller.to_string())
                    .fetch_optional(&self.registry)
                    .await
                    .map_err(storage_error)?;
            let path = match stored.ok_or(CodexErr::ThreadNotFound(caller))? {
                Some(path) => AgentPath::from_string(path).map_err(CodexErr::InvalidRequest)?,
                None if caller == self.root => AgentPath::root(),
                None => {
                    return Err(CodexErr::InvalidRequest(
                        "caller has no persisted agent path".into(),
                    ));
                }
            };
            if self.resolve_agent(path.clone()).await? != caller {
                return Err(CodexErr::InvalidRequest(
                    "caller does not belong to this board".into(),
                ));
            }
            Ok(path)
        })
    }

    fn resolve_agent(&self, path: AgentPath) -> BoxFuture<'_, Result<ThreadId>> {
        Box::pin(async move {
            let ids: Vec<String> = sqlx::query_scalar(
                r#"
WITH RECURSIVE tree(id) AS (
    SELECT id FROM threads WHERE id = ?
    UNION
    SELECT child_thread_id FROM thread_spawn_edges JOIN tree ON parent_thread_id = tree.id
)
SELECT threads.id FROM tree JOIN threads ON threads.id = tree.id
WHERE (threads.id = ? AND ? = '/root' AND (agent_path IS NULL OR agent_path = '/root'))
   OR (threads.id != ? AND agent_path = ?)
LIMIT 2
"#,
            )
            .bind(self.root.to_string())
            .bind(self.root.to_string())
            .bind(path.as_str())
            .bind(self.root.to_string())
            .bind(path.as_str())
            .fetch_all(&self.registry)
            .await
            .map_err(storage_error)?;
            match ids.as_slice() {
                [id] => ThreadId::from_string(id)
                    .map_err(|error| CodexErr::InvalidRequest(error.to_string())),
                _ => Err(CodexErr::InvalidRequest(
                    "unknown or ambiguous agent path in this board".into(),
                )),
            }
        })
    }

    fn current_time(&self, caller: ThreadId) -> BoxFuture<'_, Result<DateTime<Utc>>> {
        Box::pin(async move {
            self.agent_path(caller).await?;
            Ok(Utc::now())
        })
    }

    fn notify(
        &self,
        recipient: ThreadId,
        post: PostPreview,
    ) -> BoxFuture<'_, Result<NotificationDelivery>> {
        Box::pin(async move {
            let Some(app_server) = &self.app_server else {
                return Ok(NotificationDelivery::SkippedInactive);
            };
            let path = self.agent_path(recipient).await?;
            let notice = InterAgentCommunication::new(
                post.metadata.author.clone(),
                path,
                Vec::new(),
                post.notification_text(),
                /*trigger_turn*/ false,
            );
            app_server
                .inject(recipient, notice)
                .await
                .map_err(|error| CodexErr::Io(std::io::Error::other(error)))?;
            Ok(NotificationDelivery::Accepted)
        })
    }
}

fn storage_error(error: sqlx::Error) -> CodexErr {
    CodexErr::Io(std::io::Error::other(error))
}
