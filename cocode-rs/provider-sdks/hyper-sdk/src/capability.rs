//! Model capabilities and metadata.

use serde::Deserialize;
use serde::Serialize;

/// Capabilities that a model may support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// Text generation (chat/completion).
    TextGeneration,
    /// Streaming responses.
    Streaming,
    /// Vision/image understanding.
    Vision,
    /// Audio input/output.
    Audio,
    /// Tool/function calling.
    ToolCalling,
    /// Embedding generation.
    Embedding,
    /// Extended thinking/reasoning.
    ExtendedThinking,
    /// Structured output (JSON mode).
    StructuredOutput,
}

/// Information about a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model identifier (e.g., "gpt-4o", "claude-3-opus").
    pub id: String,
    /// Human-readable name.
    pub name: Option<String>,
    /// Provider name (e.g., "openai", "anthropic").
    pub provider: String,
    /// Capabilities this model supports.
    pub capabilities: Vec<Capability>,
    /// Maximum context window in tokens.
    pub context_window: Option<i64>,
    /// Maximum output tokens.
    pub max_output_tokens: Option<i64>,
    /// Whether the model is deprecated.
    pub deprecated: bool,
}

impl ModelInfo {
    /// Create a new ModelInfo with minimal fields.
    pub fn new(id: impl Into<String>, provider: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: None,
            provider: provider.into(),
            capabilities: vec![Capability::TextGeneration],
            context_window: None,
            max_output_tokens: None,
            deprecated: false,
        }
    }

    /// Set the human-readable name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the capabilities.
    pub fn with_capabilities(mut self, capabilities: Vec<Capability>) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// Set the context window size.
    pub fn with_context_window(mut self, tokens: i64) -> Self {
        self.context_window = Some(tokens);
        self
    }

    /// Set the max output tokens.
    pub fn with_max_output_tokens(mut self, tokens: i64) -> Self {
        self.max_output_tokens = Some(tokens);
        self
    }

    /// Mark the model as deprecated.
    pub fn deprecated(mut self) -> Self {
        self.deprecated = true;
        self
    }

    /// Check if model has a specific capability.
    pub fn has_capability(&self, cap: Capability) -> bool {
        self.capabilities.contains(&cap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_info_builder() {
        let info = ModelInfo::new("gpt-4o", "openai")
            .with_name("GPT-4o")
            .with_capabilities(vec![
                Capability::TextGeneration,
                Capability::Streaming,
                Capability::Vision,
                Capability::ToolCalling,
            ])
            .with_context_window(128000)
            .with_max_output_tokens(4096);

        assert_eq!(info.id, "gpt-4o");
        assert_eq!(info.provider, "openai");
        assert_eq!(info.name, Some("GPT-4o".to_string()));
        assert!(info.has_capability(Capability::Vision));
        assert!(!info.has_capability(Capability::Embedding));
        assert_eq!(info.context_window, Some(128000));
    }

    #[test]
    fn test_capability_serde() {
        let cap = Capability::ExtendedThinking;
        let json = serde_json::to_string(&cap).unwrap();
        assert_eq!(json, "\"extended_thinking\"");

        let parsed: Capability = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, Capability::ExtendedThinking);
    }
}
