//! Session state from the agent.
//!
//! This module contains state that comes from or is synchronized with
//! the core agent loop.

use std::path::PathBuf;

use cocode_protocol::AgentProgress;
use cocode_protocol::ThinkingLevel;
use cocode_protocol::TokenUsage;
use cocode_protocol::UserQueuedCommand;

/// State synchronized with the agent session.
#[derive(Debug, Clone)]
pub struct SessionState {
    /// Messages in the conversation.
    pub messages: Vec<ChatMessage>,

    /// Current model being used.
    pub current_model: String,

    /// Current thinking level.
    pub thinking_level: ThinkingLevel,

    /// Whether plan mode is active.
    pub plan_mode: bool,

    /// Current phase in plan mode (if active).
    pub plan_phase: Option<PlanPhase>,

    /// Path to the plan file (when in plan mode).
    pub plan_file: Option<PathBuf>,

    /// Active tool executions.
    pub tool_executions: Vec<ToolExecution>,

    /// Active subagent instances.
    pub subagents: Vec<SubagentInstance>,

    /// Total token usage for the session.
    pub token_usage: TokenUsage,

    /// Session ID (if resuming).
    pub session_id: Option<String>,

    /// Thinking budget in tokens (if set).
    pub thinking_budget_tokens: Option<i32>,

    /// Thinking tokens used in the current turn.
    pub thinking_tokens_used: i32,

    /// Connected MCP servers.
    pub mcp_servers: Vec<McpServerStatus>,

    /// Fallback model being used (if model fallback occurred).
    pub fallback_model: Option<String>,

    /// Whether context compaction is in progress.
    pub is_compacting: bool,

    /// Queue of commands for steering injection (Enter during streaming).
    /// Commands are consumed once in the agent loop and injected as steering
    /// system-reminders (consume-then-remove pattern).
    pub queued_commands: Vec<UserQueuedCommand>,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            current_model: "claude-sonnet-4-20250514".to_string(),
            thinking_level: ThinkingLevel::default(),
            plan_mode: false,
            plan_phase: None,
            plan_file: None,
            tool_executions: Vec::new(),
            subagents: Vec::new(),
            token_usage: TokenUsage::default(),
            session_id: None,
            thinking_budget_tokens: None,
            thinking_tokens_used: 0,
            mcp_servers: Vec::new(),
            fallback_model: None,
            is_compacting: false,
            queued_commands: Vec::new(),
        }
    }
}

impl SessionState {
    /// Add a message to the conversation.
    pub fn add_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
    }

    /// Get the last message, if any.
    pub fn last_message(&self) -> Option<&ChatMessage> {
        self.messages.last()
    }

    /// Get the last assistant message, if any.
    pub fn last_assistant_message(&self) -> Option<&ChatMessage> {
        self.messages
            .iter()
            .rev()
            .find(|m| m.role == MessageRole::Assistant)
    }

    /// Update token usage.
    pub fn update_tokens(&mut self, usage: TokenUsage) {
        self.token_usage.input_tokens += usage.input_tokens;
        self.token_usage.output_tokens += usage.output_tokens;
        if let Some(cache) = usage.cache_read_tokens {
            *self.token_usage.cache_read_tokens.get_or_insert(0) += cache;
        }
    }

    /// Set the thinking budget.
    pub fn set_thinking_budget(&mut self, budget: Option<i32>) {
        self.thinking_budget_tokens = budget;
    }

    /// Reset thinking tokens used (call at start of turn).
    pub fn reset_thinking_tokens(&mut self) {
        self.thinking_tokens_used = 0;
    }

    /// Add thinking tokens used.
    pub fn add_thinking_tokens(&mut self, tokens: i32) {
        self.thinking_tokens_used += tokens;
    }

    /// Get remaining thinking budget (if budget is set).
    pub fn thinking_budget_remaining(&self) -> Option<i32> {
        self.thinking_budget_tokens
            .map(|budget| (budget - self.thinking_tokens_used).max(0))
    }

    /// Set the current plan phase.
    pub fn set_plan_phase(&mut self, phase: Option<PlanPhase>) {
        self.plan_phase = phase;
    }

    /// Add or update an MCP server status.
    pub fn update_mcp_server(&mut self, name: String, connected: bool, tool_count: i32) {
        if let Some(server) = self.mcp_servers.iter_mut().find(|s| s.name == name) {
            server.connected = connected;
            server.tool_count = tool_count;
        } else {
            self.mcp_servers
                .push(McpServerStatus::new(name, connected, tool_count));
        }
    }

    /// Remove an MCP server.
    pub fn remove_mcp_server(&mut self, name: &str) {
        self.mcp_servers.retain(|s| s.name != name);
    }

    /// Get the count of connected MCP servers.
    pub fn connected_mcp_count(&self) -> i32 {
        self.mcp_servers.iter().filter(|s| s.connected).count() as i32
    }

    /// Start a tool execution.
    pub fn start_tool(&mut self, call_id: String, name: String) {
        self.tool_executions.push(ToolExecution {
            call_id,
            name,
            status: ToolStatus::Running,
            progress: None,
            output: None,
        });
    }

    /// Update tool progress.
    pub fn update_tool_progress(&mut self, call_id: &str, progress: String) {
        if let Some(tool) = self
            .tool_executions
            .iter_mut()
            .find(|t| t.call_id == call_id)
        {
            tool.progress = Some(progress);
        }
    }

    /// Complete a tool execution.
    pub fn complete_tool(&mut self, call_id: &str, output: String, is_error: bool) {
        if let Some(tool) = self
            .tool_executions
            .iter_mut()
            .find(|t| t.call_id == call_id)
        {
            tool.status = if is_error {
                ToolStatus::Failed
            } else {
                ToolStatus::Completed
            };
            tool.output = Some(output);
        }
    }

    /// Remove completed tools older than a certain threshold.
    pub fn cleanup_completed_tools(&mut self, max_completed: usize) {
        let completed_count = self
            .tool_executions
            .iter()
            .filter(|t| matches!(t.status, ToolStatus::Completed | ToolStatus::Failed))
            .count();

        if completed_count > max_completed {
            let to_remove = completed_count - max_completed;
            let mut removed = 0;
            self.tool_executions.retain(|t| {
                if removed >= to_remove {
                    return true;
                }
                if matches!(t.status, ToolStatus::Completed | ToolStatus::Failed) {
                    removed += 1;
                    return false;
                }
                true
            });
        }
    }

    /// Start a new subagent.
    pub fn start_subagent(&mut self, agent_id: String, agent_type: String, description: String) {
        self.subagents.push(SubagentInstance {
            id: agent_id,
            agent_type,
            description,
            status: SubagentStatus::Running,
            progress: None,
            result: None,
            output_file: None,
        });
    }

    /// Update subagent progress.
    pub fn update_subagent_progress(&mut self, agent_id: &str, progress: AgentProgress) {
        if let Some(subagent) = self.subagents.iter_mut().find(|s| s.id == agent_id) {
            subagent.progress = Some(progress);
        }
    }

    /// Complete a subagent.
    pub fn complete_subagent(&mut self, agent_id: &str, result: String) {
        if let Some(subagent) = self.subagents.iter_mut().find(|s| s.id == agent_id) {
            subagent.status = SubagentStatus::Completed;
            subagent.result = Some(result);
        }
    }

    /// Mark a subagent as failed.
    pub fn fail_subagent(&mut self, agent_id: &str, error: String) {
        if let Some(subagent) = self.subagents.iter_mut().find(|s| s.id == agent_id) {
            subagent.status = SubagentStatus::Failed;
            subagent.result = Some(error);
        }
    }

    /// Move a subagent to background.
    pub fn background_subagent(&mut self, agent_id: &str, output_file: PathBuf) {
        if let Some(subagent) = self.subagents.iter_mut().find(|s| s.id == agent_id) {
            subagent.status = SubagentStatus::Backgrounded;
            subagent.output_file = Some(output_file);
        }
    }

    /// Check if there are any running subagents.
    pub fn has_running_subagents(&self) -> bool {
        self.subagents
            .iter()
            .any(|s| s.status == SubagentStatus::Running)
    }

    /// Remove completed/failed subagents older than a certain threshold.
    pub fn cleanup_completed_subagents(&mut self, max_completed: usize) {
        let completed_count = self
            .subagents
            .iter()
            .filter(|s| matches!(s.status, SubagentStatus::Completed | SubagentStatus::Failed))
            .count();

        if completed_count > max_completed {
            let to_remove = completed_count - max_completed;
            let mut removed = 0;
            self.subagents.retain(|s| {
                if removed >= to_remove {
                    return true;
                }
                if matches!(s.status, SubagentStatus::Completed | SubagentStatus::Failed) {
                    removed += 1;
                    return false;
                }
                true
            });
        }
    }

    // ========== Queue Management ==========

    /// Queue a visible command for later processing (Enter during streaming).
    ///
    /// Returns the command ID.
    pub fn queue_command(&mut self, prompt: impl Into<String>) -> String {
        let cmd = UserQueuedCommand::new(prompt);
        let id = cmd.id.clone();
        self.queued_commands.push(cmd);
        id
    }

    /// Dequeue the next command to process.
    pub fn dequeue_command(&mut self) -> Option<UserQueuedCommand> {
        if self.queued_commands.is_empty() {
            None
        } else {
            Some(self.queued_commands.remove(0))
        }
    }

    /// Get the number of queued commands.
    pub fn queued_count(&self) -> i32 {
        self.queued_commands.len() as i32
    }

    /// Clear all queued commands.
    pub fn clear_queues(&mut self) {
        self.queued_commands.clear();
    }

    /// Check if there are any queued commands.
    pub fn has_queued_items(&self) -> bool {
        !self.queued_commands.is_empty()
    }
}

/// A message in the conversation.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// Unique identifier for this message.
    pub id: String,

    /// Role of the message sender.
    pub role: MessageRole,

    /// Content of the message.
    pub content: String,

    /// Whether this message is still being streamed.
    pub streaming: bool,

    /// Thinking content (if applicable).
    pub thinking: Option<String>,
}

impl ChatMessage {
    /// Create a new user message.
    pub fn user(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            role: MessageRole::User,
            content: content.into(),
            streaming: false,
            thinking: None,
        }
    }

    /// Create a new assistant message.
    pub fn assistant(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            role: MessageRole::Assistant,
            content: content.into(),
            streaming: false,
            thinking: None,
        }
    }

    /// Create a new streaming assistant message.
    pub fn streaming_assistant(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            role: MessageRole::Assistant,
            content: String::new(),
            streaming: true,
            thinking: None,
        }
    }

    /// Append content to the message.
    pub fn append(&mut self, delta: &str) {
        self.content.push_str(delta);
    }

    /// Append thinking content.
    pub fn append_thinking(&mut self, delta: &str) {
        self.thinking
            .get_or_insert_with(String::new)
            .push_str(delta);
    }

    /// Mark the message as complete (no longer streaming).
    pub fn complete(&mut self) {
        self.streaming = false;
    }
}

/// Role of a message sender.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    /// User (human) message.
    User,
    /// Assistant (AI) message.
    Assistant,
    /// System message.
    System,
}

/// Status of a tool execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolStatus {
    /// Tool is currently running.
    Running,
    /// Tool completed successfully.
    Completed,
    /// Tool failed with an error.
    Failed,
}

/// A tool execution in progress or completed.
#[derive(Debug, Clone)]
pub struct ToolExecution {
    /// Call identifier.
    pub call_id: String,
    /// Tool name.
    pub name: String,
    /// Current status.
    pub status: ToolStatus,
    /// Progress message (if available).
    pub progress: Option<String>,
    /// Output (when completed).
    pub output: Option<String>,
}

/// Status of a subagent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubagentStatus {
    /// Subagent is currently running.
    Running,
    /// Subagent completed successfully.
    Completed,
    /// Subagent failed with an error.
    Failed,
    /// Subagent was moved to background.
    Backgrounded,
}

/// Phase of plan mode workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanPhase {
    /// Understanding the requirements.
    Understanding,
    /// Designing the solution.
    Design,
    /// Reviewing existing code.
    Review,
    /// Creating the plan.
    Planning,
    /// Waiting for user approval.
    Approval,
}

impl PlanPhase {
    /// Get a short display name for this phase.
    pub fn display_name(&self) -> &'static str {
        match self {
            PlanPhase::Understanding => "Understanding",
            PlanPhase::Design => "Design",
            PlanPhase::Review => "Review",
            PlanPhase::Planning => "Planning",
            PlanPhase::Approval => "Approval",
        }
    }

    /// Get a short emoji indicator for this phase.
    pub fn emoji(&self) -> &'static str {
        match self {
            PlanPhase::Understanding => "🔍",
            PlanPhase::Design => "🎨",
            PlanPhase::Review => "📖",
            PlanPhase::Planning => "📝",
            PlanPhase::Approval => "✅",
        }
    }
}

/// Status of a connected MCP server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServerStatus {
    /// Name of the MCP server.
    pub name: String,
    /// Whether the server is connected.
    pub connected: bool,
    /// Number of tools provided by this server.
    pub tool_count: i32,
}

impl McpServerStatus {
    /// Create a new MCP server status.
    pub fn new(name: impl Into<String>, connected: bool, tool_count: i32) -> Self {
        Self {
            name: name.into(),
            connected,
            tool_count,
        }
    }
}

/// A subagent instance spawned by the main agent.
#[derive(Debug, Clone)]
pub struct SubagentInstance {
    /// Unique identifier for this subagent.
    pub id: String,
    /// Type of agent (e.g., "Explore", "Plan").
    pub agent_type: String,
    /// Short description of what the agent is doing.
    pub description: String,
    /// Current status.
    pub status: SubagentStatus,
    /// Progress information (if available).
    pub progress: Option<AgentProgress>,
    /// Result from the agent (when completed).
    pub result: Option<String>,
    /// Path to output file (when backgrounded).
    pub output_file: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_state_default() {
        let state = SessionState::default();
        assert!(state.messages.is_empty());
        assert!(!state.plan_mode);
        assert!(state.tool_executions.is_empty());
    }

    #[test]
    fn test_add_message() {
        let mut state = SessionState::default();
        state.add_message(ChatMessage::user("1", "Hello"));
        assert_eq!(state.messages.len(), 1);
        assert_eq!(
            state.last_message().map(|m| m.content.as_str()),
            Some("Hello")
        );
    }

    #[test]
    fn test_chat_message_streaming() {
        let mut msg = ChatMessage::streaming_assistant("1");
        assert!(msg.streaming);
        assert!(msg.content.is_empty());

        msg.append("Hello ");
        msg.append("World");
        assert_eq!(msg.content, "Hello World");

        msg.complete();
        assert!(!msg.streaming);
    }

    #[test]
    fn test_tool_lifecycle() {
        let mut state = SessionState::default();

        state.start_tool("call-1".to_string(), "bash".to_string());
        assert_eq!(state.tool_executions.len(), 1);
        assert_eq!(state.tool_executions[0].status, ToolStatus::Running);

        state.update_tool_progress("call-1", "Running...".to_string());
        assert_eq!(
            state.tool_executions[0].progress,
            Some("Running...".to_string())
        );

        state.complete_tool("call-1", "Success".to_string(), false);
        assert_eq!(state.tool_executions[0].status, ToolStatus::Completed);
        assert_eq!(state.tool_executions[0].output, Some("Success".to_string()));
    }

    #[test]
    fn test_cleanup_completed_tools() {
        let mut state = SessionState::default();

        // Add 5 completed tools
        for i in 0..5 {
            state.tool_executions.push(ToolExecution {
                call_id: format!("call-{i}"),
                name: "test".to_string(),
                status: ToolStatus::Completed,
                progress: None,
                output: None,
            });
        }

        // Keep only 2
        state.cleanup_completed_tools(2);
        assert_eq!(state.tool_executions.len(), 2);
    }

    #[test]
    fn test_thinking_budget() {
        let mut state = SessionState::default();

        // Initially no budget
        assert!(state.thinking_budget_tokens.is_none());
        assert_eq!(state.thinking_tokens_used, 0);
        assert!(state.thinking_budget_remaining().is_none());

        // Set a budget
        state.set_thinking_budget(Some(10000));
        assert_eq!(state.thinking_budget_remaining(), Some(10000));

        // Add tokens used
        state.add_thinking_tokens(3000);
        assert_eq!(state.thinking_tokens_used, 3000);
        assert_eq!(state.thinking_budget_remaining(), Some(7000));

        // Add more tokens
        state.add_thinking_tokens(5000);
        assert_eq!(state.thinking_tokens_used, 8000);
        assert_eq!(state.thinking_budget_remaining(), Some(2000));

        // Reset tokens
        state.reset_thinking_tokens();
        assert_eq!(state.thinking_tokens_used, 0);
        assert_eq!(state.thinking_budget_remaining(), Some(10000));

        // Over budget should return 0
        state.add_thinking_tokens(15000);
        assert_eq!(state.thinking_budget_remaining(), Some(0));
    }
}
