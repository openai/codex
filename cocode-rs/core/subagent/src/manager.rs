use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use cocode_protocol::execution::ExecutionIdentity;
use serde::Deserialize;
use serde::Serialize;
use tokio_util::sync::CancellationToken;

use crate::background::BackgroundAgent;
use crate::definition::AgentDefinition;
use crate::filter::filter_tools_for_agent;
use crate::spawn::SpawnInput;

/// Runtime status of a subagent instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    Running,
    Completed,
    Failed,
    Backgrounded,
}

/// Result of spawning a subagent.
#[derive(Debug, Clone)]
pub struct SpawnResult {
    /// Unique identifier for the spawned agent.
    pub agent_id: String,

    /// Final output (only for foreground agents that completed).
    pub output: Option<String>,

    /// Background agent info (only for background agents).
    pub background: Option<BackgroundAgent>,
}

/// A live subagent instance.
pub struct AgentInstance {
    /// Unique identifier for this instance.
    pub id: String,

    /// The agent type this instance was spawned from.
    pub agent_type: String,

    /// Current execution status.
    pub status: AgentStatus,

    /// Final output text (populated on completion).
    pub output: Option<String>,

    /// Cancellation token for aborting the agent.
    pub cancel_token: Option<CancellationToken>,

    /// Background output file path (if running in background).
    pub output_file: Option<PathBuf>,
}

/// Callback type for executing an agent with filtered tools.
///
/// The callback receives:
/// - `agent_type`: The type of agent being spawned
/// - `prompt`: The task prompt for the agent
/// - `identity`: Optional execution identity for model selection
/// - `max_turns`: Optional turn limit override
/// - `tools`: Filtered list of available tool names
/// - `cancel_token`: Token for cancellation
///
/// Returns the agent output as a string on success.
pub type AgentExecuteFn = Box<
    dyn Fn(
            String,                    // agent_type
            String,                    // prompt
            Option<ExecutionIdentity>, // identity
            Option<i32>,               // max_turns
            Vec<String>,               // filtered tools
            CancellationToken,         // cancel_token
        )
            -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<String>> + Send>>
        + Send
        + Sync,
>;

/// Manages subagent registration, spawning, and lifecycle tracking.
pub struct SubagentManager {
    agents: HashMap<String, AgentInstance>,
    definitions: Vec<AgentDefinition>,
    /// All available tool names (used for filtering).
    all_tools: Vec<String>,
    /// Optional callback for actual agent execution.
    execute_fn: Option<Arc<AgentExecuteFn>>,
    /// Base directory for background agent output files.
    output_dir: PathBuf,
}

impl SubagentManager {
    /// Create a new empty subagent manager.
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
            definitions: Vec::new(),
            all_tools: Vec::new(),
            execute_fn: None,
            output_dir: std::env::temp_dir().join("cocode-agents"),
        }
    }

    /// Set the available tool names for filtering.
    pub fn with_tools(mut self, tools: Vec<String>) -> Self {
        self.all_tools = tools;
        self
    }

    /// Set the agent execution callback.
    pub fn with_execute_fn(mut self, f: AgentExecuteFn) -> Self {
        self.execute_fn = Some(Arc::new(f));
        self
    }

    /// Set the output directory for background agents.
    pub fn with_output_dir(mut self, dir: PathBuf) -> Self {
        self.output_dir = dir;
        self
    }

    /// Register a new agent type definition.
    pub fn register_agent_type(&mut self, definition: AgentDefinition) {
        tracing::info!(agent_type = %definition.agent_type, "Registering agent type");
        self.definitions.push(definition);
    }

    /// Spawn a new subagent instance of the given type (simple version).
    ///
    /// Returns the unique agent ID on success. This is a basic spawn that
    /// just registers the agent without executing it.
    pub async fn spawn(&mut self, agent_type: &str, prompt: &str) -> anyhow::Result<String> {
        let input = SpawnInput {
            agent_type: agent_type.to_string(),
            prompt: prompt.to_string(),
            identity: None,
            max_turns: None,
            run_in_background: false,
            allowed_tools: None,
        };
        let result = self.spawn_full(input).await?;
        Ok(result.agent_id)
    }

    /// Spawn a subagent with full configuration and tool filtering.
    ///
    /// This is the main entry point for spawning subagents:
    /// 1. Resolves the agent definition
    /// 2. Filters tools based on definition and spawn input
    /// 3. Executes the agent (foreground or background)
    /// 4. Returns the result
    pub async fn spawn_full(&mut self, input: SpawnInput) -> anyhow::Result<SpawnResult> {
        let definition = self
            .definitions
            .iter()
            .find(|d| d.agent_type == input.agent_type)
            .ok_or_else(|| anyhow::anyhow!("Unknown agent type: {}", input.agent_type))?
            .clone();

        let agent_id = uuid::Uuid::new_v4().to_string();
        tracing::info!(
            agent_id = %agent_id,
            agent_type = %input.agent_type,
            prompt_len = input.prompt.len(),
            background = input.run_in_background,
            "Spawning subagent"
        );

        // Resolve identity (spawn input > definition > inherit parent)
        // Priority: input.identity > definition.identity > None (inherit)
        let identity = input
            .identity
            .clone()
            .or_else(|| definition.identity.clone());

        // Resolve max_turns (spawn input > definition)
        let max_turns = input.max_turns.or(definition.max_turns);

        // Apply three-layer tool filtering
        let tools_to_filter = if let Some(ref allowed) = input.allowed_tools {
            // If spawn input specifies tools, use those as the base
            allowed.clone()
        } else {
            self.all_tools.clone()
        };
        let filtered_tools =
            filter_tools_for_agent(&tools_to_filter, &definition, input.run_in_background);

        tracing::debug!(
            agent_id = %agent_id,
            tools_count = filtered_tools.len(),
            "Filtered tools for subagent"
        );

        // Create cancellation token for this agent
        let cancel_token = CancellationToken::new();

        if input.run_in_background {
            // Background execution
            let output_file = self.output_dir.join(format!("{agent_id}.jsonl"));

            // Ensure output directory exists
            if let Err(e) = tokio::fs::create_dir_all(&self.output_dir).await {
                tracing::warn!(error = %e, "Failed to create output directory");
            }

            let instance = AgentInstance {
                id: agent_id.clone(),
                agent_type: input.agent_type.clone(),
                status: AgentStatus::Backgrounded,
                output: None,
                cancel_token: Some(cancel_token.clone()),
                output_file: Some(output_file.clone()),
            };
            self.agents.insert(agent_id.clone(), instance);

            // Spawn background task if we have an execute function
            if let Some(execute_fn) = &self.execute_fn {
                let execute_fn = execute_fn.clone();
                let agent_id_clone = agent_id.clone();
                let prompt = input.prompt.clone();
                let agent_type = input.agent_type.clone();
                let output_file_clone = output_file.clone();

                tokio::spawn(async move {
                    let result = execute_fn(
                        agent_type,
                        prompt,
                        identity,
                        max_turns,
                        filtered_tools,
                        cancel_token,
                    )
                    .await;

                    // Write result to output file
                    let entry = match &result {
                        Ok(output) => serde_json::json!({
                            "status": "completed",
                            "agent_id": agent_id_clone,
                            "output": output
                        }),
                        Err(e) => serde_json::json!({
                            "status": "failed",
                            "agent_id": agent_id_clone,
                            "error": e.to_string()
                        }),
                    };
                    if let Err(e) = tokio::fs::write(
                        &output_file_clone,
                        serde_json::to_string_pretty(&entry).unwrap_or_default(),
                    )
                    .await
                    {
                        tracing::error!(error = %e, "Failed to write agent output");
                    }
                });
            }

            let bg_agent = BackgroundAgent {
                agent_id: agent_id.clone(),
                output_file,
            };

            Ok(SpawnResult {
                agent_id,
                output: None,
                background: Some(bg_agent),
            })
        } else {
            // Foreground execution
            let instance = AgentInstance {
                id: agent_id.clone(),
                agent_type: input.agent_type.clone(),
                status: AgentStatus::Running,
                output: None,
                cancel_token: Some(cancel_token.clone()),
                output_file: None,
            };
            self.agents.insert(agent_id.clone(), instance);

            // Register for background signal (Ctrl+B support)
            let bg_signal_rx = crate::signal::register_backgroundable_agent(agent_id.clone());

            // Execute the agent if we have an execute function
            let output = if let Some(execute_fn) = &self.execute_fn {
                let execute_future = execute_fn(
                    input.agent_type.clone(),
                    input.prompt.clone(),
                    identity.clone(),
                    max_turns,
                    filtered_tools.clone(),
                    cancel_token.clone(),
                );

                // Use select! to handle both normal completion and background signal
                tokio::select! {
                    result = execute_future => {
                        // Normal completion - unregister from background signals
                        crate::signal::unregister_backgroundable_agent(&agent_id);

                        match result {
                            Ok(result) => {
                                if let Some(instance) = self.agents.get_mut(&agent_id) {
                                    instance.status = AgentStatus::Completed;
                                    instance.output = Some(result.clone());
                                }
                                Some(result)
                            }
                            Err(e) => {
                                if let Some(instance) = self.agents.get_mut(&agent_id) {
                                    instance.status = AgentStatus::Failed;
                                }
                                return Err(e);
                            }
                        }
                    }
                    _ = bg_signal_rx => {
                        // Background signal received - transition to background
                        tracing::info!(
                            agent_id = %agent_id,
                            "Agent transitioned to background via signal"
                        );

                        // Create output file for background results
                        let output_file = self.output_dir.join(format!("{agent_id}.jsonl"));

                        // Update instance to background status
                        if let Some(instance) = self.agents.get_mut(&agent_id) {
                            instance.status = AgentStatus::Backgrounded;
                            instance.output_file = Some(output_file.clone());
                        }

                        // Spawn background task to continue execution
                        let execute_fn = execute_fn.clone();
                        let agent_id_clone = agent_id.clone();
                        let agent_type = input.agent_type.clone();
                        let prompt = input.prompt.clone();
                        let output_file_clone = output_file.clone();

                        tokio::spawn(async move {
                            let result = execute_fn(
                                agent_type,
                                prompt,
                                identity,
                                max_turns,
                                filtered_tools,
                                cancel_token,
                            )
                            .await;

                            // Write result to output file
                            let entry = match &result {
                                Ok(output) => serde_json::json!({
                                    "status": "completed",
                                    "agent_id": agent_id_clone,
                                    "output": output,
                                    "transitioned_from_foreground": true
                                }),
                                Err(e) => serde_json::json!({
                                    "status": "failed",
                                    "agent_id": agent_id_clone,
                                    "error": e.to_string(),
                                    "transitioned_from_foreground": true
                                }),
                            };
                            if let Err(e) = tokio::fs::write(
                                &output_file_clone,
                                serde_json::to_string_pretty(&entry).unwrap_or_default(),
                            )
                            .await
                            {
                                tracing::error!(error = %e, "Failed to write agent output");
                            }
                        });

                        let bg_agent = BackgroundAgent {
                            agent_id: agent_id.clone(),
                            output_file,
                        };

                        return Ok(SpawnResult {
                            agent_id,
                            output: None,
                            background: Some(bg_agent),
                        });
                    }
                }
            } else {
                // No execute function - return stub (no background signal handling)
                crate::signal::unregister_backgroundable_agent(&agent_id);
                tracing::warn!(
                    agent_id = %agent_id,
                    "No execute_fn configured, returning stub response"
                );
                let stub_output = format!(
                    "Agent '{}' completed task (stub - no executor configured)",
                    input.agent_type
                );
                if let Some(instance) = self.agents.get_mut(&agent_id) {
                    instance.status = AgentStatus::Completed;
                    instance.output = Some(stub_output.clone());
                }
                Some(stub_output)
            };

            Ok(SpawnResult {
                agent_id,
                output,
                background: None,
            })
        }
    }

    /// Resume a previously backgrounded agent.
    pub async fn resume(&mut self, agent_id: &str) -> anyhow::Result<String> {
        let instance = self
            .agents
            .get_mut(agent_id)
            .ok_or_else(|| anyhow::anyhow!("Agent not found: {agent_id}"))?;

        if instance.status != AgentStatus::Backgrounded {
            anyhow::bail!(
                "Agent {agent_id} is not backgrounded (status: {:?})",
                instance.status
            );
        }

        tracing::info!(agent_id, "Resuming backgrounded agent");
        instance.status = AgentStatus::Running;
        Ok(agent_id.to_string())
    }

    /// Get the output of a completed agent.
    pub async fn get_output(&self, agent_id: &str) -> Option<String> {
        self.agents.get(agent_id).and_then(|a| a.output.clone())
    }

    /// Get the current status of an agent.
    pub fn get_status(&self, agent_id: &str) -> Option<AgentStatus> {
        self.agents.get(agent_id).map(|a| a.status.clone())
    }
}

impl Default for SubagentManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_definition(name: &str) -> AgentDefinition {
        AgentDefinition {
            name: name.to_string(),
            description: format!("{name} agent"),
            agent_type: name.to_string(),
            tools: vec![],
            disallowed_tools: vec![],
            identity: None,
            max_turns: None,
        }
    }

    #[test]
    fn test_new_manager() {
        let mgr = SubagentManager::new();
        assert!(mgr.agents.is_empty());
        assert!(mgr.definitions.is_empty());
    }

    #[test]
    fn test_register_agent_type() {
        let mut mgr = SubagentManager::new();
        mgr.register_agent_type(test_definition("bash"));
        assert_eq!(mgr.definitions.len(), 1);
        assert_eq!(mgr.definitions[0].agent_type, "bash");
    }

    #[tokio::test]
    async fn test_spawn_agent() {
        let mut mgr = SubagentManager::new();
        mgr.register_agent_type(test_definition("bash"));

        let id = mgr.spawn("bash", "run ls").await.expect("spawn");
        assert!(!id.is_empty());
        // Without an execute_fn, the stub completes immediately
        assert_eq!(mgr.get_status(&id), Some(AgentStatus::Completed));
    }

    #[tokio::test]
    async fn test_spawn_unknown_type() {
        let mut mgr = SubagentManager::new();
        let result = mgr.spawn("nonexistent", "test").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_spawn_full_with_stub() {
        let mut mgr = SubagentManager::new();
        mgr.register_agent_type(test_definition("bash"));

        let input = SpawnInput {
            agent_type: "bash".to_string(),
            prompt: "test".to_string(),
            identity: None,
            max_turns: None,
            run_in_background: false,
            allowed_tools: None,
        };

        let result = mgr.spawn_full(input).await.expect("spawn_full");
        assert!(!result.agent_id.is_empty());
        assert!(result.output.is_some()); // Stub returns output
        assert!(result.background.is_none());
    }

    #[tokio::test]
    async fn test_spawn_full_background() {
        let mut mgr = SubagentManager::new();
        mgr.register_agent_type(test_definition("bash"));

        let input = SpawnInput {
            agent_type: "bash".to_string(),
            prompt: "test".to_string(),
            identity: None,
            max_turns: None,
            run_in_background: true,
            allowed_tools: None,
        };

        let result = mgr.spawn_full(input).await.expect("spawn_full");
        assert!(!result.agent_id.is_empty());
        assert!(result.output.is_none()); // Background has no immediate output
        assert!(result.background.is_some());
        assert_eq!(
            mgr.get_status(&result.agent_id),
            Some(AgentStatus::Backgrounded)
        );
    }

    #[tokio::test]
    async fn test_resume_non_backgrounded() {
        let mut mgr = SubagentManager::new();
        mgr.register_agent_type(test_definition("bash"));
        let id = mgr.spawn("bash", "test").await.expect("spawn");
        let result = mgr.resume(&id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_resume_backgrounded() {
        let mut mgr = SubagentManager::new();
        mgr.register_agent_type(test_definition("bash"));
        let id = mgr.spawn("bash", "test").await.expect("spawn");

        // Manually set to backgrounded for test.
        mgr.agents.get_mut(&id).expect("agent").status = AgentStatus::Backgrounded;

        let resumed_id = mgr.resume(&id).await.expect("resume");
        assert_eq!(resumed_id, id);
        assert_eq!(mgr.get_status(&id), Some(AgentStatus::Running));
    }

    #[test]
    fn test_get_status_missing() {
        let mgr = SubagentManager::new();
        assert!(mgr.get_status("nonexistent").is_none());
    }
}
