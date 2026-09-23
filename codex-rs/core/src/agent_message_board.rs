//! Bridges the board extension to existing tree, clock and active-turn services.
//!
//! The extension owns storage and tools. This adapter never starts or restores
//! recipients and never queues a notification for an idle agent.

use crate::CodexThread;
use crate::ThreadManager;
use crate::config::Config;
use crate::context::AgentMessageBoardNotification;
use crate::context::ContextualUserFragment;
use crate::tools::MULTI_AGENT_V2_NAMESPACE_DESCRIPTION;
use chrono::DateTime;
use chrono::Utc;
use codex_agent_message_board_extension::AgentMessageBoard;
use codex_agent_message_board_extension::LocalAgentMessageBoard;
use codex_agent_message_board_extension::MessageBoardHost;
use codex_agent_message_board_extension::NotificationDelivery;
use codex_agent_message_board_extension::PostPreview;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_features::Feature;
use codex_protocol::AgentPath;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::error::Result;
use codex_protocol::protocol::InterAgentCommunication;
use futures::future::BoxFuture;
use std::sync::Arc;
use std::sync::Weak;

/// Registers the local board for opted-in, persistent MAv2 runtimes.
/// Shared session identity and the configured SQLite home survive runtime reloads.
pub fn install_agent_message_board(
    registry: &mut ExtensionRegistryBuilder<Config>,
    manager: Weak<ThreadManager>,
) {
    codex_agent_message_board_extension::install(
        registry,
        MULTI_AGENT_V2_NAMESPACE_DESCRIPTION,
        |config: &Config| config.multi_agent_v2.tool_namespace.clone(),
        move |config: &Config, tree, caller| {
            // MAv2 supplies tree paths; ephemeral runtimes must not open durable storage.
            if !config.features.enabled(Feature::AgentMessageBoard)
                || !config.features.enabled(Feature::MultiAgentV2)
                || config.ephemeral
            {
                return Box::pin(async { Ok(None) });
            }
            let sqlite = config.sqlite_config().clone();
            let host = Arc::new(LocalBoardHost {
                manager: manager.clone(),
                tree,
                caller,
            });
            Box::pin(async move {
                let board = LocalAgentMessageBoard::open(&sqlite, tree, host).await?;
                Ok(Some(Arc::new(board) as Arc<dyn AgentMessageBoard>))
            })
        },
    );
}

struct LocalBoardHost {
    manager: Weak<ThreadManager>,
    tree: SessionId,
    caller: ThreadId,
}

impl LocalBoardHost {
    async fn actor(&self) -> Result<Arc<CodexThread>> {
        let manager = self
            .manager
            .upgrade()
            .ok_or_else(|| CodexErr::ThreadNotFound(self.caller))?;
        let actor = manager.get_thread(self.caller).await?;
        if actor.session.session_id() != self.tree {
            return Err(CodexErr::InvalidRequest(
                "agent belongs to another message board".into(),
            ));
        }
        // Match existing collaboration resolution: roots are registered lazily.
        if self.caller == ThreadId::from(self.tree) {
            actor
                .session
                .services
                .local_agent_runtime
                .register_session_root(self.caller, /*current_parent_thread_id*/ None);
        }
        Ok(actor)
    }
}

impl MessageBoardHost for LocalBoardHost {
    fn agent_path(&self, caller: ThreadId) -> BoxFuture<'_, Result<AgentPath>> {
        Box::pin(async move {
            self.actor()
                .await?
                .session
                .services
                .local_agent_runtime
                .ensure_agent_known(caller)?
                .agent_path
                .ok_or_else(|| CodexErr::InvalidRequest("agent has no tree path".into()))
        })
    }

    fn resolve_agent(&self, path: AgentPath) -> BoxFuture<'_, Result<ThreadId>> {
        Box::pin(async move {
            let actor = self.actor().await?;
            actor
                .session
                .services
                .local_agent_runtime
                .resolve_agent_reference(self.caller, &actor.session_source, path.as_str())
                .await
        })
    }

    fn current_time(&self, caller: ThreadId) -> BoxFuture<'_, Result<DateTime<Utc>>> {
        Box::pin(async move {
            self.agent_path(caller).await?;
            let manager = self
                .manager
                .upgrade()
                .ok_or_else(|| CodexErr::ThreadNotFound(caller))?;
            let actor = manager.get_thread(caller).await?;
            if actor.session.session_id() != self.tree {
                return Err(CodexErr::InvalidRequest(
                    "agent belongs to another message board".into(),
                ));
            }
            actor
                .session
                .services
                .time_provider
                .current_time(caller)
                .await
                .map_err(|error| CodexErr::Io(std::io::Error::other(error)))
        })
    }

    fn notify(
        &self,
        recipient_id: ThreadId,
        post: PostPreview,
    ) -> BoxFuture<'_, Result<NotificationDelivery>> {
        Box::pin(async move {
            let Some(manager) = self.manager.upgrade() else {
                return Ok(NotificationDelivery::SkippedInactive);
            };
            let recipient = match manager.get_thread(recipient_id).await {
                Ok(thread) => thread,
                Err(error) if matches!(error.details(), CodexErrorDetails::ThreadNotFound(_)) => {
                    return Ok(NotificationDelivery::SkippedInactive);
                }
                Err(error) => return Err(error),
            };
            if recipient.session.session_id() != self.tree {
                return Err(CodexErr::InvalidRequest(
                    "notification recipient belongs to another board".into(),
                ));
            }
            let recipient_path = recipient
                .session
                .services
                .local_agent_runtime
                .ensure_agent_known(recipient_id)?
                .agent_path
                .ok_or_else(|| CodexErr::InvalidRequest("agent has no tree path".into()))?;
            let notice = AgentMessageBoardNotification(post);
            let communication = InterAgentCommunication::new(
                notice.0.metadata.author.clone(),
                recipient_path,
                Vec::new(),
                notice.render(),
                /*trigger_turn*/ false,
            );
            Ok(
                match recipient
                    .inject_if_running(vec![communication.to_model_input_item()])
                    .await
                {
                    Ok(()) => NotificationDelivery::Accepted,
                    Err(_) => NotificationDelivery::SkippedInactive,
                },
            )
        })
    }
}
