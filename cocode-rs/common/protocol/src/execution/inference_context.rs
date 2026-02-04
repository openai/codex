//! Complete inference context for LLM requests.

use super::AgentKind;
use super::ExecutionIdentity;
use crate::model::ModelInfo;
use crate::model::ModelSpec;
use crate::thinking::ThinkingLevel;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;

/// Complete inference context for an LLM request.
///
/// `InferenceContext` carries all the resolved information needed to build
/// a request, enabling centralized parameter assembly in `RequestBuilder`.
///
/// # Lifecycle
///
/// 1. Created by `ModelHub::prepare_main_with_selections()` after resolving the `ExecutionIdentity`
/// 2. Passed to `RequestBuilder` along with messages and tools
/// 3. Used by `RequestBuilder::build()` to assemble the final `GenerateRequest`
///
/// # Example
///
/// ```ignore
/// // In AgentLoop:
/// let (ctx, model) = model_hub.prepare_main_with_selections(
///     &self.selections,  // Session owns selections
///     session_id,
///     turn_number,
/// )?;
///
/// let request = RequestBuilder::new(ctx)
///     .messages(messages)
///     .tools(tools)
///     .build();
///
/// model.stream(request).await?;
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceContext {
    // === Identity Tracking ===
    /// Unique identifier for this inference call.
    pub call_id: String,

    /// Session ID for correlation.
    pub session_id: String,

    /// Turn number within the session.
    pub turn_number: i32,

    // === Resolved Model Configuration ===
    /// Resolved model specification (provider/model).
    pub model_spec: ModelSpec,

    /// Full model information (capabilities, limits, thinking config).
    pub model_info: ModelInfo,

    // === Thinking Configuration ===
    /// Merged thinking level (from RoleSelection override or ModelInfo default).
    ///
    /// This is the final, resolved thinking level to use for this request.
    /// It takes into account:
    /// 1. Explicit `RoleSelection.thinking_level` override (highest priority)
    /// 2. `ModelInfo.default_thinking_level` (fallback)
    /// 3. Model's supported thinking levels (for validation/snapping)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_level: Option<ThinkingLevel>,

    // === Agent Metadata ===
    /// Type of agent making this request.
    pub agent_kind: AgentKind,

    /// Original identity that was resolved to produce this context.
    ///
    /// Preserved for debugging and logging purposes.
    pub original_identity: ExecutionIdentity,

    // === Extended Parameters ===
    /// Merged extra parameters from ModelInfo.extra and ProviderInfo.extra.
    ///
    /// These are provider-specific parameters that get passed through to the SDK.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<HashMap<String, serde_json::Value>>,
}

impl InferenceContext {
    /// Create a new inference context.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        call_id: impl Into<String>,
        session_id: impl Into<String>,
        turn_number: i32,
        model_spec: ModelSpec,
        model_info: ModelInfo,
        agent_kind: AgentKind,
        original_identity: ExecutionIdentity,
    ) -> Self {
        Self {
            call_id: call_id.into(),
            session_id: session_id.into(),
            turn_number,
            model_spec,
            model_info,
            thinking_level: None,
            agent_kind,
            original_identity,
            extra: None,
        }
    }

    /// Set the thinking level.
    pub fn with_thinking_level(mut self, level: ThinkingLevel) -> Self {
        self.thinking_level = Some(level);
        self
    }

    /// Set extra parameters.
    pub fn with_extra(mut self, extra: HashMap<String, serde_json::Value>) -> Self {
        self.extra = Some(extra);
        self
    }

    /// Get the provider name.
    pub fn provider(&self) -> &str {
        &self.model_spec.provider
    }

    /// Get the model name.
    pub fn model(&self) -> &str {
        &self.model_spec.model
    }

    /// Get the context window size.
    pub fn context_window(&self) -> Option<i64> {
        self.model_info.context_window
    }

    /// Get the max output tokens.
    pub fn max_output_tokens(&self) -> Option<i64> {
        self.model_info.max_output_tokens
    }

    /// Get the timeout in seconds.
    pub fn timeout_secs(&self) -> Option<i64> {
        self.model_info.timeout_secs
    }

    /// Get the temperature.
    pub fn temperature(&self) -> Option<f32> {
        self.model_info.temperature
    }

    /// Get the top_p value.
    pub fn top_p(&self) -> Option<f32> {
        self.model_info.top_p
    }

    /// Check if thinking is enabled for this context.
    pub fn is_thinking_enabled(&self) -> bool {
        self.thinking_level.as_ref().is_some_and(|l| l.is_enabled())
    }

    /// Get the effective thinking level.
    ///
    /// Returns the explicitly set thinking level, or falls back to the
    /// model's default thinking level.
    pub fn effective_thinking_level(&self) -> Option<&ThinkingLevel> {
        self.thinking_level
            .as_ref()
            .or(self.model_info.default_thinking_level.as_ref())
    }

    /// Get an extra parameter value.
    pub fn get_extra(&self, key: &str) -> Option<&serde_json::Value> {
        self.extra.as_ref().and_then(|e| e.get(key))
    }

    /// Check if this is a main agent context.
    pub fn is_main(&self) -> bool {
        self.agent_kind.is_main()
    }

    /// Check if this is a subagent context.
    pub fn is_subagent(&self) -> bool {
        self.agent_kind.is_subagent()
    }

    /// Check if this is a compaction context.
    pub fn is_compaction(&self) -> bool {
        self.agent_kind.is_compaction()
    }

    /// Create a child context for a subagent.
    ///
    /// The child context inherits the model configuration but has its own
    /// call_id and agent_kind.
    pub fn child_context(
        &self,
        call_id: impl Into<String>,
        agent_type: impl Into<String>,
        identity: ExecutionIdentity,
    ) -> Self {
        Self {
            call_id: call_id.into(),
            session_id: self.session_id.clone(),
            turn_number: self.turn_number,
            model_spec: self.model_spec.clone(),
            model_info: self.model_info.clone(),
            thinking_level: self.thinking_level.clone(),
            agent_kind: AgentKind::subagent(&self.session_id, agent_type),
            original_identity: identity,
            extra: self.extra.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProviderType;
    use crate::model::ModelRole;

    fn sample_context() -> InferenceContext {
        let spec = ModelSpec::new("anthropic", "claude-opus-4");
        let info = ModelInfo {
            slug: "claude-opus-4".to_string(),
            context_window: Some(200000),
            max_output_tokens: Some(16384),
            temperature: Some(1.0),
            default_thinking_level: Some(ThinkingLevel::high()),
            ..Default::default()
        };

        InferenceContext::new(
            "call-123",
            "session-456",
            1,
            spec,
            info,
            AgentKind::Main,
            ExecutionIdentity::main(),
        )
    }

    #[test]
    fn test_new_context() {
        let ctx = sample_context();
        assert_eq!(ctx.call_id, "call-123");
        assert_eq!(ctx.session_id, "session-456");
        assert_eq!(ctx.turn_number, 1);
        assert_eq!(ctx.provider(), "anthropic");
        assert_eq!(ctx.model(), "claude-opus-4");
        assert_eq!(ctx.model_spec.provider_type, ProviderType::Anthropic);
    }

    #[test]
    fn test_model_info_accessors() {
        let ctx = sample_context();
        assert_eq!(ctx.context_window(), Some(200000));
        assert_eq!(ctx.max_output_tokens(), Some(16384));
        assert_eq!(ctx.temperature(), Some(1.0));
    }

    #[test]
    fn test_thinking_level() {
        let mut ctx = sample_context();

        // No explicit thinking level, falls back to model default
        assert!(ctx.thinking_level.is_none());
        assert!(ctx.effective_thinking_level().is_some());
        assert_eq!(
            ctx.effective_thinking_level().unwrap().effort,
            ThinkingLevel::high().effort
        );

        // Set explicit thinking level
        ctx = ctx.with_thinking_level(ThinkingLevel::medium());
        assert!(ctx.thinking_level.is_some());
        assert!(ctx.is_thinking_enabled());
        assert_eq!(
            ctx.effective_thinking_level().unwrap().effort,
            ThinkingLevel::medium().effort
        );
    }

    #[test]
    fn test_agent_kind_checks() {
        let ctx = sample_context();
        assert!(ctx.is_main());
        assert!(!ctx.is_subagent());
        assert!(!ctx.is_compaction());
    }

    #[test]
    fn test_child_context() {
        let parent = sample_context();
        let child = parent.child_context(
            "call-child",
            "explore",
            ExecutionIdentity::Role(ModelRole::Explore),
        );

        // Inherits model config
        assert_eq!(child.model_spec, parent.model_spec);
        assert_eq!(child.model_info, parent.model_info);
        assert_eq!(child.session_id, parent.session_id);

        // Has own identity
        assert_eq!(child.call_id, "call-child");
        assert!(child.is_subagent());
        assert_eq!(
            child.original_identity,
            ExecutionIdentity::Role(ModelRole::Explore)
        );
    }

    #[test]
    fn test_extra_parameters() {
        let mut extra = HashMap::new();
        extra.insert("seed".to_string(), serde_json::json!(42));

        let ctx = sample_context().with_extra(extra);

        assert_eq!(ctx.get_extra("seed"), Some(&serde_json::json!(42)));
        assert_eq!(ctx.get_extra("nonexistent"), None);
    }

    #[test]
    fn test_serde_roundtrip() {
        let ctx = sample_context().with_thinking_level(ThinkingLevel::high());

        let json = serde_json::to_string(&ctx).unwrap();
        let parsed: InferenceContext = serde_json::from_str(&json).unwrap();

        assert_eq!(ctx.call_id, parsed.call_id);
        assert_eq!(ctx.session_id, parsed.session_id);
        assert_eq!(ctx.model_spec, parsed.model_spec);
        assert_eq!(ctx.thinking_level, parsed.thinking_level);
    }
}
