//! Local registry lookups for startup, host context and loaded descendant checks.
//! These use the same local registry and thread manager as lifecycle operations.

use super::LocalAgentRuntime;
use crate::agent::types::AgentMetadata;
use crate::session_prefix::format_subagent_context_line;
use crate::thread_manager::ThreadManagerState;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::MultiAgentVersion;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

const MAX_ENVIRONMENT_SUBAGENTS: usize = 8;
const MAX_ENVIRONMENT_SUBAGENT_BYTES: usize = 1_024;

impl LocalAgentRuntime {
    pub(crate) fn register_session_root(
        &self,
        current_thread_id: ThreadId,
        current_parent_thread_id: Option<ThreadId>,
    ) {
        if current_parent_thread_id.is_none() {
            self.registry.register_root_thread(current_thread_id);
        }
    }

    pub(crate) fn ensure_agent_known(&self, agent_id: ThreadId) -> CodexResult<AgentMetadata> {
        self.registry
            .agent_metadata_for_thread(agent_id)
            .ok_or_else(|| CodexErr::ThreadNotFound(agent_id))
    }

    pub(crate) async fn list_live_agent_subtree_thread_ids(
        &self,
        agent_id: ThreadId,
    ) -> CodexResult<Vec<ThreadId>> {
        let mut thread_ids = vec![agent_id];
        thread_ids.extend(self.live_thread_spawn_descendants(agent_id).await?);
        Ok(thread_ids)
    }

    pub(crate) async fn format_environment_context_subagents(
        &self,
        parent_thread_id: ThreadId,
        multi_agent_version: MultiAgentVersion,
    ) -> String {
        if multi_agent_version != MultiAgentVersion::V2 {
            let Ok(agents) = self.open_thread_spawn_children(parent_thread_id).await else {
                return String::new();
            };
            return agents
                .into_iter()
                .map(|(thread_id, metadata)| {
                    let reference = metadata
                        .agent_path
                        .as_ref()
                        .map(|path| path.name().to_string())
                        .unwrap_or_else(|| thread_id.to_string());
                    format_subagent_context_line(&reference, metadata.agent_nickname.as_deref())
                })
                .collect::<Vec<_>>()
                .join("\n");
        }

        let Some(parent_path) = self
            .registry
            .agent_metadata_for_thread(parent_thread_id)
            .and_then(|metadata| metadata.agent_path)
        else {
            return String::new();
        };
        let parent_prefix = format!("{parent_path}/");
        let mut agent_paths = self
            .registry
            .live_agents()
            .into_iter()
            .filter_map(|metadata| metadata.agent_path)
            .filter(|path| {
                path.as_str()
                    .strip_prefix(&parent_prefix)
                    .is_some_and(|name| !name.contains('/'))
            })
            .collect::<Vec<_>>();
        let loaded_paths = self
            .open_thread_spawn_children(parent_thread_id)
            .await
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(_, metadata)| metadata.agent_path)
            .collect::<HashSet<_>>();
        agent_paths.sort();
        // Stable sorting preserves alphabetical order within each group.
        agent_paths.sort_by_key(|path| !loaded_paths.contains(path));

        let mut lines = Vec::with_capacity(agent_paths.len().min(MAX_ENVIRONMENT_SUBAGENTS));
        let mut rendered_bytes = "  <subagents>\n  </subagents>\n".len();
        for agent_path in agent_paths {
            if lines.len() == MAX_ENVIRONMENT_SUBAGENTS {
                break;
            }
            let line = format!(r#"<agent name="{agent_path}" />"#);
            let line_bytes = "    \n".len() + line.len();
            if rendered_bytes + line_bytes <= MAX_ENVIRONMENT_SUBAGENT_BYTES {
                rendered_bytes += line_bytes;
                lines.push(line);
            }
        }
        lines.join("\n")
    }

    pub(super) async fn open_thread_spawn_children(
        &self,
        parent_thread_id: ThreadId,
    ) -> CodexResult<Vec<(ThreadId, AgentMetadata)>> {
        let mut children_by_parent = self.live_thread_spawn_children().await?;
        Ok(children_by_parent
            .remove(&parent_thread_id)
            .unwrap_or_default())
    }

    pub(super) async fn live_thread_spawn_children(
        &self,
    ) -> CodexResult<HashMap<ThreadId, Vec<(ThreadId, AgentMetadata)>>> {
        let state = self.upgrade()?;
        let mut children_by_parent = HashMap::<ThreadId, Vec<(ThreadId, AgentMetadata)>>::new();

        for (parent_thread_id, child_thread_id) in state.list_live_thread_spawn_edges().await {
            children_by_parent
                .entry(parent_thread_id)
                .or_default()
                .push((
                    child_thread_id,
                    self.registry
                        .agent_metadata_for_thread(child_thread_id)
                        .unwrap_or(AgentMetadata {
                            agent_id: Some(child_thread_id),
                            ..Default::default()
                        }),
                ));
        }

        for children in children_by_parent.values_mut() {
            children.sort_by(|left, right| {
                left.1
                    .agent_path
                    .as_deref()
                    .unwrap_or_default()
                    .cmp(right.1.agent_path.as_deref().unwrap_or_default())
                    .then_with(|| left.0.to_string().cmp(&right.0.to_string()))
            });
        }

        Ok(children_by_parent)
    }

    pub(super) async fn live_thread_spawn_descendants(
        &self,
        root_thread_id: ThreadId,
    ) -> CodexResult<Vec<ThreadId>> {
        let mut children_by_parent = self.live_thread_spawn_children().await?;
        let mut descendants = Vec::new();
        let mut stack = children_by_parent
            .remove(&root_thread_id)
            .unwrap_or_default()
            .into_iter()
            .map(|(child_thread_id, _)| child_thread_id)
            .rev()
            .collect::<Vec<_>>();

        while let Some(thread_id) = stack.pop() {
            descendants.push(thread_id);
            if let Some(children) = children_by_parent.remove(&thread_id) {
                for (child_thread_id, _) in children.into_iter().rev() {
                    stack.push(child_thread_id);
                }
            }
        }

        Ok(descendants)
    }

    pub(super) fn upgrade(&self) -> CodexResult<Arc<ThreadManagerState>> {
        self.manager
            .upgrade()
            .ok_or_else(|| CodexErr::UnsupportedOperation("thread manager dropped".to_string()))
    }
}
