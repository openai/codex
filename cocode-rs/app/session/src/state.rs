//! Session state aggregate that wires together all components.
//!
//! [`SessionState`] is the main runtime container for an active session,
//! holding references to the API client, tool registry, hooks, and message history.

use std::sync::Arc;

use cocode_api::ApiClient;
use cocode_config::ConfigManager;
use cocode_context::{ConversationContext, EnvironmentInfo};
use cocode_hooks::HookRegistry;
use cocode_loop::{AgentLoop, CompactionConfig, FallbackConfig, LoopConfig, LoopResult};
use cocode_message::MessageHistory;
use cocode_protocol::{LoopEvent, ProviderType, TokenUsage};
use cocode_skill::SkillInterface;
use cocode_tools::ToolRegistry;
use hyper_sdk::Provider;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

use crate::session::Session;

/// Result of a single turn in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnResult {
    /// Final text response from the model.
    pub final_text: String,

    /// Number of turns completed so far.
    pub turns_completed: i32,

    /// Token usage for this turn.
    pub usage: TokenUsage,

    /// Whether the model requested more tool calls.
    pub has_pending_tools: bool,

    /// Whether the loop completed (model stop signal).
    pub is_complete: bool,
}

impl TurnResult {
    /// Create a turn result from a loop result.
    pub fn from_loop_result(result: &LoopResult) -> Self {
        Self {
            final_text: result.final_text.clone(),
            turns_completed: result.turns_completed,
            usage: TokenUsage {
                input_tokens: result.total_input_tokens,
                output_tokens: result.total_output_tokens,
                cache_read_tokens: None,
                cache_creation_tokens: None,
            },
            has_pending_tools: false,
            is_complete: true,
        }
    }
}

/// Session state aggregate for an active conversation.
///
/// This struct holds all the runtime components needed to drive a conversation:
/// - Session metadata
/// - Message history
/// - Tool registry
/// - Hook registry
/// - Skills
/// - API client
/// - Cancellation token
///
/// # Example
///
/// ```ignore
/// use cocode_session::{Session, SessionState};
/// use cocode_config::ConfigManager;
/// use cocode_protocol::ProviderType;
/// use std::path::PathBuf;
///
/// let session = Session::new(
///     PathBuf::from("."),
///     "gpt-5",
///     ProviderType::Openai,
/// );
///
/// let config = ConfigManager::from_default()?;
/// let mut state = SessionState::new(session, &config).await?;
///
/// // Run a turn
/// let result = state.run_turn("Hello!").await?;
/// println!("Response: {}", result.final_text);
///
/// // Cancel if needed
/// state.cancel();
/// ```
pub struct SessionState {
    /// Session metadata.
    pub session: Session,

    /// Message history for the conversation.
    pub message_history: MessageHistory,

    /// Tool registry (built-in + MCP tools).
    pub tool_registry: Arc<ToolRegistry>,

    /// Hook registry for event interception.
    pub hook_registry: Arc<HookRegistry>,

    /// Loaded skills.
    pub skills: Vec<SkillInterface>,

    /// API client for model inference.
    api_client: ApiClient,

    /// Cancellation token for graceful shutdown.
    cancel_token: CancellationToken,

    /// Loop configuration.
    loop_config: LoopConfig,

    /// Total turns run.
    total_turns: i32,

    /// Total input tokens consumed.
    total_input_tokens: i32,

    /// Total output tokens generated.
    total_output_tokens: i32,

    /// Context window size for the model.
    context_window: i32,
}

impl SessionState {
    /// Create a new session state from a session and configuration.
    ///
    /// This initializes all components including:
    /// - API client from the resolved provider/model
    /// - Tool registry with built-in tools
    /// - Hook registry (empty by default)
    /// - Skills (loaded from project/user directories)
    pub async fn new(session: Session, config: &ConfigManager) -> anyhow::Result<Self> {
        info!(
            session_id = %session.id,
            model = %session.model,
            provider = %session.provider,
            "Creating session state"
        );

        // Resolve provider info
        let provider_info = config.resolve_provider(&session.provider)?;

        // Get model context window (default to 200k if not specified)
        let context_window = provider_info
            .get_model(&session.model)
            .and_then(|m| m.info.context_window)
            .unwrap_or(200_000) as i32;

        // Create API client
        let api_client = Self::create_api_client(&provider_info, &session.model)?;

        // Create tool registry with built-in tools
        let mut tool_registry = ToolRegistry::new();
        cocode_tools::builtin::register_builtin_tools(&mut tool_registry);

        // Create hook registry (empty for now)
        let hook_registry = HookRegistry::new();

        // Load skills (empty for now, can be populated later)
        let skills = Vec::new();

        // Build loop config from session
        let loop_config = LoopConfig {
            max_turns: session.max_turns,
            ..LoopConfig::default()
        };

        Ok(Self {
            session,
            message_history: MessageHistory::new(),
            tool_registry: Arc::new(tool_registry),
            hook_registry: Arc::new(hook_registry),
            skills,
            api_client,
            cancel_token: CancellationToken::new(),
            loop_config,
            total_turns: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            context_window,
        })
    }

    /// Create an API client from provider info and model.
    fn create_api_client(
        provider_info: &cocode_protocol::ProviderInfo,
        model: &str,
    ) -> anyhow::Result<ApiClient> {
        use hyper_sdk::providers::{
            anthropic::AnthropicConfig, gemini::GeminiConfig, openai::OpenAIConfig,
            volcengine::VolcengineConfig, zai::ZaiConfig,
        };

        // Get provider model info
        let provider_model = provider_info.get_model(model).ok_or_else(|| {
            anyhow::anyhow!(
                "Model '{}' not found in provider '{}'",
                model,
                provider_info.name
            )
        })?;

        // Get the actual model name to use for API
        let api_model_name = provider_model.api_model_name();

        // Create provider-specific model
        let model: Arc<dyn hyper_sdk::Model> = match provider_info.provider_type {
            ProviderType::Openai | ProviderType::OpenaiCompat => {
                let config = OpenAIConfig {
                    api_key: provider_info.api_key.clone(),
                    base_url: provider_info.base_url.clone(),
                    ..Default::default()
                };
                let provider = hyper_sdk::OpenAIProvider::new(config)
                    .map_err(|e| anyhow::anyhow!("Failed to create OpenAI provider: {e}"))?;
                provider
                    .model(api_model_name)
                    .map_err(|e| anyhow::anyhow!("Failed to create model: {e}"))?
            }
            ProviderType::Anthropic => {
                let config = AnthropicConfig {
                    api_key: provider_info.api_key.clone(),
                    base_url: provider_info.base_url.clone(),
                    ..Default::default()
                };
                let provider = hyper_sdk::AnthropicProvider::new(config)
                    .map_err(|e| anyhow::anyhow!("Failed to create Anthropic provider: {e}"))?;
                provider
                    .model(api_model_name)
                    .map_err(|e| anyhow::anyhow!("Failed to create model: {e}"))?
            }
            ProviderType::Gemini => {
                let config = GeminiConfig {
                    api_key: provider_info.api_key.clone(),
                    base_url: provider_info.base_url.clone(),
                    ..Default::default()
                };
                let provider = hyper_sdk::GeminiProvider::new(config)
                    .map_err(|e| anyhow::anyhow!("Failed to create Gemini provider: {e}"))?;
                provider
                    .model(api_model_name)
                    .map_err(|e| anyhow::anyhow!("Failed to create model: {e}"))?
            }
            ProviderType::Volcengine => {
                let config = VolcengineConfig {
                    api_key: provider_info.api_key.clone(),
                    base_url: provider_info.base_url.clone(),
                    ..Default::default()
                };
                let provider = hyper_sdk::VolcengineProvider::new(config)
                    .map_err(|e| anyhow::anyhow!("Failed to create Volcengine provider: {e}"))?;
                provider
                    .model(api_model_name)
                    .map_err(|e| anyhow::anyhow!("Failed to create model: {e}"))?
            }
            ProviderType::Zai => {
                let config = ZaiConfig {
                    api_key: provider_info.api_key.clone(),
                    base_url: provider_info.base_url.clone(),
                    ..Default::default()
                };
                let provider = hyper_sdk::ZaiProvider::new(config)
                    .map_err(|e| anyhow::anyhow!("Failed to create Z.AI provider: {e}"))?;
                provider
                    .model(api_model_name)
                    .map_err(|e| anyhow::anyhow!("Failed to create model: {e}"))?
            }
        };

        Ok(ApiClient::new(model))
    }

    /// Run a single turn with the given user input.
    ///
    /// This creates an agent loop and runs it to completion,
    /// returning the result of the conversation turn.
    pub async fn run_turn(&mut self, user_input: &str) -> anyhow::Result<TurnResult> {
        info!(
            session_id = %self.session.id,
            input_len = user_input.len(),
            "Running turn"
        );

        // Update session activity
        self.session.touch();

        // Create event channel
        let (event_tx, mut event_rx) = mpsc::channel::<LoopEvent>(256);

        // Spawn task to handle events (logging for now)
        let cancel_token = self.cancel_token.clone();
        let event_task = tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                if cancel_token.is_cancelled() {
                    break;
                }
                Self::handle_event(&event);
            }
        });

        // Build environment info
        let environment = EnvironmentInfo::builder()
            .cwd(&self.session.working_dir)
            .model(&self.session.model)
            .context_window(self.context_window)
            .output_token_limit(16_384)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build environment: {e}"))?;

        // Build conversation context
        let context = ConversationContext::builder()
            .environment(environment)
            .tool_names(self.tool_registry.tool_names())
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build context: {e}"))?;

        // Build and run the agent loop
        let mut loop_instance = AgentLoop::builder()
            .api_client(self.api_client.clone())
            .tool_registry(self.tool_registry.clone())
            .context(context)
            .config(self.loop_config.clone())
            .fallback_config(FallbackConfig::default())
            .compaction_config(CompactionConfig::default())
            .hooks(self.hook_registry.clone())
            .event_tx(event_tx)
            .cancel_token(self.cancel_token.clone())
            .build();

        let result = loop_instance.run(user_input).await?;

        // Drop the event sender to signal end of events, then wait for task to complete
        drop(loop_instance);
        let _ = event_task.await;

        // Update totals
        self.total_turns += result.turns_completed;
        self.total_input_tokens += result.total_input_tokens;
        self.total_output_tokens += result.total_output_tokens;

        Ok(TurnResult::from_loop_result(&result))
    }

    /// Handle a loop event (logging).
    fn handle_event(event: &LoopEvent) {
        match event {
            LoopEvent::TurnStarted {
                turn_id,
                turn_number,
            } => {
                debug!(turn_id, turn_number, "Turn started");
            }
            LoopEvent::TurnCompleted { turn_id, usage } => {
                debug!(
                    turn_id,
                    input_tokens = usage.input_tokens,
                    output_tokens = usage.output_tokens,
                    "Turn completed"
                );
            }
            LoopEvent::TextDelta { delta, .. } => {
                // In a real implementation, this would stream to UI
                debug!(delta_len = delta.len(), "Text delta");
            }
            LoopEvent::ToolUseQueued { name, call_id, .. } => {
                debug!(name, call_id, "Tool queued");
            }
            LoopEvent::Error { error } => {
                tracing::error!(code = %error.code, message = %error.message, "Loop error");
            }
            _ => {
                debug!(?event, "Loop event");
            }
        }
    }

    /// Cancel the current operation.
    pub fn cancel(&self) {
        info!(session_id = %self.session.id, "Cancelling session");
        self.cancel_token.cancel();
    }

    /// Check if the session is cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancel_token.is_cancelled()
    }

    /// Get the session ID.
    pub fn session_id(&self) -> &str {
        &self.session.id
    }

    /// Get the model name.
    pub fn model(&self) -> &str {
        &self.session.model
    }

    /// Get the provider name.
    pub fn provider(&self) -> &str {
        &self.session.provider
    }

    /// Get total turns run.
    pub fn total_turns(&self) -> i32 {
        self.total_turns
    }

    /// Get total input tokens consumed.
    pub fn total_input_tokens(&self) -> i32 {
        self.total_input_tokens
    }

    /// Get total output tokens generated.
    pub fn total_output_tokens(&self) -> i32 {
        self.total_output_tokens
    }

    /// Get the message history.
    pub fn history(&self) -> &MessageHistory {
        &self.message_history
    }

    /// Get mutable access to the message history.
    pub fn history_mut(&mut self) -> &mut MessageHistory {
        &mut self.message_history
    }

    /// Set the hook registry.
    pub fn set_hooks(&mut self, hooks: Arc<HookRegistry>) {
        self.hook_registry = hooks;
    }

    /// Add a skill to the session.
    pub fn add_skill(&mut self, skill: SkillInterface) {
        self.skills.push(skill);
    }

    /// Get the loaded skills.
    pub fn skills(&self) -> &[SkillInterface] {
        &self.skills
    }

    /// Update the loop configuration.
    pub fn set_loop_config(&mut self, config: LoopConfig) {
        self.loop_config = config;
    }

    /// Get the loop configuration.
    pub fn loop_config(&self) -> &LoopConfig {
        &self.loop_config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper_sdk::ContentBlock;

    #[test]
    fn test_turn_result_from_loop_result() {
        let loop_result = LoopResult::completed(
            3,
            1000,
            500,
            "Hello!".to_string(),
            vec![ContentBlock::text("Hello!")],
        );

        let turn = TurnResult::from_loop_result(&loop_result);
        assert_eq!(turn.final_text, "Hello!");
        assert_eq!(turn.turns_completed, 3);
        assert_eq!(turn.usage.input_tokens, 1000);
        assert_eq!(turn.usage.output_tokens, 500);
        assert!(turn.is_complete);
    }

    #[test]
    fn test_turn_result_serde() {
        let turn = TurnResult {
            final_text: "test".to_string(),
            turns_completed: 5,
            usage: TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
                cache_read_tokens: None,
                cache_creation_tokens: None,
            },
            has_pending_tools: false,
            is_complete: true,
        };

        let json = serde_json::to_string(&turn).expect("serialize");
        let parsed: TurnResult = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(parsed.final_text, turn.final_text);
        assert_eq!(parsed.turns_completed, turn.turns_completed);
        assert_eq!(parsed.usage.input_tokens, turn.usage.input_tokens);
    }
}
