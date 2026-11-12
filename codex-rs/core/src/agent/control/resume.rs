//! Reopens an agent only when its runtime is absent, then returns its effective state.
//! Restores recorded ancestor ownership before loading a suspended descendant.

use super::LocalAgentControl;
use crate::CodexThread;
use crate::agent::types::LiveAgent;
use crate::codex_thread::ThreadConfigSnapshot;
use crate::config::Config;
use crate::thread_manager::LoadedSuspendedAgent;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_thread_store::ReadThreadParams;
use std::collections::HashSet;
use std::sync::Arc;

impl LocalAgentControl {
    /// Load ancestors first so each child inherits its actual parent's runtime and permissions.
    pub(crate) async fn load_suspended_child(
        &self,
        root: &Arc<CodexThread>,
        child_id: ThreadId,
    ) -> CodexResult<LoadedSuspendedAgent> {
        let root_id = root.startup_metadata().thread_id;
        if child_id == root_id || root.multi_agent_version() != Some(MultiAgentVersion::V2) {
            return Err(CodexErr::InvalidRequest(
                "expected a named V2 descendant".to_owned(),
            ));
        }
        let state = self.runtime.upgrade()?;
        let mut chain = Vec::new();
        let mut visited = HashSet::new();
        let mut id = child_id;
        while id != root_id {
            if !visited.insert(id) {
                return Err(CodexErr::InvalidRequest(
                    "cycle in stored agent ancestry".into(),
                ));
            }
            let stored = state
                .read_stored_thread(ReadThreadParams {
                    thread_id: id,
                    include_archived: false,
                    include_history: false,
                })
                .await?;
            let SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id,
                agent_path: Some(path),
                ..
            }) = &stored.source
            else {
                return Err(CodexErr::InvalidRequest(format!(
                    "thread {id} is not a named child of {root_id}"
                )));
            };
            if stored
                .parent_thread_id
                .is_some_and(|parent| parent != *parent_thread_id)
                || stored
                    .agent_path
                    .as_deref()
                    .is_some_and(|name| name != path.as_str())
            {
                return Err(CodexErr::InvalidRequest(format!(
                    "inconsistent stored ancestry for {id}"
                )));
            }
            chain.push(id);
            id = *parent_thread_id;
        }
        chain.reverse();
        let mut thread = Arc::clone(root);
        let mut residency = None;
        for id in chain {
            let config = thread.session.get_config().await;
            // Usually root resume registered the name from the graph. This also handles
            // stores without a graph, using the same identity restoration path.
            self.restore_agent_identities(&config, &state, vec![id])
                .await;
            let pin = self
                .ensure_v2_agent_loaded(config.as_ref().clone(), id, Some(thread))
                .await?;
            thread = state.get_thread(id).await?;
            // Allow an idle ancestor to give up its slot while its child reloads at capacity.
            if id == child_id {
                residency = pin;
            }
        }
        Ok(LoadedSuspendedAgent {
            thread,
            _residency: residency,
        })
    }

    pub(crate) async fn resume_agent(
        &self,
        config: Config,
        thread_id: ThreadId,
        source: SessionSource,
    ) -> CodexResult<(LiveAgent, ThreadConfigSnapshot)> {
        let manager = self.runtime.upgrade()?;
        let thread = match manager.get_thread(thread_id).await {
            Ok(thread) => thread,
            Err(err) if matches!(err.details(), CodexErrorDetails::ThreadNotFound(_)) => {
                Box::pin(self.resume_agent_from_rollout(config, thread_id, source)).await?;
                manager.get_thread(thread_id).await?
            }
            Err(err) => return Err(err),
        };
        let agent = LiveAgent {
            thread_id,
            metadata: self.get_agent_metadata(thread_id).unwrap_or_default(),
            status: thread.agent_status().await,
        };
        let config = thread.config_snapshot().await;
        Ok((agent, config))
    }
}
