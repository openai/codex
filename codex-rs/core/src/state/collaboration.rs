//! Session-scoped collaboration state for multi-agent flows.

use std::collections::HashMap;

use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use serde::Deserialize;
use serde::Serialize;

use crate::codex::SessionConfiguration;
use crate::context_manager::ContextManager;
use crate::protocol::TokenUsageInfo;
use crate::truncate::TruncationPolicy;
use tracing::warn;

fn content_for_log(message: &ResponseItem) -> String {
    match message {
        ResponseItem::Message { content, .. } => {
            let mut rendered = String::new();
            let mut is_first = true;
            for item in content {
                if !is_first {
                    rendered.push('\n');
                }
                is_first = false;
                match item {
                    ContentItem::InputText { text } => rendered.push_str(text),
                    _ => rendered.push_str("<non-text content>"),
                }
            }
            rendered
        }
        _ => "<non-message item>".to_string(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) struct AgentId(pub i32);

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(crate) enum AgentLifecycleState {
    Idle { last_agent_message: Option<String> },
    Running,
    Exhausted,
    Error { error: String },
    Closed,
    WaitingForApproval { request: String },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ContextStrategy {
    New,
    Fork,
    Replace(Vec<ResponseItem>),
}

impl Default for ContextStrategy {
    fn default() -> Self {
        Self::New
    }
}

#[derive(Clone, Debug)]
pub(crate) struct AgentState {
    pub(crate) id: AgentId,
    pub(crate) name: String,
    pub(crate) parent: Option<AgentId>,
    pub(crate) depth: i32,
    pub(crate) config: SessionConfiguration,
    pub(crate) instructions: Option<String>,
    pub(crate) status: AgentLifecycleState,
    pub(crate) history: ContextManager,
}

impl AgentState {
    pub(crate) fn new_root(
        name: String,
        config: SessionConfiguration,
        history: ContextManager,
        instructions: Option<String>,
    ) -> Self {
        Self {
            id: AgentId(0),
            name,
            parent: None,
            depth: 0,
            config,
            instructions,
            status: AgentLifecycleState::Idle {
                last_agent_message: None,
            },
            history,
        }
    }

    pub(crate) fn new_child(
        id: AgentId,
        name: String,
        parent: AgentId,
        depth: i32,
        config: SessionConfiguration,
        instructions: Option<String>,
        history: ContextManager,
    ) -> Self {
        Self {
            id,
            name,
            parent: Some(parent),
            depth,
            config,
            instructions,
            status: AgentLifecycleState::Idle {
                last_agent_message: None,
            },
            history,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CollaborationLimits {
    pub(crate) max_agents: i32,
    pub(crate) max_depth: i32,
}

impl Default for CollaborationLimits {
    fn default() -> Self {
        Self {
            max_agents: 8,
            max_depth: 4,
        }
    }
}

pub(crate) struct CollaborationState {
    agents: Vec<AgentState>,
    children: HashMap<AgentId, Vec<AgentId>>,
    limits: CollaborationLimits,
    next_sub_id: i64,
    sub_ids: HashMap<String, AgentId>,
}

impl CollaborationState {
    pub(crate) fn new(limits: CollaborationLimits) -> Self {
        Self {
            agents: Vec::new(),
            children: HashMap::new(),
            limits,
            next_sub_id: 0,
            sub_ids: HashMap::new(),
        }
    }

    pub(crate) fn limits(&self) -> &CollaborationLimits {
        &self.limits
    }

    pub(crate) fn ensure_root_agent(
        &mut self,
        session_configuration: &SessionConfiguration,
        session_history: &ContextManager,
    ) -> AgentId {
        if self.agents.is_empty() {
            let root = AgentState::new_root(
                "main".to_string(),
                session_configuration.clone(),
                session_history.clone(),
                session_configuration
                    .developer_instructions()
                    .or_else(|| session_configuration.user_instructions()),
            );
            self.agents.push(root);
        } else if let Some(root) = self.agents.get_mut(0) {
            root.config = session_configuration.clone();
            root.history = session_history.clone();
            if root.instructions.is_none() {
                root.instructions = session_configuration
                    .developer_instructions()
                    .or_else(|| session_configuration.user_instructions());
            }
        }
        AgentId(0)
    }

    pub(crate) fn agents(&self) -> &[AgentState] {
        &self.agents
    }

    pub(crate) fn agent(&self, id: AgentId) -> Option<&AgentState> {
        self.index_for(id).and_then(|idx| self.agents.get(idx))
    }

    pub(crate) fn agent_mut(&mut self, id: AgentId) -> Option<&mut AgentState> {
        let index = self.index_for(id)?;
        self.agents.get_mut(index)
    }

    pub(crate) fn next_agent_id(&self) -> AgentId {
        AgentId(self.agents.len() as i32)
    }

    pub(crate) fn clone_agent_history(&self, id: AgentId) -> Option<ContextManager> {
        self.agent(id).map(|agent| agent.history.clone())
    }

    pub(crate) fn set_agent_history(
        &mut self,
        id: AgentId,
        items: Vec<ResponseItem>,
        token_info: Option<TokenUsageInfo>,
    ) -> Result<(), String> {
        let agent = self
            .agent_mut(id)
            .ok_or_else(|| format!("unknown agent {}", id.0))?;
        agent.history.replace(items);
        agent.history.set_token_info(token_info);
        Ok(())
    }

    pub(crate) fn record_message_for_agent(&mut self, id: AgentId, message: ResponseItem) {
        let role = match &message {
            ResponseItem::Message { role, .. } => role.as_str(),
            _ => "other",
        };
        let content = content_for_log(&message);
        if let Some(agent) = self.agent_mut(id) {
            warn!(
                agent_idx = id.0,
                agent_name = agent.name.as_str(),
                role,
                content,
                "collaboration: agent received message"
            );
            agent
                .history
                .record_items([message].iter(), TruncationPolicy::Bytes(10_000));
        } else {
            warn!(
                agent_idx = id.0,
                agent_name = "<unknown>",
                role,
                content,
                "collaboration: message delivered to unknown agent"
            );
        }
    }

    #[allow(dead_code)]
    pub(crate) fn record_items_for_agent(
        &mut self,
        id: AgentId,
        items: &[ResponseItem],
        policy: TruncationPolicy,
    ) {
        if let Some(agent) = self.agent_mut(id) {
            agent.history.record_items(items.iter(), policy);
        }
    }

    pub(crate) fn add_child(&mut self, mut agent: AgentState) -> Result<AgentId, String> {
        if self.agents.len() as i32 >= self.limits.max_agents {
            return Err("max agent count reached".to_string());
        }
        if agent.depth > self.limits.max_depth {
            return Err("max collaboration depth reached".to_string());
        }

        let id = self.next_agent_id();
        agent.id = id;

        if let Some(parent) = agent.parent {
            self.children.entry(parent).or_default().push(id);
        }

        self.agents.push(agent);
        Ok(id)
    }

    pub(crate) fn is_direct_child(&self, parent: AgentId, child: AgentId) -> bool {
        self.children
            .get(&parent)
            .map(|kids| kids.contains(&child))
            .unwrap_or(false)
    }

    pub(crate) fn descendants(&self, roots: &[AgentId]) -> Vec<AgentId> {
        let mut result = Vec::new();
        let mut stack: Vec<AgentId> = roots.to_vec();
        while let Some(id) = stack.pop() {
            result.push(id);
            if let Some(children) = self.children.get(&id) {
                for child in children {
                    stack.push(*child);
                }
            }
        }
        result
    }

    pub(crate) fn next_sub_id(&mut self, agent: AgentId) -> String {
        let sub_id = format!("collab-agent-{}-{}", agent.0, self.next_sub_id);
        self.next_sub_id += 1;
        sub_id
    }

    fn index_for(&self, id: AgentId) -> Option<usize> {
        if id.0 < 0 {
            return None;
        }
        let index = id.0 as usize;
        if index < self.agents.len() {
            Some(index)
        } else {
            None
        }
    }

    pub(crate) fn register_sub_id(&mut self, agent: AgentId, sub_id: String) {
        self.sub_ids.insert(sub_id, agent);
    }

    pub(crate) fn agent_for_sub_id(&self, sub_id: &str) -> Option<AgentId> {
        self.sub_ids.get(sub_id).copied()
    }
}
