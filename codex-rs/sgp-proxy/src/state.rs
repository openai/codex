use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::agentex_client::AgentexClient;
use crate::config::Args;
use crate::config::TaskLifecycleMode;

/// Per-session context tracking the Agentex task and tool-call metadata.
#[derive(Debug)]
pub struct SessionContext {
    pub task_id: String,
    /// Maps tool call_id → tool name so we can populate ToolResponseContent
    /// when Codex sends back FunctionCallOutput.
    pub tool_name_by_call_id: HashMap<String, String>,
    pub is_first_turn: bool,
}

/// Shared proxy state accessible from Axum handlers.
pub struct ProxyState {
    pub sessions: RwLock<HashMap<String, SessionContext>>,
    pub client: AgentexClient,
    pub agent_id: String,
    pub task_lifecycle: TaskLifecycleMode,
    pub agent_tools: HashSet<String>,
    pub http_shutdown: bool,
}

impl ProxyState {
    pub fn new(args: &Args, auth_header: &'static str) -> Arc<Self> {
        let client = AgentexClient::new(args.agentex_url.clone(), auth_header);
        let agent_tools: HashSet<String> = args.agent_tools.iter().cloned().collect();

        Arc::new(Self {
            sessions: RwLock::new(HashMap::new()),
            client,
            agent_id: args.agent_id.clone(),
            task_lifecycle: args.task_lifecycle,
            agent_tools,
            http_shutdown: args.http_shutdown,
        })
    }
}
