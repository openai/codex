//! Exposes local status changes as shared agent snapshots instead of Tokio receivers.
//! Subscriptions retain no runtime handle and preserve the watch channel's coalescing.

use super::LocalAgentControl;
use crate::agent::api::AgentInfo;
use crate::agent::api::StatusSubscription;
use crate::agent::types::LiveAgent;
use codex_protocol::ThreadId;
use codex_protocol::error::Result as CodexResult;
use futures::StreamExt;
use futures::stream;
use std::sync::Arc;

impl LocalAgentControl {
    pub(crate) async fn subscribe_status(
        &self,
        agent_id: ThreadId,
    ) -> CodexResult<StatusSubscription> {
        let manager = self.upgrade()?;
        let thread = manager.get_thread(agent_id).await?;
        // Subscribe before reading settings; mark the initial status seen only afterward.
        let mut receiver = thread.subscribe_status();
        let config = Box::new(thread.config_snapshot().await);
        let initial = LiveAgent {
            thread_id: agent_id,
            metadata: self.get_agent_metadata(agent_id).unwrap_or_default(),
            status: receiver.borrow_and_update().clone(),
        };
        let weak_thread = Arc::downgrade(&thread);
        let changes = stream::unfold(
            (receiver, initial.clone(), config.clone()),
            move |(mut receiver, mut snapshot, mut config)| {
                let weak_thread = weak_thread.clone();
                async move {
                    receiver.changed().await.ok()?;
                    snapshot.status = receiver.borrow_and_update().clone();
                    if let Some(thread) = weak_thread.upgrade() {
                        *config = thread.config_snapshot().await;
                    }
                    // A final status can outlive the runtime. Keep its last observed settings
                    // so removal from the manager does not hide an already-published update.
                    Some((
                        Ok(AgentInfo::Loaded {
                            agent: snapshot.clone(),
                            config: config.clone(),
                        }),
                        (receiver, snapshot, config),
                    ))
                }
            },
        );
        Ok(stream::once(async move {
            Ok(AgentInfo::Loaded {
                agent: initial,
                config,
            })
        })
        .chain(changes)
        .boxed())
    }
}
