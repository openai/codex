//! Top-level agent executor that drives a single agent session.

use std::sync::Arc;

use cocode_api::ApiClient;
use cocode_api::ModelHub;
use cocode_context::ConversationContext;
use cocode_context::EnvironmentInfo;
use cocode_hooks::HookRegistry;
use cocode_loop::AgentLoop;
use cocode_loop::CompactionConfig;
use cocode_loop::FallbackConfig;
use cocode_loop::LoopConfig;
use cocode_loop::LoopResult;
use cocode_protocol::LoopEvent;
use cocode_tools::SpawnAgentFn;
use cocode_tools::ToolRegistry;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::info;

/// Configuration for the agent executor.
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// Model name for logging/tracking.
    pub model: String,
    /// Maximum number of turns before stopping.
    pub max_turns: Option<i32>,
    /// Context window size.
    pub context_window: i32,
    /// Output token limit.
    pub output_token_limit: i32,
    /// Auto-compact threshold (0.0-1.0).
    pub auto_compact_threshold: f32,
    /// Enable micro-compaction.
    pub enable_micro_compaction: bool,
    /// Enable streaming tools.
    pub enable_streaming_tools: bool,
    /// Feature flags propagated to subagent tool executors.
    pub features: cocode_protocol::Features,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            model: "unknown".to_string(),
            max_turns: Some(200),
            context_window: 200_000,
            output_token_limit: 16_384,
            auto_compact_threshold: 0.8,
            enable_micro_compaction: true,
            enable_streaming_tools: true,
            features: cocode_protocol::Features::with_defaults(),
        }
    }
}

/// Top-level agent executor that drives a single agent session.
///
/// The executor wires together the ApiClient, ToolRegistry, and AgentLoop
/// to process user prompts and return final results.
pub struct AgentExecutor {
    /// Unique session identifier.
    session_id: String,

    /// API client for model inference.
    api_client: ApiClient,

    /// Model hub for model resolution.
    model_hub: Arc<ModelHub>,

    /// Tool registry for tool execution.
    tool_registry: Arc<ToolRegistry>,

    /// Hook registry for event interception.
    hooks: Arc<HookRegistry>,

    /// Executor configuration.
    config: ExecutorConfig,

    /// Cancellation token for graceful shutdown.
    cancel_token: CancellationToken,

    /// Optional callback for spawning subagents (used by Task tool).
    spawn_agent_fn: Option<SpawnAgentFn>,

    /// Pre-configured permission rules loaded from settings files.
    permission_rules: Vec<cocode_tools::PermissionRule>,
}

impl AgentExecutor {
    /// Create a new executor with the given API client, model hub, and tool registry.
    pub fn new(
        api_client: ApiClient,
        model_hub: Arc<ModelHub>,
        tool_registry: Arc<ToolRegistry>,
        config: ExecutorConfig,
    ) -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            api_client,
            model_hub,
            tool_registry,
            hooks: Arc::new(HookRegistry::new()),
            config,
            cancel_token: CancellationToken::new(),
            spawn_agent_fn: None,
            permission_rules: Vec::new(),
        }
    }

    /// Create a builder for configuring the executor.
    pub fn builder() -> ExecutorBuilder {
        ExecutorBuilder::new()
    }

    /// Get the session ID.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Get the model name.
    pub fn model(&self) -> &str {
        &self.config.model
    }

    /// Set the hook registry.
    pub fn with_hooks(mut self, hooks: Arc<HookRegistry>) -> Self {
        self.hooks = hooks;
        self
    }

    /// Set the cancellation token.
    pub fn with_cancel_token(mut self, token: CancellationToken) -> Self {
        self.cancel_token = token;
        self
    }

    /// Set the spawn agent callback for the Task tool.
    pub fn with_spawn_agent_fn(mut self, f: SpawnAgentFn) -> Self {
        self.spawn_agent_fn = Some(f);
        self
    }

    /// Execute the agent with the given prompt.
    ///
    /// Returns the final output text from the agent.
    pub async fn execute(&self, prompt: &str) -> anyhow::Result<String> {
        info!(
            session_id = %self.session_id,
            model = %self.config.model,
            prompt_len = prompt.len(),
            "Executing agent"
        );

        // Create event channel
        let (event_tx, mut event_rx) = mpsc::channel::<LoopEvent>(256);

        // Spawn a task to log events (in production, this would update UI)
        let _event_task = tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                match &event {
                    LoopEvent::TurnStarted {
                        turn_id,
                        turn_number,
                    } => {
                        tracing::debug!(turn_id, turn_number, "Turn started");
                    }
                    LoopEvent::TurnCompleted { turn_id, usage } => {
                        tracing::debug!(
                            turn_id,
                            input_tokens = usage.input_tokens,
                            output_tokens = usage.output_tokens,
                            "Turn completed"
                        );
                    }
                    LoopEvent::ToolUseQueued { name, call_id, .. } => {
                        tracing::debug!(name, call_id, "Tool queued");
                    }
                    LoopEvent::Error { error } => {
                        tracing::error!(code = %error.code, message = %error.message, "Loop error");
                    }
                    _ => {}
                }
            }
        });

        // Build environment info
        let environment = EnvironmentInfo::builder()
            .cwd(std::env::current_dir().unwrap_or_default())
            .model(&self.config.model)
            .context_window(self.config.context_window)
            .output_token_limit(self.config.output_token_limit)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build environment: {e}"))?;

        // Build conversation context
        let context = ConversationContext::builder()
            .environment(environment)
            .tool_names(self.tool_registry.tool_names())
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build context: {e}"))?;

        // Build loop config
        let loop_config = LoopConfig {
            max_turns: self.config.max_turns,
            auto_compact_threshold: self.config.auto_compact_threshold,
            enable_micro_compaction: self.config.enable_micro_compaction,
            enable_streaming_tools: self.config.enable_streaming_tools,
            ..LoopConfig::default()
        };

        // Build and run the agent loop
        let mut builder = AgentLoop::builder()
            .api_client(self.api_client.clone())
            .model_hub(self.model_hub.clone())
            .tool_registry(self.tool_registry.clone())
            .context(context)
            .config(loop_config)
            .fallback_config(FallbackConfig::default())
            .compaction_config(CompactionConfig::default())
            .hooks(self.hooks.clone())
            .event_tx(event_tx)
            .cancel_token(self.cancel_token.clone())
            .features(self.config.features.clone())
            .permission_rules(self.permission_rules.clone());

        // Add spawn_agent_fn if available for Task tool
        if let Some(ref spawn_fn) = self.spawn_agent_fn {
            builder = builder.spawn_agent_fn(spawn_fn.clone());
        }

        let mut loop_instance = builder.build();

        let result = loop_instance.run(prompt).await?;

        self.format_result(&result)
    }

    /// Format the loop result as a string.
    fn format_result(&self, result: &LoopResult) -> anyhow::Result<String> {
        // Return the final text from the model
        if result.final_text.is_empty() {
            // If no text, provide a summary
            Ok(format!(
                "Completed {} turns. Input tokens: {}, Output tokens: {}",
                result.turns_completed, result.total_input_tokens, result.total_output_tokens
            ))
        } else {
            Ok(result.final_text.clone())
        }
    }

    /// Cancel the execution.
    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }
}

/// Builder for creating an [`AgentExecutor`].
pub struct ExecutorBuilder {
    api_client: Option<ApiClient>,
    model_hub: Option<Arc<ModelHub>>,
    tool_registry: Option<Arc<ToolRegistry>>,
    hooks: Option<Arc<HookRegistry>>,
    config: ExecutorConfig,
    cancel_token: CancellationToken,
    spawn_agent_fn: Option<SpawnAgentFn>,
    features: cocode_protocol::Features,
    permission_rules: Vec<cocode_tools::PermissionRule>,
}

impl ExecutorBuilder {
    /// Create a new builder with defaults.
    pub fn new() -> Self {
        Self {
            api_client: None,
            model_hub: None,
            tool_registry: None,
            hooks: None,
            config: ExecutorConfig::default(),
            cancel_token: CancellationToken::new(),
            spawn_agent_fn: None,
            features: cocode_protocol::Features::with_defaults(),
            permission_rules: Vec::new(),
        }
    }

    /// Set the model hub.
    pub fn model_hub(mut self, hub: Arc<ModelHub>) -> Self {
        self.model_hub = Some(hub);
        self
    }

    /// Set the API client.
    pub fn api_client(mut self, client: ApiClient) -> Self {
        self.api_client = Some(client);
        self
    }

    /// Set the tool registry.
    pub fn tool_registry(mut self, registry: Arc<ToolRegistry>) -> Self {
        self.tool_registry = Some(registry);
        self
    }

    /// Set the hook registry.
    pub fn hooks(mut self, hooks: Arc<HookRegistry>) -> Self {
        self.hooks = Some(hooks);
        self
    }

    /// Set the model name.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.config.model = model.into();
        self
    }

    /// Set the maximum turns.
    pub fn max_turns(mut self, max: i32) -> Self {
        self.config.max_turns = Some(max);
        self
    }

    /// Set the context window size.
    pub fn context_window(mut self, size: i32) -> Self {
        self.config.context_window = size;
        self
    }

    /// Set the output token limit.
    pub fn output_token_limit(mut self, limit: i32) -> Self {
        self.config.output_token_limit = limit;
        self
    }

    /// Set the auto-compact threshold.
    pub fn auto_compact_threshold(mut self, threshold: f32) -> Self {
        self.config.auto_compact_threshold = threshold;
        self
    }

    /// Enable or disable micro-compaction.
    pub fn enable_micro_compaction(mut self, enabled: bool) -> Self {
        self.config.enable_micro_compaction = enabled;
        self
    }

    /// Enable or disable streaming tools.
    pub fn enable_streaming_tools(mut self, enabled: bool) -> Self {
        self.config.enable_streaming_tools = enabled;
        self
    }

    /// Set the cancellation token.
    pub fn cancel_token(mut self, token: CancellationToken) -> Self {
        self.cancel_token = token;
        self
    }

    /// Set the spawn agent callback for the Task tool.
    pub fn spawn_agent_fn(mut self, f: SpawnAgentFn) -> Self {
        self.spawn_agent_fn = Some(f);
        self
    }

    /// Set the feature flags for subagent tool executors.
    pub fn features(mut self, features: cocode_protocol::Features) -> Self {
        self.features = features;
        self
    }

    /// Set pre-configured permission rules.
    pub fn permission_rules(mut self, rules: Vec<cocode_tools::PermissionRule>) -> Self {
        self.permission_rules = rules;
        self
    }

    /// Build the executor.
    ///
    /// # Panics
    /// Panics if `api_client`, `model_hub`, or `tool_registry` have not been set.
    pub fn build(self) -> AgentExecutor {
        let mut config = self.config;
        config.features = self.features;

        let mut executor = AgentExecutor::new(
            self.api_client.expect("api_client is required"),
            self.model_hub.expect("model_hub is required"),
            self.tool_registry.expect("tool_registry is required"),
            config,
        );

        if let Some(hooks) = self.hooks {
            executor.hooks = hooks;
        }

        executor.cancel_token = self.cancel_token;
        executor.spawn_agent_fn = self.spawn_agent_fn;
        executor.permission_rules = self.permission_rules;
        executor
    }
}

impl Default for ExecutorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_config_defaults() {
        let config = ExecutorConfig::default();
        assert_eq!(config.max_turns, Some(200));
        assert_eq!(config.context_window, 200_000);
        assert_eq!(config.output_token_limit, 16_384);
        assert!((config.auto_compact_threshold - 0.8).abs() < f32::EPSILON);
        assert!(config.enable_micro_compaction);
        assert!(config.enable_streaming_tools);
        assert_eq!(config.features, cocode_protocol::Features::with_defaults());
    }

    #[test]
    fn test_builder_defaults() {
        let builder = ExecutorBuilder::new();
        assert!(builder.api_client.is_none());
        assert!(builder.model_hub.is_none());
        assert!(builder.tool_registry.is_none());
        assert!(builder.hooks.is_none());
        assert!(builder.spawn_agent_fn.is_none());
    }

    #[test]
    fn test_builder_configuration() {
        let builder = ExecutorBuilder::new()
            .model("test-model")
            .max_turns(100)
            .context_window(128000)
            .output_token_limit(8192)
            .auto_compact_threshold(0.7)
            .enable_micro_compaction(false)
            .enable_streaming_tools(false);

        assert_eq!(builder.config.model, "test-model");
        assert_eq!(builder.config.max_turns, Some(100));
        assert_eq!(builder.config.context_window, 128000);
        assert_eq!(builder.config.output_token_limit, 8192);
        assert!((builder.config.auto_compact_threshold - 0.7).abs() < f32::EPSILON);
        assert!(!builder.config.enable_micro_compaction);
        assert!(!builder.config.enable_streaming_tools);
    }
}
