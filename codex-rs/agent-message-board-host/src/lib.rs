//! Supplies board host capabilities through the public Core runtime interfaces.
//!
//! The extension owns backend construction, storage and tool registration. This adapter
//! selects configuration and supplies host capabilities. It never starts or restores
//! recipients during lookup. Notifications use ordinary agent-message scheduling.
//! Permanent board deletion uses the thread store's existing host cleanup callback.

#![deny(clippy::print_stdout, clippy::print_stderr)]

use chrono::DateTime;
use chrono::Utc;
use codex_agent_message_board_client::RemoteAgentMessageBoard;
use codex_agent_message_board_extension::BoardBackend;
use codex_agent_message_board_extension::local_board_options;
use codex_core::CodexThread;
use codex_core::MULTI_AGENT_V2_NAMESPACE_DESCRIPTION;
use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadStartInput;
use codex_features::Feature;
use codex_message_board::LocalAgentMessageBoard;
use codex_message_board::LocalBoardConnection;
use codex_message_board::MessageBoardHost;
use codex_message_board::NotificationDelivery;
use codex_message_board::PostPreview;
use codex_protocol::AgentPath;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::error::Result;
use codex_protocol::protocol::InterAgentCommunication;
use codex_protocol::protocol::Op;
use codex_state::SqliteConfig;
use codex_thread_store::ThreadStoreError;
use codex_thread_store::ThreadStoreResult;
use futures::future::BoxFuture;
use std::sync::Arc;
use std::sync::Weak;

/// Registers the configured board for opted-in MAv2 runtimes.
/// Training can share an in-memory board per tree, including ephemeral sessions.
pub fn install_agent_message_board(
    registry: &mut ExtensionRegistryBuilder<Config>,
    manager: Weak<ThreadManager>,
) {
    codex_agent_message_board_client::install_notifications(registry, |input| {
        let board = input.thread_store.get::<RemoteAgentMessageBoard>()?;
        let host = input.thread_store.get::<LocalBoardHost>()?;
        Some((
            board,
            Arc::new(LocalBoardHost {
                remote_notifications: true,
                ..host.as_ref().clone()
            }),
        ))
    });
    codex_agent_message_board_extension::install(
        registry,
        MULTI_AGENT_V2_NAMESPACE_DESCRIPTION,
        |config: &Config| config.multi_agent_v2.tool_namespace.clone(),
        move |input: &ThreadStartInput<'_, Config>, tree, caller| {
            let config = input.config;
            let connection = input.thread_store.get::<LocalBoardConnection>();
            let in_memory = config.multi_agent_v2.message_board_in_memory;
            let mcp =
                config.features.enabled(Feature::AgentMessageBoardMcp) && connection.is_none();
            // Automatic boards need MAv2. In-memory boards and explicit host
            // connections also work for isolated, ephemeral runtimes.
            if connection.is_none()
                && ((!config.features.enabled(Feature::AgentMessageBoard) && !mcp)
                    || !config.features.enabled(Feature::MultiAgentV2)
                    || (config.ephemeral
                        && !mcp
                        && !in_memory
                        && config.multi_agent_v2.message_board_remote.is_none()))
            {
                return None;
            }
            let backend = if mcp {
                BoardBackend::Mcp
            } else if let Some(connection) = &connection {
                BoardBackend::Connected(connection.clone())
            } else if let Some(remote) = config.multi_agent_v2.message_board_remote.clone() {
                BoardBackend::Remote {
                    config: remote,
                    http: config.http_client_factory(),
                }
            } else if in_memory {
                BoardBackend::InMemory
            } else {
                BoardBackend::Local(config.sqlite_config().clone())
            };
            Some((
                backend,
                LocalBoardHost {
                    manager: manager.clone(),
                    tree,
                    caller,
                    remote_notifications: false,
                    connection,
                },
            ))
        },
    );
}

/// Removes persisted agent-board data through the thread store's deletion callback.
/// Cleanup also applies when the board feature is currently disabled.
pub async fn delete_agent_message_boards(
    sqlite: &SqliteConfig,
    thread_ids: &[ThreadId],
) -> ThreadStoreResult<()> {
    let boards = thread_ids
        .iter()
        .copied()
        .map(Into::into)
        .collect::<Vec<_>>();
    LocalAgentMessageBoard::delete_boards(sqlite, &local_board_options(), &boards)
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to delete agent message boards: {err}"),
        })
}

#[derive(Clone)]
struct LocalBoardHost {
    manager: Weak<ThreadManager>,
    tree: SessionId,
    caller: ThreadId,
    remote_notifications: bool,
    connection: Option<Arc<LocalBoardConnection>>,
}

impl LocalBoardHost {
    async fn actor(&self, caller: ThreadId) -> Result<Arc<CodexThread>> {
        let manager = self
            .manager
            .upgrade()
            .ok_or_else(|| CodexErr::ThreadNotFound(caller))?;
        if let Some(connection) = &self.connection {
            return manager
                .get_thread_with_extension::<LocalBoardConnection>(caller, |other| {
                    connection.same_board(other)
                })
                .await;
        }
        let actor = manager.get_thread(caller).await?;
        if actor.startup_metadata().session_id != self.tree
            || actor
                .thread_extension_data()
                .get::<LocalBoardConnection>()
                .is_some()
        {
            return Err(CodexErr::InvalidRequest(
                "agent belongs to another message board".into(),
            ));
        }
        Ok(actor)
    }

    async fn registered_path(&self, actor: &CodexThread) -> Result<AgentPath> {
        let caller = actor.startup_metadata().thread_id;
        if let Some(connection) = &self.connection {
            return connection.caller_path(caller);
        }
        let path = actor
            .session_source()
            .get_agent_path()
            .or_else(|| (caller == ThreadId::from(self.tree)).then(AgentPath::root))
            .ok_or_else(|| CodexErr::InvalidRequest("agent has no tree path".into()))?;
        let resolved = actor.resolve_agent_reference(path.as_str()).await?;
        if resolved != caller {
            return Err(CodexErr::InvalidRequest(
                "agent does not own its tree path".into(),
            ));
        }
        Ok(path)
    }
}

impl MessageBoardHost for LocalBoardHost {
    fn agent_path(&self, caller: ThreadId) -> BoxFuture<'_, Result<AgentPath>> {
        Box::pin(async move {
            let actor = self.actor(caller).await?;
            self.registered_path(&actor).await
        })
    }

    fn resolve_agent(&self, path: AgentPath) -> BoxFuture<'_, Result<ThreadId>> {
        Box::pin(async move {
            if self.connection.is_some() {
                let caller = path
                    .as_str()
                    .strip_prefix("/root/")
                    .and_then(|id| ThreadId::from_string(id).ok())
                    .ok_or_else(|| {
                        CodexErr::InvalidRequest("invalid connected board agent path".into())
                    })?;
                return self
                    .actor(caller)
                    .await
                    .map(|actor| actor.startup_metadata().thread_id);
            }
            let actor = self.actor(self.caller).await?;
            actor.resolve_agent_reference(path.as_str()).await
        })
    }

    fn current_time(&self, caller: ThreadId) -> BoxFuture<'_, Result<DateTime<Utc>>> {
        Box::pin(async move {
            let actor = self.actor(caller).await?;
            self.registered_path(&actor).await?;
            actor.current_time().await
        })
    }

    fn notify(
        &self,
        recipient_id: ThreadId,
        post: PostPreview,
    ) -> BoxFuture<'_, Result<NotificationDelivery>> {
        Box::pin(async move {
            let recipient = match self.actor(recipient_id).await {
                Ok(thread) => thread,
                Err(error) if matches!(error.details(), CodexErrorDetails::ThreadNotFound(_)) => {
                    return Ok(NotificationDelivery::SkippedInactive);
                }
                Err(error) => return Err(error),
            };
            let recipient_path = self.registered_path(&recipient).await?;
            let communication = InterAgentCommunication::new(
                post.metadata.author.clone(),
                recipient_path,
                Vec::new(),
                post.notification_text(),
                /*trigger_turn*/ false,
            );
            // Remote metadata is untrusted. Skip an oversized notice without closing
            // the receiver; the post remains available through the board tools.
            if self.remote_notifications
                && communication.content.len()
                    + communication.author.as_str().len()
                    + communication.recipient.as_str().len()
                    > 1024
            {
                return Err(CodexErr::InvalidRequest(
                    "remote board notice exceeds the context budget".into(),
                ));
            }
            recipient
                .submit(Op::InterAgentCommunication {
                    communication,
                    start_options: Default::default(),
                })
                .await?;
            Ok(NotificationDelivery::Accepted)
        })
    }
}
